// Copyright (c) 2022-2025 Sophie Herold
// Copyright (c) 2022, 2024 Christopher Davis
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

use std::time::Duration;

use adw::subclass::prelude::*;
use glib::{Properties, clone};
use gtk::CompositeTemplate;
use gtk::prelude::*;

use super::LpErrorDetails;
use crate::decoder::DecoderError;
use crate::deps::*;
use crate::util::ErrorType;
use crate::util::gettext::*;
use crate::widgets::LpImage;

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
        pub(super) spinner_revealer: TemplateChild<gtk::Revealer>,

        #[template_child]
        pub(super) error_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub(super) error_more_info: TemplateChild<gtk::Button>,

        #[template_child]
        pub(super) image_stack_page: TemplateChild<gtk::Widget>,
        #[template_child]
        pub(super) spinner: TemplateChild<gtk::Widget>,
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

    #[glib::derived_properties]
    impl ObjectImpl for LpImagePage {
        fn constructed(&self) {
            let obj = self.obj();

            self.parent_constructed();

            self.right_click_gesture.connect_pressed(clone!(
                #[weak]
                obj,
                move |gesture, _, x, y| {
                    obj.show_popover_at(x, y);
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                }
            ));

            self.press_gesture.connect_pressed(clone!(
                #[weak]
                obj,
                move |gesture, x, y| {
                    tracing::debug!("Long press triggered");
                    obj.show_popover_at(x, y);
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                }
            ));

            self.stack.connect_visible_child_notify(clone!(
                #[weak]
                obj,
                move |stack| {
                    let imp = obj.imp();
                    let spinner_visible =
                        stack.visible_child().as_ref() == Some(imp.spinner_page.upcast_ref());
                    // Hide spinner for crossfading into other content since it does not look great
                    imp.spinner.set_visible(spinner_visible);
                }
            ));

            obj.image().connect_is_loaded_notify(clone!(
                #[weak]
                obj,
                move |_| {
                    if obj.image().is_loaded() {
                        tracing::debug!("Showing image");
                        obj.imp()
                            .stack
                            .set_visible_child(&*obj.imp().image_stack_page);
                    }
                }
            ));

            obj.image().connect_is_error_notify(clone!(
                #[weak]
                obj,
                move |_| {
                    glib::spawn_future_local(async move { obj.show_error().await });
                }
            ));

            self.spinner_revealer.connect_map(|revealer| {
                let revealer = revealer.clone();
                glib::timeout_add_local_once(Duration::from_millis(200), move || {
                    revealer.set_reveal_child(true);
                });
            });
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
        @implements gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable, gtk::Accessible;
}

impl LpImagePage {
    pub fn from_file(file: &gio::File) -> Self {
        let obj = glib::Object::new::<Self>();

        obj.imp().image.init(file);

        glib::spawn_future_local(clone!(
            #[weak]
            obj,
            #[strong]
            file,
            async move {
                let imp = obj.imp();
                imp.image.load(&file).await;
            }
        ));

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

    pub async fn show_error(&self) {
        let imp = self.imp();
        let image = self.image();

        if let Some(error) = image.specific_error() {
            let details;
            let description;

            match error {
                DecoderError::NoLoadersConfigured(err) => {
                    // Translators: {} is replaced with a version number
                    description = gettext_f(
                        "No image loaders available. Maybe the “glycin-loaders” package with compatibility version “{}” is not installed.",
                        [format!("{}+", glycin::COMPAT_VERSION)],
                    );

                    details = err.to_string();
                }
                DecoderError::UnsupportedFormat(mime_type, err) => {
                    let content_type =
                        gio::content_type_from_mime_type(&mime_type).unwrap_or_default();
                    let mime_type_description =
                        gio::content_type_get_description(&content_type).to_string();

                    description =
                        if glycin::Loader::DEFAULT_MIME_TYPES.contains(&mime_type.as_str()) {
                            // Translators: The first occurance of {} is replace with a description
                            // of the format and the second with an id
                            // (mime-type) of the format.
                            gettext_f(
                                "The image format “{} ({})” is known but not installed.",
                                [&mime_type_description, &mime_type],
                            )
                        } else {
                            // Translators: The first occurance of {} is replace with a description
                            // of the format and the second with an id
                            // (mime-type) of the format.
                            gettext_f(
                                "Unknown image format “{} ({}).”",
                                [&mime_type_description, &mime_type],
                            )
                        };

                    details = err.to_string();
                }
                DecoderError::OutOfMemory(err) => {
                    description =
                        gettext("There is not enough free memory available to load this image.");
                    details = err.to_string();
                }
                DecoderError::ImageSource(glib_err, err) => {
                    // Translators: The first occurance of {} is replace with a
                    // translated, more detailled error message.
                    description =
                        gettext_f("Failed to load image file: {}", [&glib_err.to_string()]);
                    details = err.to_string();
                }
                DecoderError::Generic(err) => {
                    description = gettext(
                        "Either the image file is corrupted or it contains unsupported elements.",
                    );
                    details = err.to_string();
                }
            }

            imp.error_more_info.connect_clicked(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                move |_| {
                    LpErrorDetails::new(&obj.root().unwrap(), &details, ErrorType::Loader);
                }
            ));

            imp.error_page.set_description(Some(&description));

            imp.error_more_info.set_visible(true);
            imp.stack.set_visible_child(&*imp.error_page);
        }
    }
}
