// Copyright (c) 2023-2025 Sophie Herold
// Copyright (c) 2024 Fina Wilke
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

use decoder::DecoderError;

use super::*;
use crate::widgets::LpEditWindow;

impl imp::LpImage {
    // Set filename when (re)loading image
    async fn set_file_loaded(&self, file: &gio::File) {
        // Do not check if same file since it might be set via initialized with
        // `Self::init` before
        self.file.replace(Some(file.clone()));
        self.reload_file_info().await;
        self.setup_file_monitor().await;
    }

    /// Set filename after filename changed
    async fn set_file_changed(&self, file: &gio::File) {
        let obj = self.obj();
        let prev_mime_type = obj.metadata().unreliable_mime_type();

        self.file.replace(Some(file.clone()));
        self.reload_file_info().await;

        // Reload if mime type changed since other decoding path might be responsible
        // now
        if obj.metadata().unreliable_mime_type() != prev_mime_type {
            obj.load(file).await;
            return;
        }

        self.setup_file_monitor().await;
    }

    async fn setup_file_monitor(&self) {
        let obj = self.obj();

        if let Some(file) = obj.file() {
            let monitor =
                file.monitor_file(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE);
            if let Ok(m) = &monitor {
                m.connect_changed(glib::clone!(
                    #[weak]
                    obj,
                    move |_, file_a, file_b, event| {
                        obj.imp().file_changed(event, file_a, file_b);
                    }
                ));
            }

            self.file_monitor.replace(monitor.ok());
        }
    }

    /// Called when decoder sends update
    pub fn update(&self, update: DecoderUpdate) {
        let obj = self.obj();

        match update {
            DecoderUpdate::Metadata(metadata) => {
                log::debug!("Received metadata");
                self.metadata.borrow_mut().merge(*metadata);
                self.emmit_metadata_changed();

                obj.reset_rotation();
                glib::spawn_future_local(glib::clone!(
                    #[weak(rename_to=obj)]
                    self,
                    async move { obj.check_editable().await },
                ));
            }
            DecoderUpdate::Dimensions => {
                log::debug!("Received dimensions: {:?}", self.untransformed_dimensions());
                obj.notify_image_size_available();
                self.configure_best_fit();
                self.request_tiles();
                self.configure_adjustments();
            }
            DecoderUpdate::Redraw => {
                self.set_loaded(true);
                self.previous_frame_buffer.reset();

                obj.queue_draw();
                self.frame_buffer.rcu(|tiles| {
                    let mut new_tiles = (**tiles).clone();
                    new_tiles.cleanup(self.zoom_target.get(), self.preload_area());
                    new_tiles
                });
                if self.background_color.borrow().is_none() {
                    glib::spawn_future_local(glib::clone!(
                        #[weak]
                        obj,
                        async move {
                            let imp = obj.imp();

                            let color = imp.background_color_guess().await;
                            imp.set_background_color(color);
                            if obj.is_mapped() {
                                obj.queue_draw();
                            }
                        }
                    ));
                }
            }
            DecoderUpdate::Animated => {
                if self.still.get() {
                    // Just load the first frame
                    self.frame_buffer.next_frame();
                } else {
                    let callback_id = obj.add_tick_callback(glib::clone!(
                        #[weak(rename_to = obj)]
                        self,
                        #[upgrade_or]
                        glib::ControlFlow::Break,
                        move |_, clock| obj.tick_callback(clock)
                    ));
                    self.tick_callback.replace(Some(callback_id));
                }
            }
            DecoderUpdate::SpecificError(err) => {
                if self.obj().is_loaded() {
                    log::warn!("Error occured while loading additional data: {err:?}");
                } else {
                    self.set_specific_error(err);
                }
            }
            DecoderUpdate::GenericError(err) => {
                if self.obj().is_loaded() {
                    log::warn!("Error occured while loading additional data: {err:?}");
                } else {
                    self.set_error(Some(err));
                }
            }
        }
    }

