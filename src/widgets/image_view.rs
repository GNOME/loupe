// Copyright (c) 2020-2023 Christopher Davis
// Copyright (c) 2022-2024 Sophie Herold
// Copyright (c) 2022 Maximiliano Sandoval R
// Copyright (c) 2023 FineFindus
// Copyright (c) 2023 Huan Nguyen
// Copyright (c) 2023 Philipp Kiemle
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

//! A widget that shows the [`LpSlidingView`] and coordinates it's content
//!
//! This widget coordinates the content of [`LpSlidingView`] with the
//! [`LpFileModel`]. It also handles input that changes the image.
//!
//! [`LpSlidingView`]: crate::widgets::LpSlidingView
//! [`LpFileModel`]: crate::file_model::LpFileModel

use std::cell::{Cell, OnceCell, RefCell};
use std::marker::PhantomData;

use adw::prelude::*;
use adw::subclass::prelude::*;
use anyhow::Context;
use ashpd::desktop::{wallpaper, ResponseError};
use ashpd::{Error, WindowIdentifier};
use glib::translate::IntoGlib;
use glib::{clone, Properties};
use gtk::CompositeTemplate;

use crate::decoder::DecoderError;
use crate::deps::*;
use crate::file_model::LpFileModel;
use crate::util::gettext::*;
use crate::util::{Direction, Position};
use crate::widgets::{LpImage, LpImagePage, LpPrint, LpSlidingView};

// The number of pages we want to buffer
// on either side of the current page.
const BUFFER: usize = 2;

mod imp {

    use super::*;

    #[derive(Debug, Default, CompositeTemplate, Properties)]
    #[properties(wrapper_type = super::LpImageView)]
    #[template(file = "image_view.ui")]
    pub struct LpImageView {
        /// Direct child of this Adw::Bin
        #[template_child]
        pub(super) bin_child: TemplateChild<gtk::Widget>,
        /// overlayed controls
        #[template_child]
        pub(super) controls_box_start: TemplateChild<gtk::Widget>,
        #[template_child]
        pub(super) controls_box_start_events: TemplateChild<gtk::EventControllerMotion>,
        /// overlayed controls
        #[template_child]
        pub(super) controls_box_end: TemplateChild<gtk::Widget>,
        #[template_child]
        pub(super) controls_box_end_events: TemplateChild<gtk::EventControllerMotion>,
        #[template_child]
        pub(super) fullscreen_button: TemplateChild<gtk::Button>,

        #[template_child]
        pub sliding_view: TemplateChild<LpSlidingView>,

        pub drag_source: gtk::DragSource,

        /// File where trash operation has been undone
        #[property(get, set, nullable)]
        trash_restore: RefCell<Option<gio::File>>,

        pub(super) model: RefCell<LpFileModel>,
        pub(super) preserve_content: Cell<bool>,

        pub(super) current_image_signals: OnceCell<glib::SignalGroup>,

        #[property(get = Self::current_page)]
        _current_page: PhantomData<Option<LpImagePage>>,

        #[property(get = Self::is_previous_available)]
        _is_previous_available: PhantomData<bool>,

        #[property(get = Self::is_next_available)]
        _is_next_available: PhantomData<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpImageView {
        const NAME: &'static str = "LpImageView";
        type Type = super::LpImageView;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for LpImageView {
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec)
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }

