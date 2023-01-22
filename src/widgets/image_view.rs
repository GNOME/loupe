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
use crate::i18n::*;

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::clone;
use gtk::CompositeTemplate;

use anyhow::{bail, Context};
use ashpd::desktop::wallpaper;
use ashpd::desktop::ResponseError;
use ashpd::Error;
use ashpd::WindowIdentifier;
use once_cell::sync::Lazy;
use std::cell::{Cell, RefCell};

use gtk_macros::spawn;

use crate::file_model::LpFileModel;
use crate::thumbnail::Thumbnail;
use crate::widgets::LpImagePage;

// The number of pages we want to buffer
// on either side of the current page.
const BUFFER: u32 = 2;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/org/gnome/Loupe/gtk/image_view.ui")]
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
        pub carousel: TemplateChild<adw::Carousel>,

        pub model: RefCell<Option<LpFileModel>>,
        pub current_model_index: Cell<u32>,

        pub scrolling: Cell<bool>,

        pub current_page_strict: RefCell<Option<LpImagePage>>,
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
                    glib::ParamSpecObject::builder::<gio::File>("active-file")
                        .read_only()
                        .build(),
                    glib::ParamSpecObject::builder::<LpImagePage>("current-page-strict")
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
                "active-file" => obj.active_file().to_value(),
                "current-page-strict" => obj.current_page_strict().to_value(),
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

            self.carousel
                .connect_position_notify(clone!(@weak obj => move |_| {
                    obj.notify("active-file");
                    obj.notify("current-page-strict");
                }));
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

            if let Some(page) = self.instance().current_page_strict() {
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
    pub fn set_image_from_file(&self, file: &gio::File) -> anyhow::Result<()> {
        if let Some(current_file) = self.active_file() {
            if current_file.equal(file) {
                bail!("Image is the same as the previous image; Doing nothing.");
            }
        }

        self.build_model_from_file(file)?;
        self.notify("active-file");

        Ok(())
    }

    // Builds an `LpFileModel`, which is an implementation of `gio::ListModel`
    // that holds a `gio::File` for each child within the same directory as the
    // file we pass. This model will update with changes to the directory,
    // and in turn we'll update our `adw::Carousel`.
    fn build_model_from_file(&self, file: &gio::File) -> anyhow::Result<()> {
        let imp = self.imp();
        let carousel = &imp.carousel;

        let model = {
            // Here we use a nested scope so that the mutable borrow only lasts as long as we need it

            if imp.model.borrow().is_some() {
                let is_same_directory = !file.parent().map_or(false, |p| {
                    imp.model
                        .borrow()
                        .as_ref()
                        .and_then(|x| x.directory())
                        .map_or(false, |f| !f.equal(&p))
                });

                if is_same_directory {
                    log::debug!("Re-using old model and navigating to the current file");
                    self.navigate_to_file(file);
                    return Ok(());
                } else {
                    // Clear the carousel before creating the new model
                    self.clear_carousel(false);
                    let model = LpFileModel::from_file(file)?;
                    imp.model.replace(Some(model.clone()));
                    log::debug!("new model created");
                    model
                }
            } else {
                let model = LpFileModel::from_file(file)?;
                imp.model.replace(Some(model.clone()));
                log::debug!("first model created");
                model
            }
        };

        let page = LpImagePage::from_file(file);
        imp.current_page_strict.replace(Some(page.clone()));
        self.notify("current-page-strict");
        self.notify("is-previous-available");
        self.notify("is-next-available");

        carousel.append(&page);

        spawn!(glib::clone!(@weak self as obj, @strong file => async move {
            if let Err(err) = model.load_directory().await {
                log::warn!("Failed to load directory: {:?}", err);
                obj.activate_action("win.show-toast", Some(&(err.to_string(), 1).to_variant()))
                    .unwrap();
                return;
            }

            let index = model
                .index_of(&file)
                .context("File not found in model.")
                .unwrap();
            log::debug!("Currently at file {index} in the directory");

            obj.fill_carousel(&model, index);
        }));

        Ok(())
    }

    pub fn navigate(&self, direction: adw::NavigationDirection) {
        let carousel = &self.imp().carousel;
        let pos = carousel.position().round() as u32;
        match direction {
            adw::NavigationDirection::Forward => {
                if pos < carousel.n_pages() - 1 {
                    carousel.scroll_to(&carousel.nth_page(pos + 1), true);
                }
            }
            adw::NavigationDirection::Back => {
                if pos > 0 {
                    carousel.scroll_to(&carousel.nth_page(pos - 1), true)
                }
            }
            _ => unimplemented!("Navigation direction should only be back or forward."),
        };
    }

    fn navigate_to_file(&self, file: &gio::File) {
        let imp = self.imp();

        let Some(new_index) = imp.model.borrow().as_ref().and_then(|model| model.index_of(file)) else {
            log::warn!("Could not navigate to file {:?}", file.path());
            return;
        };

        let carousel = imp.carousel.get();
        let current_index = imp.current_model_index.get();

        if new_index == current_index {
            return;
        }

        let page = LpImagePage::from_file(file);

        if new_index > current_index {
            carousel.append(&page);
        } else {
            carousel.prepend(&page);
        }

        // Set a flag and scroll to the page. Our `page-changed` signal handler
        // will handle clearing and refilling the carousel. This is so we're
        // not changing the state of the carousel while it's scrolling to the
        // new page.
        log::debug!("Scrolling to page for {new_index}");
        imp.scrolling.set(true);
        carousel.scroll_to(&page, true);

        self.notify("is-previous-available");
        self.notify("is-next-available");
    }

    // Fills the carousel with items on either side of the given `index` of `model`
    fn fill_carousel(&self, model: &LpFileModel, index: u32) {
        let imp = self.imp();
        let carousel = imp.carousel.get();

        for i in 1..=BUFFER {
            if let Some(ref file) = model.file(index + i) {
                carousel.append(&LpImagePage::from_file(file))
            }
        }

        for i in 1..=BUFFER {
            if let Some(ref file) = index.checked_sub(i).and_then(|i| model.file(i)) {
                carousel.prepend(&LpImagePage::from_file(file))
            }
        }

        log::debug!("Carousel filled, current file at index {index}");
        imp.current_model_index.set(index);

        self.notify("is-previous-available");
        self.notify("is-next-available");
    }

    // Clear the carousel, optionally preserving the current position
    // as a point to refill from
    fn clear_carousel(&self, preserve_current_page: bool) {
        let carousel = self.imp().carousel.get();

        if preserve_current_page {
            // Remove everything before the current page
            for _ in 0..(carousel.position() as u32) {
                carousel.remove(&carousel.nth_page(0));
            }

            // Then everything after
            while carousel.n_pages() > 1 {
                carousel.remove(&carousel.nth_page(carousel.n_pages() - 1));
            }
        } else {
            while carousel.n_pages() > 0 {
                carousel.remove(&carousel.nth_page(0));
            }
        }

        self.notify("is-previous-available");
        self.notify("is-next-available");
    }

    /// Returns `true` if there is an image before the current one
    pub fn is_previous_available(&self) -> bool {
        let imp = self.imp();
        if let Some(model) = imp.model.borrow().as_ref() {
            imp.current_model_index
                .get()
                .checked_sub(1)
                .and_then(|i| model.item(i))
                .is_some()
        } else {
            false
        }
    }

    /// Returns `true` if there is an image after the current one
    pub fn is_next_available(&self) -> bool {
        let imp = self.imp();
        if let Some(model) = imp.model.borrow().as_ref() {
            model.item(imp.current_model_index.get() + 1).is_some()
        } else {
            false
        }
    }

    #[template_callback]
    fn page_changed_cb(&self, index: u32, carousel: &adw::Carousel) {
        let imp = self.imp();

        imp.current_page_strict
            .replace(carousel.nth_page(index).downcast().ok());
        self.notify("current-page-strict");

        let b = imp.model.borrow();
        let model = b.as_ref().unwrap();
        let current = self.active_file().unwrap();

        let model_index = model.index_of(&current).unwrap();
        let prev_index = imp.current_model_index.get();

        if model_index != prev_index {
            if imp.scrolling.get() {
                log::debug!("Scrolling finished, refilling carousel");
                imp.scrolling.set(false);
                // We need to clear the carousel (excluding the current page)
                // and refill the page buffer.
                self.clear_carousel(true);
                self.fill_carousel(model, model_index);
            } else {
                self.update_page_buffer(model, carousel, model_index, prev_index);
            }
        }
    }

    fn update_page_buffer(
        &self,
        model: &LpFileModel,
        carousel: &adw::Carousel,
        model_index: u32,
        prev_index: u32,
    ) {
        let imp = self.imp();

        // We've moved forward
        if let Some(diff) = model_index.checked_sub(prev_index) {
            for i in 0..diff {
                if prev_index
                    .checked_sub(BUFFER)
                    .and_then(|r| r.checked_sub(i))
                    .is_some()
                {
                    carousel.remove(&carousel.nth_page(0));
                }

                let s = prev_index + BUFFER + i + 1;
                if s <= model.n_items() {
                    if let Some(ref file) = model.file(s) {
                        carousel.append(&LpImagePage::from_file(file));
                    }
                }
            }
        }

        // We've moved backward
        if let Some(diff) = prev_index.checked_sub(model_index) {
            for i in 0..diff {
                let s = prev_index + BUFFER + i + 1;
                if s <= model.n_items() {
                    carousel.remove(&carousel.nth_page(carousel.n_pages() - 1));
                }

                if let Some(ref file) = prev_index
                    .checked_sub(BUFFER)
                    .and_then(|d| d.checked_sub(i + 1))
                    .and_then(|d| model.file(d))
                {
                    carousel.prepend(&LpImagePage::from_file(file));
                }
            }
        }

        imp.current_model_index.set(model_index);
        self.notify("is-previous-available");
        self.notify("is-next-available");
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
        let background = self.uri().context("No URI for current file")?;
        let native = self.native().expect("View should have a GtkNative");
        let id = WindowIdentifier::from_native(&native).await;

        match wallpaper::WallpaperRequest::default()
            .set_on(wallpaper::SetOn::Background)
            .show_preview(true)
            .identifier(id)
            .build_uri(&background)
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
        let path = self
            .active_file()
            .context("No file")?
            .peek_path()
            .context("No path")?;
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

        if let Some(texture) = self.current_page().context("No current page")?.texture() {
            clipboard.set_texture(&texture);
        } else {
            anyhow::bail!("No Image displayed.");
        }

        Ok(())
    }

    // TODO: check how often we actually want to use this
    pub fn current_page(&self) -> Option<LpImagePage> {
        let carousel = &self.imp().carousel;
        let pos = carousel.position().round() as u32;
        if carousel.n_pages() > 0 && pos < carousel.n_pages() {
            carousel.nth_page(pos).downcast().ok()
        } else {
            None
        }
    }

    /// Returns `None` during animations instead of returning the closest image
    pub fn current_page_strict(&self) -> Option<LpImagePage> {
        self.imp().current_page_strict.borrow().clone()
    }

    pub fn uri(&self) -> Option<url::Url> {
        self.active_file()
            .and_then(|f| url::Url::parse(&f.uri()).ok())
    }

    pub fn active_file(&self) -> Option<gio::File> {
        let page = self.current_page()?;
        page.file()
    }
}