    /// Roughly called for every frame if image is visible
    ///
    /// We handle advancing to the next frame for animated GIFs etc here.
    fn tick_callback(&self, clock: &gdk::FrameClock) -> glib::ControlFlow {
        let obj = self.obj();

        // Do not animate if not visible
        if !obj.is_mapped() {
            return glib::ControlFlow::Continue;
        }

        let elapsed = clock.frame_time() - self.last_animated_frame.get();
        let duration = std::time::Duration::from_micros(elapsed as u64);

        // Check if it's time to show the next frame
        if self.frame_buffer.frame_timeout(duration) {
            // Just draw since frame_timeout updated to new frame
            obj.queue_draw();
            self.last_animated_frame.set(clock.frame_time());
            if let Some(decoder) = self.decoder.borrow().as_ref() {
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
        if let Some(decoder) = self.decoder.borrow().as_ref() {
            if self.zoom_animation().state() != adw::AnimationState::Playing {
                // Force minimum tile size of 1000x1000 since with smaller
                // tiles the tiled rendering advantage disappears
                let x_inset = f32::min(-3., (self.viewport().width() - 1000.) / 2.);
                let y_inset = f32::min(-3., (self.viewport().height() - 1000.) / 2.);

                decoder.request(crate::decoder::TileRequest {
                    viewport: self.viewport().inset_r(x_inset, y_inset),
                    area: self.preload_area(),
                    zoom: self.zoom_target.get(),
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
        let obj = self.obj().to_owned();

        match event {
            gio::FileMonitorEvent::Renamed => {
                if let Some(file) = file_b.cloned() {
                    log::debug!("Moved to {}", file.uri());

                    // current file got replaced with a new one
                    let file_replace = obj.file().is_some_and(|x| x.equal(&file));
                    if file_replace {
                        log::debug!("Image got replaced {}", file.uri());
                        glib::spawn_future_local(async move {
                            obj.load(&file).await;
                        });
                    } else {
                        glib::spawn_future_local(async move {
                            if obj.error().is_some() {
                                // Tmp files might be renamed quickly after creation. In this case
                                // the original load fails, because the filename is already invalid.
                                // Therefore reload on name change loading caused an error.
                                obj.load(&file).await;
                            } else {
                                obj.imp().set_file_changed(&file).await;
                            }
                        });
                    }
                }
            }
            gio::FileMonitorEvent::MovedIn => {
                log::debug!("Image got replaced by moving into dir {}", file_a.uri());
                glib::spawn_future_local(glib::clone!(
                    #[strong]
                    file_a,
                    async move {
                        obj.load(&file_a).await;
                    }
                ));
            }
            gio::FileMonitorEvent::ChangesDoneHint => {
                let obj = self.obj().to_owned();
                let file = file_a.clone();
                log::debug!("Image was edited {}", file.uri());
                glib::spawn_future_local(async move {
                    obj.load(&file).await;
                });
            }
            gio::FileMonitorEvent::Deleted
            | gio::FileMonitorEvent::MovedOut
            | gio::FileMonitorEvent::Unmounted => {
                log::debug!("File no longer available: {event:?} {}", file_a.uri());
                self.is_deleted.set(true);
                self.obj().notify_is_deleted();
            }
            _ => {}
        }
    }

    fn set_error(&self, err: Option<anyhow::Error>) {
        log::debug!("Decoding error: {err:?}");

        // Keeping first error instead of replacing with new error
        if err.is_some() && self.error.borrow().is_some() {
            return;
        }

        self.error.replace(err.as_ref().map(|x| x.to_string()));
        self.obj().notify_error();

        if err.is_some() {
            self.set_loaded(false);
        }
    }

    fn set_specific_error(&self, error: DecoderError) {
        let obj = self.obj();
        if obj.specific_error() != error {
            self.specific_error.set(error);
            obj.notify_specific_error();

            if error.is_err() {
                self.set_loaded(false);
            }
        }
    }

    fn set_loaded(&self, is_loaded: bool) {
        let obj = self.obj();

        if obj.is_loaded() != is_loaded {
            self.is_loaded.set(is_loaded);
            obj.notify_is_loaded();

            if is_loaded {
                self.set_error(None);
                self.set_specific_error(DecoderError::None);
            }
        }
    }

    async fn check_editable(&self) {
        let obj = self.obj();

        if let Some(mime_type) = obj.metadata().mime_type() {
            if let Ok(supported_formats) = glycin::config::Config::cached()
                .await
                .editor(&mime_type.as_str().into())
            {
                if LpEditWindow::REQUIRED_OPERATIONS
                    .iter()
                    .all(|x| supported_formats.operations.contains(x))
                {
                    self.editable.set(true);
                    obj.notify_editable();
                }
            }
        };
    }
}

impl LpImage {
    pub fn init(&self, file: &gio::File) {
        self.imp().file.replace(Some(file.clone()));
    }

    pub async fn reload(&self) {
        if let Some(file) = self.file() {
            self.load(&file).await;
        } else {
            log::error!("Trying to reload image without file");
        }
    }

    pub async fn load(&self, file: &gio::File) {
        let imp = self.imp();
        log::debug!("Loading file {}", file.uri());

        if imp.rotation_animation().state() == adw::AnimationState::Playing {
            log::debug!("Queueing image reload due to playing rotate animation.");
            self.imp().queued_reload.replace(Some(file.clone()));
            return;
        }

        imp.metadata.replace(Metadata::default());
        self.imp().emmit_metadata_changed();
        self.imp().set_file_loaded(file).await;

        let tiles = &self.imp().frame_buffer;

        // Delete all stored textures for reloads
        let previous_frame_buffer = tiles.reset();
        // Store previos frames to show until new texture is loaded
        self.imp()
            .previous_frame_buffer
            .swap(previous_frame_buffer.load_full());
        // Reset background color for reloads
        imp.set_background_color(None);

        let (decoder, decoder_update) = Decoder::new(
            file.clone(),
            self.metadata().unreliable_mime_type(),
            tiles.clone(),
        )
        .await;

        let weak_obj = self.downgrade();
        glib::spawn_future_local(async move {
            while let Ok(update) = decoder_update.recv().await {
                if let Some(obj) = weak_obj.upgrade() {
                    obj.imp().update(update);
                }
            }
            log::debug!("Stopped listening to decoder since sender is gone");
        });

        imp.decoder.replace(Some(Arc::new(decoder)));
    }
}
