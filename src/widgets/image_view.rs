// image_view.rs
//
// Copyright 2020 Christopher Davis <christopherdavis@gnome.org>
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

use crate::deps::*;
use crate::file_model::LpFileModel;
use crate::i18n::*;
use crate::thumbnail::Thumbnail;
use crate::util::{Direction, Position};
use crate::widgets::{LpImage, LpImagePage, LpSlidingView};

use adw::prelude::*;
use adw::subclass::prelude::*;
use anyhow::Context;
use ashpd::desktop::wallpaper;
use ashpd::desktop::ResponseError;
use ashpd::Error;
use ashpd::WindowIdentifier;
use glib::clone;
use gtk::CompositeTemplate;
use gtk_macros::spawn;
use once_cell::sync::Lazy;

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};

// The number of pages we want to buffer
// on either side of the current page.
const BUFFER: usize = 2;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(file = "../../data/gtk/image_view.ui")]
    pub struct LpImageView {
        /// Direct child of this Adw::Bin
        #[template_child]
        pub bin_child: TemplateChild<gtk::Widget>,
        /// overlayed controls
        #[template_child]
        pub controls_box_start: TemplateChild<gtk::Widget>,
        /// overlayed controls
        #[template_child]
        pub controls_box_end: TemplateChild<gtk::Widget>,
        #[template_child]
        pub sliding_view: TemplateChild<LpSlidingView>,

        pub model: RefCell<LpFileModel>,
        pub preserve_content: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpImageView {
        const NAME: &'static str = "LpImageView";
        type Type = super::LpImageView;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::Type::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for LpImageView {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecObject::builder::<LpImagePage>("current-page")
                        .read_only()
                        .build(),
                    glib::ParamSpecBoolean::builder("is-previous-available")
                        .read_only()
                        .build(),
                    glib::ParamSpecBoolean::builder("is-next-available")
                        .read_only()
                        .build(),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, psec: &glib::ParamSpec) -> glib::Value {
            let obj = self.instance();
            match psec.name() {
                "current-page" => obj.current_page().to_value(),
                "is-previous-available" => obj.is_previous_available().to_value(),
                "is-next-available" => obj.is_next_available().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self) {
            let obj = self.instance();

            self.parent_constructed();

            // Manually mange widget layout, see `WidgetImpl` for details
            obj.set_layout_manager(gtk::LayoutManager::NONE);

            obj.property_expression("current-page")
                .chain_property::<LpImagePage>("image")
                .chain_property::<LpImage>("path")
                .watch(
                    glib::Object::NONE,
                    glib::clone!(@weak obj => move || {
                        obj.current_image_path_changed();
                    }),
                );

            obj.property_expression("current-page")
                .chain_property::<LpImagePage>("image")
                .chain_property::<LpImage>("is-deleted")
                .watch(
                    glib::Object::NONE,
                    glib::clone!(@weak obj => move || {
                        obj.current_image_path_changed();
                    }),
                );

            let source = gtk::DragSource::new();
            source.set_exclusive(true);

            source.connect_prepare(
                glib::clone!(@weak obj => @default-return None, move |gesture, _, _| {
                    let is_scrollable = obj
                        .current_page()
                        .map(|p| p.image().is_hscrollable() || p.image().is_vscrollable());

                    // do scrolling if scrollable
                    if is_scrollable == Some(true) {
                        gesture.set_state(gtk::EventSequenceState::Denied);
                        None
                    } else {
                        obj.current_page().and_then(|p| p.content_provider())
                    }
                }),
            );

            source.connect_drag_begin(glib::clone!(@weak obj => move |source, _| {
                if let Some(texture) = obj.current_page().and_then(|p| p.texture()) {
                    let thumbnail = Thumbnail::new(&texture);
                    source.set_icon(Some(&thumbnail), 0, 0);
                };
            }));

            obj.add_controller(&source);
        }
    }

    impl WidgetImpl for LpImageView {
        /// The main target of this manual implemtation is to provide a good
        /// natural width that allows to have a neat size for newly opened `LpWindow`s.
        /// The actual calculation of the image natural width happens in
        /// the measure function of `LpImage`.
        ///
        /// This manual implementation is necessary since AdwCarousel gives the
        /// largest natural width of all of it's children. But we actually want the
        /// one of the opened (current) image.
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            // Measure child of AdwBin
            let (child_min, child_natural, _, _) = self.bin_child.measure(orientation, for_size);

            // measure size of controls overlays
            let (overlay1_min, _, _, _) = self.controls_box_start.measure(orientation, for_size);
            let (overlay2_min, _, _, _) = self.controls_box_end.measure(orientation, for_size);

            // take as minimum whatever is larger
            let min = i32::max(child_min, overlay1_min + overlay2_min);

            if let Some(page) = self.instance().current_page() {
                // measure `LpImage`
                let (_, image_natural, _, _) = page.image().measure(orientation, for_size);

                // Ensure that minimum size is not smaller than the one of the child.
                // Also ensure that the natural width is not smaller than the minimal size.
                // Both things are required by GTK.
                (min, i32::max(min, image_natural), -1, -1)
            } else {
                // also include controls overlays when mostly passing through child measurements
                (min, i32::max(child_natural, min), -1, -1)
            }
        }

        /// this is necessary because we do layout manually
        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.bin_child.allocate(width, height, baseline, None);
        }
    }

    impl BinImpl for LpImageView {}
}

