// Copyright (c) 2022-2023 Sophie Herold
// Copyright (c) 2022 Christopher Davis
// Copyright (c) 2023 Julian Hofer
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

//! A widget that shows the image, a loading screen, or an error page
//!
//! This widget also handles showing the context menu.

use crate::deps::*;
use crate::widgets::LpImage;

use adw::subclass::prelude::*;
use glib::{clone, Properties};
use gtk::prelude::*;
use gtk::CompositeTemplate;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate, Properties)]
    #[properties(wrapper_type = super::LpImagePage)]
    #[template(file = "image_page.ui")]
    pub struct LpImagePage {
        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) spinner_page: TemplateChild<gtk::Widget>,
        #[template_child]
        pub(super) error_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub(super) image_stack_page: TemplateChild<gtk::Widget>,
        #[template_child]
        pub(super) spinner: TemplateChild<gtk::Spinner>,
        #[template_child]
        pub(super) scrolled_window: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        #[property(type = LpImage, get = Self::image)]
        pub(super) image: TemplateChild<LpImage>,
        #[template_child]
        pub(super) popover: TemplateChild<gtk::PopoverMenu>,
        #[template_child]
        pub(super) right_click_gesture: TemplateChild<gtk::GestureClick>,
        #[template_child]
        pub(super) press_gesture: TemplateChild<gtk::GestureLongPress>,
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
            Self::derived_properties()
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
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

            self.stack
                .connect_visible_child_notify(clone!(@weak obj => move |stack| {
                    let imp = obj.imp();
                    let spinner_visible =
                        stack.visible_child().as_ref() == Some(imp.spinner_page.upcast_ref());
                    // Hide spinner for crossfading into other content since it does not look great
                    imp.spinner.set_visible(spinner_visible);
                }));

            obj.image()
                .connect_is_loaded_notify(clone!(@weak obj => move |_| {
                    if obj.image().is_loaded() {
                        log::debug!("Showing image");
                        obj.imp()
                            .stack
                            .set_visible_child(&*obj.imp().image_stack_page);
                    }
                }));

            obj.image()
                .connect_error_notify(clone!(@weak obj => move |_| {
                    if obj.image().error().is_some() {
                        obj.imp().stack.set_visible_child(&*obj.imp().error_page);
                    }
                }));

            // Do not waste CPU on spinner if it is not visible
            self.spinner.connect_map(|s| s.start());
            self.spinner.connect_unmap(|s| s.stop());
        }
    }

    impl WidgetImpl for LpImagePage {}
    impl BinImpl for LpImagePage {}

    impl LpImagePage {
        pub fn image(&self) -> LpImage {
            self.image.get()
        }
    }
}

glib::wrapper! {
    pub struct LpImagePage(ObjectSubclass<imp::LpImagePage>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl LpImagePage {
    pub fn from_file(file: &gio::File) -> Self {
        let obj = glib::Object::new::<Self>();

        obj.imp().image.set_file(file);

        glib::spawn_future_local(clone!(@weak obj, @strong file => async move {
            let imp = obj.imp();
            imp.image.load(&file).await;
        }));

        obj
    }

    pub fn file(&self) -> gio::File {
        self.image().file().unwrap()
    }

    pub fn scrolled_window(&self) -> gtk::ScrolledWindow {
        self.imp().scrolled_window.clone()
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