        fn constructed(&self) {
            let obj = self.obj();

            self.parent_constructed();

            // Manually manage widget layout, see `WidgetImpl` for details
            obj.set_layout_manager(None::<gtk::LayoutManager>);

            self.sliding_view.connect_current_page_notify(glib::clone!(
                #[weak]
                obj,
                move |_| obj.page_changed()
            ));

            self.sliding_view.connect_target_page_reached(glib::clone!(
                #[weak]
                obj,
                move || obj.target_page_reached()
            ));

            let signal_group = obj.current_image_signals();

            let view = &*obj;
            signal_group.connect_closure(
                "notify::file",
                true,
                glib::closure_local!(
                    #[watch]
                    view,
                    move |_: LpImage, _: glib::ParamSpec| {
                        view.current_image_path_changed();
                    }
                ),
            );

            signal_group.connect_closure(
                "notify::is-deleted",
                true,
                glib::closure_local!(
                    #[watch]
                    view,
                    move |_: LpImage, _: glib::ParamSpec| {
                        view.current_image_path_changed();
                    }
                ),
            );

            self.drag_source.set_exclusive(true);

            self.drag_source.connect_prepare(glib::clone!(
                #[weak]
                obj,
                #[upgrade_or_default]
                move |gesture, _, _| {
                    let is_scrollable = obj
                        .current_page()
                        .map(|p| p.image().is_hscrollable() || p.image().is_vscrollable());

                    if is_scrollable == Some(true)
                        || gesture.device().map(|x| x.source())
                            == Some(gdk::InputSource::Touchscreen)
                    {
                        // Do scrolling if scrollable and no drag and drop on touchscreen
                        gesture.set_state(gtk::EventSequenceState::Denied);
                        None
                    } else {
                        gesture.set_state(gtk::EventSequenceState::Claimed);
                        obj.current_page().and_then(|p| p.content_provider())
                    }
                }
            ));

            self.drag_source.connect_drag_begin(glib::clone!(
                #[weak]
                obj,
                move |source, _| {
                    if let Some(paintable) = obj.current_image().and_then(|p| p.thumbnail()) {
                        // -6 for cursor width, +16 for margin in .drag-icon
                        source.set_icon(
                            Some(&paintable),
                            paintable.intrinsic_width() / 2 - 6 + 16,
                            -12,
                        );
                        if let Some(drag) = source.drag() {
                            let drag_icon = gtk::DragIcon::for_drag(&drag);
                            // Rounds corners, adds outline and shadow
                            drag_icon.add_css_class("drag-icon");
                            // Make border-radius clip the image
                            drag_icon.set_overflow(gtk::Overflow::Hidden);
                        }
                    };
                }
            ));

            obj.add_controller(self.drag_source.clone());
        }
    }

    impl WidgetImpl for LpImageView {
        /// This manual implementation makes sure left an right overlay both fit
        /// into the window
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            // Measure child of AdwBin
            let (child_min, child_natural, _, _) = self.bin_child.measure(orientation, for_size);

            // measure size of controls overlays
            let (overlay1_min, _, _, _) = self.controls_box_start.measure(orientation, for_size);
            let (overlay2_min, _, _, _) = self.controls_box_end.measure(orientation, for_size);

            let overlay_min = match orientation {
                gtk::Orientation::Horizontal => overlay1_min + overlay2_min,
                gtk::Orientation::Vertical => i32::max(overlay1_min, overlay2_min),
                _ => unreachable!(),
            };

            // take as minimum whatever is larger
            let min = i32::max(child_min, overlay_min);
            // take as natural whatever is larger
            let natural = i32::max(child_natural, min);

            (min, natural, -1, -1)
        }

        /// this is necessary because we do layout manually
        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.bin_child.allocate(width, height, baseline, None);
        }
    }

    impl BinImpl for LpImageView {}

    impl LpImageView {
        pub fn current_page(&self) -> Option<LpImagePage> {
            self.sliding_view.current_page()
        }

        /// Returns `true` if there is an image before the current one
        pub fn is_previous_available(&self) -> bool {
            let obj = self.obj();

            if let Some(file) = obj.current_file() {
                obj.model().index_of(&file) != Some(0)
            } else {
                false
            }
        }

        /// Returns `true` if there is an image after the current one
        pub fn is_next_available(&self) -> bool {
            let obj = self.obj();

            if let Some(file) = obj.current_file() {
                let model = obj.model();
                model.index_of(&file) != Some(model.n_files().saturating_sub(1))
            } else {
                false
            }
        }
    }
}

