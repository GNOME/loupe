// Copyright (c) 2023 Sophie Herold
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: GPL-3.0-or-later

use super::*;

impl LpImage {
    pub fn init(&self, file: &gio::File) {
        self.imp().file.replace(Some(file.clone()));
    }

    pub async fn load(&self, file: &gio::File) {
        let imp = self.imp();
        log::debug!("Loading file {}", file.uri());

        imp.metadata.replace(Metadata::default());
        self.emmit_metadata_changed();
        self.set_file_loaded(file).await;

        let tiles = &self.imp().frame_buffer;
        // Delete all stored textures for reloads
        tiles.reset();
        // Reset background color for reloads
        self.set_background_color(None);

        let (decoder, mut decoder_update) =
            Decoder::new(file.clone(), self.metadata().mime_type(), tiles.clone()).await;

        let weak_obj = self.downgrade();
        glib::spawn_future_local(async move {
            while let Some(update) = decoder_update.next().await {
                if let Some(obj) = weak_obj.upgrade() {
                    obj.update(update);
                }
            }
            log::debug!("Stopped listening to decoder since sender is gone");
        });

        imp.decoder.replace(Some(Arc::new(decoder)));
    }

    // Set filename when (re)loading image
    async fn set_file_loaded(&self, file: &gio::File) {
        let imp = self.imp();

        // Do not check if same file since it might be set via initialized with
        // `Self::init` before
        imp.file.replace(Some(file.clone()));
        self.reload_file_info().await;
        self.setup_file_monitor().await;
    }

    /// Set filename after filename changed
    async fn set_file_changed(&self, file: &gio::File) {
        let imp = self.imp();
        let prev_mime_type = self.metadata().mime_type();

        imp.file.replace(Some(file.clone()));
        self.reload_file_info().await;

        // Reload if mime type changed since other decoding path might be responsible
        // now
        if self.metadata().mime_type() != prev_mime_type {
            self.load(file).await;
            return;
        }

        self.setup_file_monitor().await;
    }

    async fn setup_file_monitor(&self) {
        let imp = self.imp();

        if let Some(file) = self.file() {
            let monitor =
                file.monitor_file(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE);
            if let Ok(m) = &monitor {
                m.connect_changed(
                    glib::clone!(@weak self as obj => move |_, file_a, file_b, event| {
                        obj.file_changed(event, file_a, file_b);
                    }),
                );
            }

            imp.file_monitor.replace(monitor.ok());
        }
    }

    /// Called when decoder sends update
    pub fn update(&self, update: DecoderUpdate) {
        let imp = self.imp();

        match update {
            DecoderUpdate::Metadata(metadata) => {
                log::debug!("Received metadata");
                imp.metadata.borrow_mut().merge(metadata);
                self.emmit_metadata_changed();

                self.reset_rotation();
            }
            DecoderUpdate::Dimensions(dimension_details) => {
                log::debug!("Received dimensions: {:?}", self.original_dimensions());
                self.imp().dimension_details.replace(dimension_details);
                self.notify_image_size_available();
                self.configure_best_fit();
                self.request_tiles();
            }
            DecoderUpdate::Redraw => {
                self.set_loaded(true);

                self.queue_draw();
                imp.frame_buffer.rcu(|tiles| {
                    let mut new_tiles = (**tiles).clone();
                    new_tiles.cleanup(imp.zoom_target.get(), self.preload_area());
                    new_tiles
                });
                if imp.background_color.borrow().is_none() {
                    glib::spawn_future_local(glib::clone!(@weak self as obj => async move {
                        let color = obj.background_color_guess().await;
                        obj.set_background_color(color);
                        if obj.is_mapped() {
                            obj.queue_draw();
                        }
                    }));
                }
            }
            DecoderUpdate::Error(err) => {
                self.set_error(Some(err));
            }
            DecoderUpdate::Format(format) => {
                imp.metadata.borrow_mut().set_format(format);
                self.emmit_metadata_changed();
            }
            DecoderUpdate::Animated => {
                let callback_id = self
                        .add_tick_callback(glib::clone!(@weak self as obj => @default-return glib::ControlFlow::Break, move |_, clock| obj.tick_callback(clock)));
                imp.tick_callback.replace(Some(callback_id));
            }
            DecoderUpdate::UnsupportedFormat => {
                self.set_unsupported(true);
            }
        }
    }