glib::wrapper! {
    pub struct LpImageView(ObjectSubclass<imp::LpImageView>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

#[gtk::template_callbacks]
impl LpImageView {
    pub fn set_image_from_path(&self, path: &Path) {
        if let Err(err) = self.load_path(path) {
            log::error!("Failed to load path: {err}");
            self.activate_action("win.show-toast", Some(&(err.to_string(), 1).to_variant()))
                .unwrap();
        }
    }

    // Builds an `LpFileModel`, which is an implementation of `gio::ListModel`
    // that holds a `gio::File` for each child within the same directory as the
    // file we pass. This model will update with changes to the directory,
    // and in turn we'll update our `adw::Carousel`.
    fn load_path(&self, path: &Path) -> anyhow::Result<()> {
        let sliding_view = self.sliding_view();
        let directory = path.parent().map(|x| x.to_path_buf());

        let is_same_directory = directory == self.model().directory();

        if is_same_directory {
            log::debug!("Re-using old model and navigating to the current file");
            self.navigate_to_path(path);
            return Ok(());
        }

        let model = LpFileModel::from_path(path);
        self.set_model(model);
        log::debug!("New model created");

        let page = LpImagePage::from_path(path);

        sliding_view.clear();
        sliding_view.append(&page);

        // List other files in directory
        if let Some(directory) = directory {
            let path = path.to_path_buf();
            spawn!(glib::clone!(@weak self as obj, @strong path => async move {
                if let Err(err) = obj.model().load_directory(directory).await {
                    log::warn!("Failed to load directory: {err}");
                    obj.activate_action("win.show-toast", Some(&(err.to_string(), 1).to_variant()))
                        .unwrap();
                    return;
                }

                obj.update_sliding_view(&path);
            }));
        }

        Ok(())
    }

    /// Move forward or backwards
    pub fn navigate(&self, direction: Direction) {
        if let Some(current_path) = self.current_path() {
            let new_path = match direction {
                Direction::Forward => self.model().after(&current_path),
                Direction::Back => self.model().before(&current_path),
            };

            if let Some(new_path) = new_path {
                self.scroll_sliding_view(&new_path);
            }
        }
    }

    /// Jump to position
    pub fn jump(&self, position: Position) {
        let new_path = match position {
            Position::First => self.model().first(),
            Position::Last => self.model().last(),
        };

        if let Some(new_path) = new_path {
            self.navigate_to_path(&new_path);
        }
    }

    /// Used for drag and drop
    fn navigate_to_path(&self, new_path: &Path) {
        let sliding_view = self.sliding_view();

        if Some(new_path.to_path_buf()) == self.current_path() {
            return;
        }

        let Some(new_index) = self.model().index_of(new_path) else {
            log::warn!("Could not navigate to file {new_path:?}");
            return;
        };

        let current_index = self
            .current_path()
            .and_then(|x| self.model().index_of(&x))
            .unwrap_or_default();

        let page = if let Some(page) = sliding_view.get(new_path) {
            page
        } else {
            let page = LpImagePage::from_path(new_path);

            if new_index > current_index {
                sliding_view.append(&page);
            } else {
                sliding_view.prepend(&page);
            }

            page
        };

        log::debug!("Scrolling to page for {new_path:?}");
        self.imp().preserve_content.set(true);
        sliding_view.scroll_to(&page);
    }

    /// Ensures the sliding view contains the correct images
    fn update_sliding_view(&self, current_path: &Path) {
        log::debug!("Updating sliding_view neighbors for current path {current_path:?}");
        let sliding_view = self.sliding_view();

        self.imp().preserve_content.set(false);

        let existing = sliding_view.pages();
        let target = self.model().files_around(current_path, BUFFER);

        // remove old pages
        for (path, page) in &existing {
            if !target.contains(path) {
                sliding_view.remove(page);
            }
        }

        // add missing pages or put in correct position
        for (position, path) in target.iter().enumerate() {
            if let Some(page) = existing.get(path) {
                sliding_view.move_to(page, position);
            } else {
                sliding_view.insert(&LpImagePage::from_path(path), position);
            }
        }

        self.notify("is-previous-available");
        self.notify("is-next-available");
    }

    fn scroll_sliding_view(&self, path: &Path) {
        let Some(current_page) = self.sliding_view().pages().remove(path) else {
            log::error!("Current path not availabel in sliding_view for scrolling: {path:?}");
            return;
        };

        self.sliding_view().scroll_to(&current_page);
    }

    fn sliding_view(&self) -> LpSlidingView {
        self.imp().sliding_view.clone()
    }

    fn model(&self) -> LpFileModel {
        self.imp().model.borrow().clone()
    }

    fn set_model(&self, model: LpFileModel) {
        model.connect_changed(
            glib::clone!(@weak self as obj => move || obj.model_content_changed_cb()),
        );
        self.imp().model.replace(model);
    }

    /// Handle files are added or removed from directory
    fn model_content_changed_cb(&self) {
        let Some(current_path) = self.current_path() else { return; };

        // LpImage did not get the update yet
        // Update will be handled by current_image_path_changed
        if !self.model().contains(&current_path) {
            return;
        }

        self.update_sliding_view(&current_path);
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

        if let Some(current_path) = self.current_path() {
            if self.model().contains(&current_path) && !self.imp().preserve_content.get() {
                self.update_sliding_view(&current_path);
            }
        }
    }

    pub fn current_image(&self) -> Option<LpImage> {
        self.imp().sliding_view.current_page().map(|x| x.image())
    }

    pub fn current_page(&self) -> Option<LpImagePage> {
        self.imp().sliding_view.current_page()
    }

    pub fn current_path(&self) -> Option<PathBuf> {
        self.imp().sliding_view.current_page().map(|x| x.path())
    }

    pub fn current_file(&self) -> Option<gio::File> {
        self.imp()
            .sliding_view
            .current_page()
            .and_then(|x| x.image().file())
    }

    /// Returns `true` if there is an image before the current one
    pub fn is_previous_available(&self) -> bool {
        if let Some(path) = self.current_path() {
            self.model().index_of(&path) != Some(0)
        } else {
            false
        }
    }

    /// Returns `true` if there is an image after the current one
    pub fn is_next_available(&self) -> bool {
        if let Some(path) = self.current_path() {
            let model = self.model();
            model.index_of(&path) != Some(model.n_files().saturating_sub(1))
        } else {
            false
        }
    }

    #[template_callback]
    fn page_changed_cb(&self) {
        self.notify("current-page");
        self.notify("is-previous-available");
        self.notify("is-next-available");

        let Some(new_page) = self.current_page() else {
            log::debug!("Page changed but no current page");
            return;
        };

        if !self.imp().preserve_content.get() {
            self.update_sliding_view(&new_page.path());
        }
    }

    #[template_callback]
    fn target_page_reached_cb(&self) {
        if self.imp().preserve_content.get() {
            if let Some(new_page) = self.current_page() {
                self.update_sliding_view(&new_page.path());
            } else {
                log::error!("No LpImagePage");
            }
        }
    }

    #[template_callback]
    fn get_fullscreen_icon(&self, fullscreened: bool) -> &'static str {
        if fullscreened {
            "view-restore-symbolic"
        } else {
            "view-fullscreen-symbolic"
        }
    }

    pub fn zoom_out(&self) {
        if let Some(current_page) = self.current_page() {
            current_page.image().zoom_out();
        }
    }

    pub fn zoom_in(&self) {
        if let Some(current_page) = self.current_page() {
            current_page.image().zoom_in();
        }
    }

    pub fn zoom_to(&self, level: f64) {
        if let Some(current_page) = self.current_page() {
            current_page.image().zoom_to(level);
        }
    }

    pub fn rotate_image(&self, angle: f64) {
        if let Some(current_page) = self.current_page() {
            current_page.image().rotate_by(angle);
        }
    }

    pub async fn set_background(&self) -> anyhow::Result<()> {
        let background = self
            .current_path()
            .and_then(|p| std::fs::File::open(p).ok())
            .context(i18n("Failed to open new background image"))?;
        let native = self.native().expect("View should have a GtkNative");
        let id = WindowIdentifier::from_native(&native).await;

        match wallpaper::WallpaperRequest::default()
            .set_on(wallpaper::SetOn::Background)
            .show_preview(true)
            .identifier(id)
            .build_file(&background)
            .await
        {
            // We use `1` here because we can't pass enums directly as GVariants,
            // so we need to use the C int value of the enum.
            // `TOAST_PRIORITY_NORMAL = 0`, and `TOAST_PRIORITY_HIGH = 1`
            Ok(_) => self
                .activate_action(
                    "win.show-toast",
                    Some(&(i18n("Set as background."), 1).to_variant()),
                )
                .unwrap(),
            Err(err) => {
                if !matches!(err, Error::Response(ResponseError::Cancelled)) {
                    self.activate_action(
                        "win.show-toast",
                        Some(&(i18n("Could not set background."), 1).to_variant()),
                    )
                    .unwrap();
                }
            }
        };

        Ok(())
    }

    pub fn print(&self) -> anyhow::Result<()> {
        let operation = gtk::PrintOperation::new();
        let path = self.current_path().context("No file")?;
        let pb = gdk_pixbuf::Pixbuf::from_file(path)?;

        let setup = gtk::PageSetup::default();
        let page_size = gtk::PaperSize::new(Some(&gtk::PAPER_NAME_A4));
        setup.set_paper_size(&page_size);
        operation.set_default_page_setup(Some(&setup));

        let settings = gtk::PrintSettings::default();
        operation.set_print_settings(Some(&settings));

        operation.connect_begin_print(move |op, _| {
            op.set_n_pages(1);
        });

        // FIXME: Rework all of this after reading https://cairographics.org/manual/cairo-Image-Surfaces.html
        // Since I don't know cairo; See also eog-print.c
        operation.connect_draw_page(clone!(@weak pb => move |_, ctx, _| {
            let cr = ctx.cairo_context();
            cr.set_source_pixbuf(&pb, 0.0, 0.0);
            let _ = cr.paint();
        }));

        log::debug!("Running print operation...");
        let root = self.root().context("Could not get root for widget")?;
        let window = root
            .downcast_ref::<gtk::Window>()
            .context("Could not downcast to GtkWindow")?;
        operation.run(gtk::PrintOperationAction::PrintDialog, Some(window))?;

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
