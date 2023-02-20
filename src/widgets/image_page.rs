// image_page.rs
//
// Copyright 2022 Christopher Davis <christopherdavis@gnome.org>
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
use crate::widgets::LpImage;

use adw::subclass::prelude::*;
use glib::clone;
use gtk::prelude::*;
use gtk::CompositeTemplate;
use gtk_macros::spawn;
use once_cell::sync::Lazy;

use std::cell::RefCell;
use std::path::{Path, PathBuf};

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(file = "../../data/gtk/image_page.ui")]
    pub struct LpImagePage {
        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) spinner: TemplateChild<gtk::Spinner>,
        #[template_child]
        pub(super) error_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub(super) scrolled_window: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub(super) image: TemplateChild<LpImage>,
        #[template_child]
        pub(super) popover: TemplateChild<gtk::PopoverMenu>,
        #[template_child]
        pub(super) right_click_gesture: TemplateChild<gtk::GestureClick>,
        #[template_child]
        pub(super) press_gesture: TemplateChild<gtk::GestureLongPress>,

        /// set if an error has occurred and is shown on error_page
        pub(super) error: RefCell<Option<String>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpImagePage {
        const NAME: &'static str = "LpImagePage";
        type Type = super::LpImagePage;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for LpImagePage {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecObject::builder::<LpImage>("image")
                        .read_only()
                        .build(),
                    glib::ParamSpecString::builder("error").read_only().build(),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = self.obj();
            match pspec.name() {
                "image" => obj.image().to_value(),
                "error" => obj.error().to_value(),
                name => unimplemented!("property {name}"),
            }
        }

        fn constructed(&self) {
            let obj = self.obj();

            self.parent_constructed();

            self.right_click_gesture
                .connect_pressed(clone!(@weak obj => move |gesture, _, x, y| {
                    obj.show_popover_at(x, y);
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                }));

            self.press_gesture
                .connect_pressed(clone!(@weak obj => move |gesture, x, y| {
                    log::debug!("Long press triggered");
                    obj.show_popover_at(x, y);
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                }));
        }
    }

    impl WidgetImpl for LpImagePage {}
    impl BinImpl for LpImagePage {}
}

glib::wrapper! {
    pub struct LpImagePage(ObjectSubclass<imp::LpImagePage>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl LpImagePage {
    pub fn from_path(path: &Path) -> Self {
        let obj = glib::Object::new::<Self>();
        let path = path.to_path_buf();
        let file = gio::File::for_path(&path);

        obj.imp().image.set_file(&file);

        // This doesn't work properly for items not explicitly selected
        // via the file chooser portal. I'm not sure how to make this work.
        gtk::RecentManager::default().add_item(&file.uri());

        spawn!(clone!(@weak obj, @strong path => async move {
            let imp = obj.imp();
            match imp.image.load(&path).await {
                Ok(()) => {
                    imp.stack.set_visible_child(&*imp.scrolled_window);
                    imp.spinner.set_spinning(false);
                },
                Err(e) => {
                    imp.error.replace(Some(e.to_string()));
                    imp.stack.set_visible_child(&*imp.error_page);
                    obj.notify("error");
                    imp.spinner.set_spinning(false);
                    log::error!("Could not load image: {e}");
                }
            }
        }));

        obj
    }

    pub fn path(&self) -> PathBuf {
        self.image().path().unwrap()
    }

    pub fn image(&self) -> LpImage {
        self.imp().image.get()
    }

    pub fn texture(&self) -> Option<gdk::Texture> {
        self.imp().image.texture()
    }

    pub fn error(&self) -> Option<String> {
        self.imp().error.borrow().clone()
    }

    pub fn content_provider(&self) -> Option<gdk::ContentProvider> {
        self.imp().image.content_provider()
    }

    pub fn show_popover_at(&self, x: f64, y: f64) {
        let imp = self.imp();

        let rect = gdk::Rectangle::new(x as i32, y as i32, 0, 0);

        imp.popover.set_pointing_to(Some(&rect));
        imp.popover.popup();
    }
}
