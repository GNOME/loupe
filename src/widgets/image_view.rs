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

use adw::subclass::prelude::*;
use glib::clone;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;

use anyhow::Context;
use ashpd::desktop::wallpaper;
use ashpd::WindowIdentifier;
use once_cell::sync::Lazy;
use std::cell::RefCell;

use crate::file_model::LpFileModel;
use crate::thumbnail::Thumbnail;
use crate::util;
use crate::widgets::LpImage;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/org/gnome/Loupe/gtk/image_view.ui")]
    pub struct LpImageView {
        #[template_child]
        pub picture: TemplateChild<LpImage>,
        #[template_child]
        pub controls: TemplateChild<gtk::Box>,
        #[template_child]
        pub popover: TemplateChild<gtk::PopoverMenu>,
        #[template_child]
        pub click_gesture: TemplateChild<gtk::GestureClick>,
        #[template_child]
        pub press_gesture: TemplateChild<gtk::GestureLongPress>,

        // RefCell<T> allows for interior mutability of non-primitive types.
        pub menu_model: RefCell<Option<gio::MenuModel>>,
        pub popover_menu_model: RefCell<Option<gio::MenuModel>>,

        pub model: RefCell<Option<LpFileModel>>,
        pub current_file: RefCell<Option<gio::File>>,
        pub uri: RefCell<Option<String>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpImageView {
        const NAME: &'static str = "LpImageView";
        type Type = super::LpImageView;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);

            klass.install_action("iv.next", None, move |image_view, _, _| {
                image_view.next();
            });

            klass.install_action("iv.previous", None, move |image_view, _, _| {
                image_view.previous();
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for LpImageView {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecString::new(
                        "filename",
                        "Filename",
                        "The filename of the current file",
                        None,
                        glib::ParamFlags::READABLE,
                    ),
                    glib::ParamSpecObject::new(
                        "controls",
                        "Controls",
                        "The controls for the image view",
                        gtk::Box::static_type(),
                        glib::ParamFlags::READABLE,
                    ),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn property(&self, obj: &Self::Type, _id: usize, psec: &glib::ParamSpec) -> glib::Value {
            match psec.name() {
                "filename" => obj.filename().to_value(),
                "controls" => self.controls.to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            self.click_gesture
                .connect_pressed(clone!(@weak obj => move |_, _, x, y| {
                    obj.show_popover_at(x, y);
                }));

            self.press_gesture
                .connect_pressed(clone!(@weak obj => move |_, x, y| {
                    obj.show_popover_at(x, y);
                }));

            let source = gtk::DragSource::new();

            source.connect_prepare(
                glib::clone!(@weak obj => @default-return None, move |_, _, _| {
                    match obj.imp().picture.content_provider() {
                        Ok(content) => Some(content),
                        Err(e) => {
                            log::error!("Could not get content provider: {:?}", e);
                            None
                        }
                    }
                }),
            );

            source.connect_drag_begin(glib::clone!(@weak obj => move |source, _| {
                let imp = obj.imp();
                if let Some(texture) = imp.picture.texture() {
                    let thumbnail = Thumbnail::new(&texture);
                    source.set_icon(Some(&thumbnail), 0, 0);
                };
            }));

            obj.add_controller(&source);
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

impl LpImageView {
    pub fn set_image_from_file(&self, file: &gio::File) -> anyhow::Result<(i32, i32)> {
        let imp = self.imp();

        if let Some(current_file) = imp.picture.file() {
            if current_file.path().as_deref() == file.path().as_deref() {
                return Err(anyhow::Error::msg(
                    "Image is the same as the previous image; Doing nothing.",
                ));
            }
        }

        imp.current_file.replace(Some(file.clone()));
        self.set_parent_from_file(file);
        self.update_image(file);
        self.notify("filename");

        let width = imp.picture.image_width();
        let height = imp.picture.image_height();

        log::debug!("Image dimensions: {} x {}", width, height);
        Ok((width, height))
    }

    fn set_parent_from_file(&self, file: &gio::File) {
        let imp = self.imp();
        let mut model = imp.model.borrow_mut();

        if let Some(ref parent) = file.parent() {
            if let Some(ref m) = *model {
                if m.directory().map_or(false, |f| !f.equal(parent)) {
                    *model = Some(LpFileModel::from_directory(parent));
                    log::debug!("new model created");
                }
            } else {
                *model = Some(LpFileModel::from_directory(parent));
                log::debug!("new model created");
            }
        }
    }

    fn update_image(&self, file: &gio::File) {
        let imp = self.imp();

        imp.picture.set_file(file);
        imp.uri.replace(Some(file.uri().to_string()));
        self.update_action_state(file);
    }

    pub fn next(&self) {
        {
            let imp = self.imp();
            let b = imp.model.borrow();
            let model = b.as_ref().unwrap();
            let mut current_file = imp.current_file.borrow_mut();

            if let Some(file) = current_file.as_mut() {
                *file = model.next(file).unwrap();
                self.update_image(file);
            }
        }

        self.notify("filename");
    }

    pub fn previous(&self) {
        {
            let imp = self.imp();
            let b = imp.model.borrow();
            let model = b.as_ref().unwrap();
            let mut current_file = imp.current_file.borrow_mut();

            if let Some(file) = current_file.as_mut() {
                *file = model.previous(file).unwrap();
                self.update_image(file);
            }
        }

        self.notify("filename");
    }

    pub fn update_action_state(&self, file: &gio::File) {
        let imp = self.imp();
        let b = imp.model.borrow();
        let model = b.as_ref().unwrap();

        let next_enabled = model.next(file).is_some();
        let prev_enabled = model.previous(file).is_some();

        self.action_set_enabled("iv.next", next_enabled);
        self.action_set_enabled("iv.previous", prev_enabled);
    }

    pub fn set_wallpaper(&self) -> anyhow::Result<()> {
        let wallpaper = self.uri().context("No URI for current file")?;
        let ctx = glib::MainContext::default();
        ctx.spawn_local(clone!(@weak self as view => async move {
            let id = WindowIdentifier::from_native(
                &view.native().expect("View should have a GtkNative"),
            )
            .await;

            let status = match wallpaper::set_from_uri(
                &id,
                &wallpaper,
                true,
                wallpaper::SetOn::Background,
            )
            .await {
                Ok(_) => "Set as wallpaper.",
                Err(_) => "Could not set wallpaper.",
            };

            view.activate_action(
                "win.show-toast",
                // We use `1` here because we can't pass enums directly as GVariants,
                // so we need to use the C int value of the enum.
                // `TOAST_PRIORITY_NORMAL = 0`, and `TOAST_PRIORITY_HIGH = 1`
                Some(&(status, 1).to_variant()),
            )
            .unwrap();
        }));

        Ok(())
    }

    pub fn print(&self) -> anyhow::Result<()> {
        let imp = self.imp();

        let operation = gtk::PrintOperation::new();
        let path = (imp.current_file.borrow().as_ref())
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
        let imp = self.imp();

        if let Some(texture) = imp.picture.texture() {
            clipboard.set_texture(&texture);
        } else {
            anyhow::bail!("No Image displayed.");
        }

        Ok(())
    }

    pub fn uri(&self) -> Option<String> {
        let imp = self.imp();
        imp.uri.borrow().to_owned()
    }

    pub fn filename(&self) -> Option<String> {
        let imp = self.imp();
        let b = imp.current_file.borrow();
        let file = b.as_ref()?;
        util::get_file_display_name(file)
    }

    pub fn show_popover_at(&self, x: f64, y: f64) {
        let imp = self.imp();

        let rect = gdk::Rectangle::new(x as i32, y as i32, 0, 0);

        imp.popover.set_pointing_to(Some(&rect));
        imp.popover.popup();
    }
}