glib::wrapper! {
    pub struct LpImageView(ObjectSubclass<imp::LpImageView>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl LpImageView {
    pub fn set_images_from_files(&self, files: Vec<gio::File>) {
        // Add image to recently used file. Does not work in Flatpaks:
        // <https://github.com/flatpak/xdg-desktop-portal/issues/215>
        let recent_manager = gtk::RecentManager::default();
        for file in files.iter() {
            recent_manager.add_item(&file.uri());
        }

        if files.len() == 1 {
            self.load_file(files[0].clone());
        } else {
            self.load_files(files);
        }
    }

    fn load_files(&self, files: Vec<gio::File>) {
        let sliding_view = self.sliding_view().editor();

        if let Some(first) = files.first().cloned() {
            let model = LpFileModel::from_files(files);
            self.set_model(model);
            let page = self.new_image_page(&first);

            sliding_view.clear_lazy();
            sliding_view.append(&page);
            self.update_sliding_view(&first);
        } else {
            log::error!("File list was empty");
        }
    }

    // Builds an `LpFileModel`, which is an implementation of `gio::ListModel`
    // that holds a `gio::File` for each child within the same directory as the
    // file we pass. This model will update with changes to the directory,
    // and in turn we'll update our `adw::Carousel`.
    fn load_file(&self, file: gio::File) {
        let directory = file.parent();

        let is_same_directory = if let (Some(d1), Some(d2)) = (&directory, self.model().directory())
        {
            d1.equal(&d2)
        } else {
            false
        };

        if is_same_directory {
            log::debug!("Re-using old model and navigating to the current file");
            self.navigate_to_file(&file);
            return;
        }

        let model = LpFileModel::from_file(file.clone());
        self.set_model(model);
        log::debug!("New model created");

        self.update_sliding_view(&file);

        // List other files in directory
        if let Some(directory) = directory {
            glib::spawn_future_local(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                async move {
                    if let Err(err) = obj.model().load_directory(directory.clone()).await {
                        log::warn!("Failed to load directory: {}", err.root_cause());
                        obj.activate_action(
                            "win.show-toast",
                            Some(
                                &(
                                    format!("{} {}", err, err.root_cause()),
                                    adw::ToastPriority::High.into_glib(),
                                )
                                    .to_variant(),
                            ),
                        )
                        .unwrap();
                        return;
                    }

                    obj.update_sliding_view(&file);
                }
            ));
        }
    }

    /// Move forward or backwards
    pub fn navigate(&self, direction: Direction, animated: bool) {
        if let Some(current_file) = self.current_file() {
            let new_file = match direction {
                Direction::Forward => self.model().after(&current_file),
                Direction::Back => self.model().before(&current_file),
            };

            if let Some(new_file) = new_file {
                self.scroll_sliding_view(&new_file, animated);
            }
        }
    }

    /// Jump to position
    pub fn jump(&self, position: Position) {
        let new_file = match position {
            Position::First => self.model().first(),
            Position::Last => self.model().last(),
        };

        if let Some(new_file) = new_file {
            self.navigate_to_file(&new_file);
        }
    }

    /// Used for drag and drop
    fn navigate_to_file(&self, new_file: &gio::File) {
        let sliding_view = self.sliding_view();

        if self
            .current_file()
            .is_some_and(|current_file| current_file.equal(new_file))
        {
            return;
        }

        let Some(new_index) = self.model().index_of(new_file) else {
            log::warn!("Could not navigate to file {}", new_file.uri());
            return;
        };

        let current_index = self
            .current_file()
            .and_then(|x| self.model().index_of(&x))
            .unwrap_or_default();

        let page = if let Some(page) = sliding_view.get(new_file) {
            page
        } else {
            let page = self.new_image_page(new_file);

            if new_index > current_index {
                sliding_view.append(&page);
            } else {
                sliding_view.prepend(&page);
            }

            page
        };

        log::debug!("Scrolling to page for {}", new_file.uri());
        self.imp().preserve_content.set(true);
        sliding_view.animate_to(&page);
    }

    /// Ensures the sliding view contains the correct images
    fn update_sliding_view(&self, current_file: &gio::File) {
        log::debug!(
            "Updating sliding_view neighbors for current path {}",
            current_file.uri()
        );
        let sliding_view = self.sliding_view().editor();

        self.imp().preserve_content.set(false);

        let existing = sliding_view.pages();
        let target = self.model().files_around(current_file, BUFFER);

        // remove old pages
        for (uri, page) in &existing {
            if !target.contains_key(uri) {
                sliding_view.remove_lazy(page);
            }
        }

        // add missing pages or put in correct position
        for (position, (_, file)) in target.iter().enumerate() {
            if let Some(page) = existing.get(&file.uri()) {
                sliding_view.move_to(page, position);
            } else {
                sliding_view.insert(&self.new_image_page(file), position);
            }
        }

        self.notify_is_previous_available();
        self.notify_is_next_available();
    }

    fn scroll_sliding_view(&self, file: &gio::File, animated: bool) {
        let Some(current_page) = self.sliding_view().pages().swap_remove(&file.uri()) else {
            log::error!(
                "Current path not available in sliding_view for scrolling: {}",
                file.uri()
            );
            return;
        };

        if animated {
            self.sliding_view().animate_to(&current_page);
        } else {
            self.sliding_view().instant_to(&current_page);
        }
    }

    fn sliding_view(&self) -> LpSlidingView {
        self.imp().sliding_view.clone()
    }

    pub fn controls_box_start(&self) -> gtk::Widget {
        self.imp().controls_box_start.clone()
    }

    pub fn controls_box_start_events(&self) -> gtk::EventControllerMotion {
        self.imp().controls_box_start_events.clone()
    }

    pub fn controls_box_end(&self) -> gtk::Widget {
        self.imp().controls_box_end.clone()
    }

    pub fn controls_box_end_events(&self) -> gtk::EventControllerMotion {
        self.imp().controls_box_end_events.clone()
    }

    fn model(&self) -> LpFileModel {
        self.imp().model.borrow().clone()
    }

    /// Create new image that communicates updates to file model
    fn new_image_page(&self, file: &gio::File) -> LpImagePage {
        let page = LpImagePage::from_file(file);

        page.image().connect_specific_error_notify(glib::clone!(
            #[weak(rename_to = obj)]
            self,
            move |image| {
                if image.specific_error() == DecoderError::UnsupportedFormat {
                    if obj.current_image().as_ref() == Some(image) {
                        log::debug!(
                            "Image format unsupported but not removing since current image"
                        );
                        return;
                    }

                    if let Some(file) = image.file() {
                        log::debug!("Removing image with unsupported format {:?}", file.uri());
                        obj.model().remove(&file);
                        if let Some(current_file) = obj.current_file() {
                            obj.update_sliding_view(&current_file);
                        }
                    }
                }
            }
        ));

        page
    }

    fn set_model(&self, model: LpFileModel) {
        model.connect_changed(glib::clone!(
            #[weak(rename_to = obj)]
            self,
            move || obj.model_content_changed_cb()
        ));
        self.imp().model.replace(model);
    }

    /// Handle files are added or removed from directory
    fn model_content_changed_cb(&self) {
        // Animate to image that was restored from trash
        if let Some(trash_restore) = self.trash_restore() {
            if self.model().contains(&trash_restore) {
                self.set_trash_restore(gio::File::NONE);
                self.update_sliding_view(&trash_restore);
                self.scroll_sliding_view(&trash_restore, true);
                return;
            }
        }

        let Some(current_file) = self.current_file() else {
            return;
        };

        // LpImage did not get the update yet
        // Update will be handled by current_image_path_changed
        if !self.model().contains(&current_file) {
            return;
        }

        self.update_sliding_view(&current_file);
    }

    /// Handle current image being moved or deleted
    ///
    /// This is handled separately since we want to delete animations and
    /// want to still show the same image if renamed.
    fn current_image_path_changed(&self) {
        if let Some(image) = self.current_page().map(|x| x.image()) {
            if image.is_deleted() {
                self.sliding_view().scroll_to_neighbor();
            }
        }

        if let Some(current_file) = self.current_file() {
            if self.model().contains(&current_file) && !self.imp().preserve_content.get() {
                self.update_sliding_view(&current_file);
            }
        }
    }

    pub fn current_image(&self) -> Option<LpImage> {
        self.imp().sliding_view.current_page().map(|x| x.image())
    }

    pub fn current_image_signals(&self) -> &glib::SignalGroup {
        self.imp().current_image_signals.get_or_init(move || {
            let signal_group = glib::SignalGroup::new::<LpImage>();
            self.connect_current_page_notify(clone!(
                #[weak]
                signal_group,
                move |obj| {
                    signal_group.set_target(obj.current_image().as_ref());
                }
            ));
            signal_group
        })
    }

    pub fn current_uri(&self) -> Option<glib::GString> {
        self.imp()
            .sliding_view
            .current_page()
            .map(|x| x.file().uri())
    }

    pub fn current_file(&self) -> Option<gio::File> {
        self.imp()
            .sliding_view
            .current_page()
            .and_then(|x| x.image().file())
    }

    pub fn drag_source(&self) -> gtk::DragSource {
        self.imp().drag_source.clone()
    }

    fn page_changed(&self) {
        self.notify_current_page();
        self.notify_is_next_available();
        self.notify_is_previous_available();

        let Some(new_page) = self.current_page() else {
            log::debug!("Page changed but no current page");
            return;
        };

        if !self.imp().preserve_content.get() {
            self.update_sliding_view(&new_page.file());
        }
    }

    /// Called when page change animation completed
    fn target_page_reached(&self) {
        let current_page = self.current_page();

        if self.imp().preserve_content.get() {
            if let Some(new_page) = &current_page {
                self.update_sliding_view(&new_page.file());
            } else {
                log::error!("No LpImagePage");
            }
        }

        // Reset zoom and rotation of other pages
        if let Some(new_page) = &current_page {
            let mut other_pages = self.sliding_view().pages();
            other_pages.swap_remove(&new_page.file().uri());
            for (_, page) in other_pages {
                let image = page.image();
                image.reset_rotation();
                image.set_best_fit(true);
                image.zoom_best_fit();
            }
        }
    }

    pub fn on_fullscreen_changed(&self, is_fullscreened: bool) {
        let icon = if is_fullscreened {
            "view-restore-symbolic"
        } else {
            "view-fullscreen-symbolic"
        };

        self.imp().fullscreen_button.set_icon_name(icon);
    }

    pub fn rotate_image(&self, angle: f64) {
        if let Some(current_page) = self.current_page() {
            current_page.image().rotate_by(angle);
        }
    }

    pub async fn set_background(&self) -> anyhow::Result<()> {
        let uri = self
            .current_file()
            .and_then(|f| url::Url::parse(f.uri().as_str()).ok())
            .context("Invalid URL for background image")?;
        let native = self.native().context("View should have a GtkNative")?;
        let id = WindowIdentifier::from_native(&native).await;

        match wallpaper::WallpaperRequest::default()
            .set_on(wallpaper::SetOn::Background)
            .show_preview(true)
            .identifier(id)
            .build_uri(&uri)
            .await
            .and_then(|req| req.response())
        {
            Ok(_) => self
                .activate_action(
                    "win.show-toast",
                    Some(
                        &(
                            // Translators: This is a toast notification, informing the user that
                            // an image has been set as background.
                            gettext("Set as background."),
                            adw::ToastPriority::High.into_glib(),
                        )
                            .to_variant(),
                    ),
                )
                .unwrap(),
            Err(err) => {
                if !matches!(err, Error::Response(ResponseError::Cancelled)) {
                    self.activate_action(
                        "win.show-toast",
                        Some(
                            &(
                                gettext("Could not set background."),
                                adw::ToastPriority::High.into_glib(),
                            )
                                .to_variant(),
                        ),
                    )
                    .unwrap();
                }
            }
        };

        Ok(())
    }

    pub fn print(&self) -> anyhow::Result<()> {
        let image = self
            .current_image()
            .context("No current image for printing")?;

        let root = self.root().context("Could not get root for widget")?;
        let window = root
            .downcast_ref::<gtk::Window>()
            .context("Could not downcast to GtkWindow")?;

        LpPrint::show_print_dialog(image, window.clone(), None);

        Ok(())
    }

    pub fn copy(&self) -> anyhow::Result<()> {
        let clipboard = self.clipboard();

        let content_provider = self
            .current_page()
            .context("No current page")?
            .content_provider();
        clipboard.set_content(content_provider.as_ref())?;

        Ok(())
    }
}