    /// Roughly called for every frame if image is visible
    ///
    /// We handle advancing to the next frame for animated GIFs etc here.
    fn tick_callback(&self, clock: &gdk::FrameClock) -> glib::ControlFlow {
        // Do not animate if not visible
        if !self.is_mapped() {
            return glib::ControlFlow::Continue;
        }

        let elapsed = clock.frame_time() - self.imp().last_animated_frame.get();
        let duration = std::time::Duration::from_micros(elapsed as u64);

        // Check if it's time to show the next frame
        if self.imp().frame_buffer.frame_timeout(duration) {
            // Just draw since frame_timeout updated to new frame
            self.queue_draw();
            self.imp().last_animated_frame.set(clock.frame_time());
            if let Some(decoder) = self.imp().decoder.borrow().as_ref() {
                // Decode new frame and load it into buffer
                decoder.fill_frame_buffer();
            }
        }

        glib::ControlFlow::Continue
    }

    fn preload_area(&self) -> graphene::Rect {
        let viewport = self.viewport();
        viewport.inset_r(-viewport.width() / 3., -viewport.height() / 3.)
    }

    pub(super) fn request_tiles(&self) {
        if let Some(decoder) = self.imp().decoder.borrow().as_ref() {
            if self.zoom_animation().state() != adw::AnimationState::Playing {
                // Force minimum tile size of 1000x1000 since with smaller
                // tiles the tiled rendering advantage disappears
                let x_inset = f32::min(-3., (self.viewport().width() - 1000.) / 2.);
                let y_inset = f32::min(-3., (self.viewport().height() - 1000.) / 2.);

                decoder.request(crate::decoder::TileRequest {
                    viewport: self.viewport().inset_r(x_inset, y_inset),
                    area: self.preload_area(),
                    zoom: self.imp().zoom_target.get(),
                });
            }
        }
    }

    /// File changed on drive
    fn file_changed(
        &self,
        event: gio::FileMonitorEvent,
        file_a: &gio::File,
        file_b: Option<&gio::File>,
    ) {
        match event {
            gio::FileMonitorEvent::Renamed => {
                if let Some(file) = file_b.cloned() {
                    log::debug!("Moved to {}", file.uri());
                    // current file got replaced with a new one
                    let file_replace = self.file().map_or(false, |x| x.equal(&file));
                    let obj = self.clone();
                    if file_replace {
                        log::debug!("Image got replaced {}", file.uri());
                        // TODO: error handling is missing
                        glib::spawn_future_local(async move {
                            obj.load(&file).await;
                        });
                    } else {
                        glib::spawn_future_local(async move {
                            obj.set_file_changed(&file).await;
                        });
                    }
                }
            }
            gio::FileMonitorEvent::ChangesDoneHint => {
                let obj = self.clone();
                let file = file_a.clone();
                log::debug!("Image was edited {}", file.uri());
                // TODO: error handling is missing
                glib::spawn_future_local(async move {
                    obj.load(&file).await;
                });
            }
            gio::FileMonitorEvent::Deleted
            | gio::FileMonitorEvent::MovedOut
            | gio::FileMonitorEvent::Unmounted => {
                log::debug!("File no longer available: {event:?} {}", file_a.uri());
                self.imp().is_deleted.set(true);
                self.notify_is_deleted();
            }
            _ => {}
        }
    }

    fn set_error(&self, err: Option<anyhow::Error>) {
        log::debug!("Decoding error: {err:?}");
        self.imp()
            .error
            .replace(err.as_ref().map(|x| x.to_string()));
        self.notify_error();

        if err.is_some() {
            self.set_loaded(false);
        }
    }

    fn set_unsupported(&self, is_unsupported: bool) {
        if self.is_unsupported() != is_unsupported {
            self.imp().is_unsupported.set(true);
            self.notify_is_unsupported();

            if is_unsupported {
                self.set_loaded(false);
            }
        }
    }

    fn set_loaded(&self, is_loaded: bool) {
        if self.is_loaded() != is_loaded {
            self.imp().is_loaded.set(is_loaded);
            self.notify_is_loaded();

            if is_loaded {
                self.set_error(None);
                self.set_unsupported(false);
            }
        }
    }
}
