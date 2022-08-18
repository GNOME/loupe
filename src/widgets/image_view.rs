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

use adw::subclass::prelude::*;
use glib::clone;
use gtk::prelude::*;
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
        #[template_child]
        pub carousel: TemplateChild<adw::Carousel>,

        pub model: RefCell<Option<LpFileModel>>,
        pub current_model_index: Cell<u32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpImageView {
        const NAME: &'static str = "LpImageView";
        type Type = super::LpImageView;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::Type::bind_template_callbacks(klass);

            klass.install_action("iv.next", None, move |image_view, _, _| {
                image_view.navigate(adw::NavigationDirection::Forward);
            });

            klass.install_action("iv.previous", None, move |image_view, _, _| {
                image_view.navigate(adw::NavigationDirection::Back);
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for LpImageView {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecObject::new(
                    "active-file",
                    "",
                    "",
                    gio::File::static_type(),
                    glib::ParamFlags::READABLE,
                )]
            });

            PROPERTIES.as_ref()
        }

        fn property(&self, obj: &Self::Type, _id: usize, psec: &glib::ParamSpec) -> glib::Value {
            match psec.name() {
                "active-file" => obj.active_file().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            let source = gtk::DragSource::new();
            source.set_exclusive(true);

            source.connect_prepare(
                glib::clone!(@weak obj => @default-return None, move |_, _, _| {
                    obj.current_page().map(|p| p.content_provider())
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
                }));
        }
    }

    impl WidgetImpl for LpImageView {}
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

        self.build_model_from_file(file);
        self.notify("active-file");

        // TODO: rework width stuff
        // let width = imp.picture.image_width();
        // let height = imp.picture.image_height();

        // log::debug!("Image dimensions: {} x {}", width, height);
        // Ok((width, height))
        Ok(())
    }

    // Builds an `LpFileModel`, which is an implementation of `gio::ListModel`
    // that holds a `gio::File` for each child within the same directory as the
    // file we pass. This model will update with changes to the directory,
    // and in turn we'll update our `adw::Carousel`.
    fn build_model_from_file(&self, file: &gio::File) {
        let imp = self.imp();
        let carousel = &imp.carousel;

        {
            // Here we use a nested scope so that the mutable borrow only lasts as long as we need it
            let mut model = imp.model.borrow_mut();

            if let Some(ref parent) = file.parent() {
                if let Some(ref m) = *model {
                    if m.directory().map_or(false, |f| !f.equal(parent)) {
                        // Clear the carousel before creating the new model
                        self.clear_carousel(false);
                        *model = Some(LpFileModel::from_directory(parent));
                        log::debug!("new model created");
                    } else {
                        log::debug!("Re-using old model and navigating to the current file");
                        self.navigate_to_file(m, file);
                        return;
                    }
                } else {
                    *model = Some(LpFileModel::from_directory(parent));
                    log::debug!("new model created");
                }
            }
        }

        if let Some(model) = imp.model.borrow().as_ref() {
            let index = model.index_of(file).unwrap();
            log::debug!("Currently at file {index} in the directory");
            carousel.append(&LpImagePage::from_file(file));
            self.fill_carousel(model, index);
            self.update_action_state(model, index);
        }
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

    fn navigate_to_file(&self, model: &LpFileModel, file: &gio::File) {
        let imp = self.imp();
        let carousel = imp.carousel.get();
        let current_index = imp.current_model_index.get();
        let new_index = model.index_of(file).unwrap_or_default();

        if new_index == current_index {
            return;
        }

        let guard = carousel.freeze_notify();
        let page = LpImagePage::from_file(file);

        if new_index > current_index {
            carousel.append(&page);
        } else {
            carousel.prepend(&page);
        }

        carousel.scroll_to(&page, true);

        // Clear everything on either side, then refill
        self.clear_carousel(true);
        self.fill_carousel(model, new_index);
        self.update_action_state(model, new_index);

        drop(guard);
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

        imp.current_model_index.set(index);
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
    }

    pub fn update_action_state(&self, model: &LpFileModel, index: u32) {
        let next_enabled = model.item(index + 1).is_some();
        let prev_enabled = index.checked_sub(1).and_then(|i| model.item(i)).is_some();

        self.action_set_enabled("iv.next", next_enabled);
        self.action_set_enabled("iv.previous", prev_enabled);
    }

    #[template_callback]
    fn page_changed_cb(&self, _index: u32, carousel: &adw::Carousel) {
        let imp = self.imp();
        let b = imp.model.borrow();
        let model = b.as_ref().unwrap();
        let current = self.active_file().unwrap();

        let model_index = model.index_of(&current).unwrap();
        let prev_index = imp.current_model_index.get();

        if model_index != prev_index {
            self.update_action_state(model, model_index);

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

    pub fn set_background(&self) -> anyhow::Result<()> {
        let background = self.uri().context("No URI for current file")?;
        spawn!(clone!(@weak self as view => async move {
            let id = WindowIdentifier::from_native(
                &view.native().expect("View should have a GtkNative"),
            )
            .await;

            let _ = match wallpaper::set_from_uri(
                &id,
                &background,
                true,
                wallpaper::SetOn::Background,
            )
            .await {
                // We use `1` here because we can't pass enums directly as GVariants,
                // so we need to use the C int value of the enum.
                // `TOAST_PRIORITY_NORMAL = 0`, and `TOAST_PRIORITY_HIGH = 1`
                Ok(_) => view.activate_action("win.show-toast", Some(&(i18n("Set as background."), 1).to_variant())).unwrap(),
                Err(err) => {
                    if !matches!(err, Error::Response(ResponseError::Cancelled)) {
                        view.activate_action("win.show-toast", Some(&(i18n("Could not set background."), 1).to_variant())).unwrap();
                    }
                },
            };
        }));

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

    pub fn current_page(&self) -> Option<LpImagePage> {
        let carousel = &self.imp().carousel;
        let pos = carousel.position().round() as u32;
        if carousel.n_pages() > 0 {
            carousel.nth_page(pos).downcast().ok()
        } else {
            None
        }
    }

    pub fn uri(&self) -> Option<String> {
        self.active_file().map(|f| f.uri().to_string())
    }

    pub fn active_file(&self) -> Option<gio::File> {
        let page = self.current_page()?;
        page.file()
    }
}
