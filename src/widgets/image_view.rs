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

use gdk::prelude::*;
use glib::clone;
use glib::subclass::prelude::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::subclass::widget::WidgetImplExt;
use gtk::CompositeTemplate;
use libadwaita::subclass::prelude::*;

use anyhow::Context;
use ashpd::desktop::wallpaper;
use ashpd::WindowIdentifier;
use once_cell::sync::Lazy;
use std::cell::{Cell, RefCell};

mod imp {
    use super::*;
    use glib::subclass;

    #[derive(Debug, CompositeTemplate)]
    #[template(resource = "/org/gnome/ImageViewer/gtk/image_view.ui")]
    pub struct IvImageView {
        #[template_child]
        pub headerbar: TemplateChild<gtk::HeaderBar>,
        #[template_child]
        pub menu_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub picture: TemplateChild<gtk::Picture>,
        #[template_child]
        pub popover: TemplateChild<gtk::PopoverMenu>,
        #[template_child]
        pub click_gesture: TemplateChild<gtk::GestureClick>,
        #[template_child]
        pub press_gesture: TemplateChild<gtk::GestureLongPress>,

        pub connection: zbus::Connection,
        // Cell<T> allows for interior mutability of primitive types.
        pub header_visible: Cell<bool>,
        // RefCell<T> does the same for non-primitive types.
        pub menu_model: RefCell<Option<gio::MenuModel>>,
        pub popover_menu_model: RefCell<Option<gio::MenuModel>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for IvImageView {
        const NAME: &'static str = "IvImageView";
        type Type = super::IvImageView;
        type ParentType = libadwaita::Bin;

        fn new() -> Self {
            let connection =
                zbus::Connection::new_session().expect("Could not create zbus session");

            Self {
                headerbar: TemplateChild::default(),
                menu_button: TemplateChild::default(),
                picture: TemplateChild::default(),
                popover: TemplateChild::default(),
                click_gesture: TemplateChild::default(),
                press_gesture: TemplateChild::default(),
                connection,
                header_visible: Cell::new(false),
                menu_model: RefCell::default(),
                popover_menu_model: RefCell::default(),
            }
        }

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for IvImageView {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpec::new_boolean(
                        "header-visible",
                        "Header visible",
                        "Whether or not the headerbar is visible",
                        false,
                        glib::ParamFlags::READWRITE,
                    ),
                    glib::ParamSpec::new_object(
                        "primary-menu-model",
                        "Primary Menu Model",
                        "The menu model for the menu button",
                        gio::MenuModel::static_type(),
                        glib::ParamFlags::READWRITE,
                    ),
                    glib::ParamSpec::new_object(
                        "popover-menu-model",
                        "Popover Menu Model",
                        "The menu model for the menu button",
                        gio::MenuModel::static_type(),
                        glib::ParamFlags::READWRITE,
                    ),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: Lazy<Vec<glib::subclass::Signal>> = Lazy::new(|| {
                vec![subclass::Signal::builder(
                    // Called when the dimensions of the image
                    // have been found.
                    "dimensions-loaded",
                    &[
                        // The width of the image
                        i32::static_type().into(),
                        // The height of the image
                        i32::static_type().into(),
                    ],
                    glib::Type::UNIT.into(),
                )
                .build()]
            });

            SIGNALS.as_ref()
        }

        fn property(&self, _obj: &Self::Type, _id: usize, psec: &glib::ParamSpec) -> glib::Value {
            match psec.name() {
                "header-visible" => self.header_visible.get().to_value(),
                "primary-menu-model" => self.menu_model.borrow().to_value(),
                "popover-menu-model" => self.popover_menu_model.borrow().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(
            &self,
            _obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            psec: &glib::ParamSpec,
        ) {
            match psec.name() {
                "header-visible" => {
                    self.header_visible.set(value.get::<bool>().unwrap());
                }
                "primary-menu-model" => {
                    let model: Option<gio::MenuModel> = value.get().unwrap();
                    self.menu_button.set_menu_model(model.as_ref());
                    *self.menu_model.borrow_mut() = model;
                }
                "popover-menu-model" => {
                    let model: Option<gio::MenuModel> = value.get().unwrap();
                    self.popover.set_menu_model(model.as_ref());
                    *self.popover_menu_model.borrow_mut() = model;
                }
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
        }
    }

    impl WidgetImpl for IvImageView {
        fn size_allocate(&self, widget: &Self::Type, width: i32, height: i32, baseline: i32) {
            self.parent_size_allocate(widget, width, height, baseline);
            self.popover.present();
        }
    }

    impl BinImpl for IvImageView {}
}

glib::wrapper! {
    pub struct IvImageView(ObjectSubclass<imp::IvImageView>)
        @extends gtk::Widget, libadwaita::Bin,
        @implements gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl IvImageView {
    pub fn set_image_from_file(&self, file: &gio::File) {
        let imp = imp::IvImageView::from_instance(&self);

        imp.picture.set_file(Some(file));

        if let Err(e) = self.load_dimensions_from_file(file) {
            log::error!("Could not load image dimensions: {}", e);
        };
    }

    fn load_dimensions_from_file(&self, file: &gio::File) -> anyhow::Result<()> {
        let pb = file.path().context("No path for current file")?;
        let (_, width, height) =
            gdk_pixbuf::Pixbuf::file_info(&pb.as_path()).context("Could not get file info")?;

        log::debug!("Image dimensions: {} x {}", width, height);
        let _ = self.emit_by_name("dimensions-loaded", &[&width, &height]);

        Ok(())
    }

    pub fn set_wallpaper(&self) -> anyhow::Result<()> {
        let wallpaper = self.uri().context("No URI for current file")?;
        let ctx = glib::MainContext::default();
        ctx.spawn_local(async move {
            set_wallpaper(wallpaper).await;
        });

        Ok(())
    }

    pub fn print(&self) -> anyhow::Result<()> {
        let imp = imp::IvImageView::from_instance(&self);

        let file = imp.picture.file().context("No file to print")?;
        let operation = gtk::PrintOperation::new();
        let path = file.path().context("No path for current file")?;
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

        operation.connect_draw_page(clone!(@weak pb => move |_, ctx, _| {
            let cr = ctx.cairo_context().expect("No cairo context for print context");
            cr.set_source_pixbuf(&pb, 0.0, 0.0);
            cr.paint();
        }));

        log::debug!("Running print operation...");
        let root = self.root().context("Could not get root for widget")?;
        let window = root
            .downcast_ref::<gtk::Window>()
            .context("Could not downcast to GtkWindow")?;
        operation.run(gtk::PrintOperationAction::PrintDialog, Some(window))?;

        Ok(())
    }

    pub fn uri(&self) -> Option<String> {
        let imp = imp::IvImageView::from_instance(&self);
        let file = imp.picture.file()?;
        Some(file.uri().to_string())
    }

    pub fn show_popover_at(&self, x: f64, y: f64) {
        let imp = imp::IvImageView::from_instance(&self);

        let rect = gdk::Rectangle {
            x: x as i32,
            y: y as i32,
            width: 0,
            height: 0,
        };

        imp.popover.set_pointing_to(&rect);
        imp.popover.popup();
    }

    pub fn connect_dimensions_loaded<F: Fn(&Self, i32, i32) + 'static>(
        &self,
        f: F,
    ) -> glib::SignalHandlerId {
        self.connect_local("dimensions-loaded", true, move |values| {
            let view = values[0].get::<Self>().unwrap();
            let width = values[1].get::<i32>().unwrap();
            let height = values[2].get::<i32>().unwrap();

            f(&view, width, height);

            None
        })
        .unwrap()
    }
}

async fn set_wallpaper(uri: String) {
    match wallpaper::set_from_uri(
        &WindowIdentifier::default(),
        &uri,
        false,
        wallpaper::SetOn::Background,
    )
    .await
    {
        Err(e) => log::error!(
            "Failed to set the wallpaper using the freedesktop portal: {}",
            e
        ),
        Ok(_) => (),
    }
}
