// Copyright (c) 2024 Sophie Herold
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

use adw::prelude::*;
use adw::subclass::prelude::*;
use gio::glib::VariantTy;
use gtk::CompositeTemplate;

use crate::config;
use crate::deps::*;
use crate::widgets::{LpImageView, LpWindowEdit, LpWindowImage};

/// Show window after X milliseconds even if image dimensions are not known yet
const SHOW_WINDOW_AFTER: u64 = 2000;

mod imp {
    use super::*;

    // To use composite templates, you need
    // to use derive macro. Derive macros generate
    // code to e.g. implement a trait on something.
    // In this case, code is generated for Debug output
    // and to handle binding the template children.
    //
    // For this derive macro, you need to have
    // `use gtk::CompositeTemplate` in your code.
    //
    // Because all of our member fields implement the
    // `Default` trait, we can use `#[derive(Default)]`.
    // If some member fields did not implement default,
    // we'd need to have a `new()` function in the
    // `impl ObjectSubclass for $TYPE` section.
    #[derive(Default, Debug, CompositeTemplate)]
    #[template(file = "window.ui")]
    pub struct LpWindow {
        #[template_child]
        pub(super) toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub(super) stack: TemplateChild<adw::ViewStack>,

        #[template_child]
        pub(super) window_image: TemplateChild<LpWindowImage>,
        #[template_child]
        pub(super) window_edit: TemplateChild<LpWindowEdit>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpWindow {
        const NAME: &'static str = "LpWindow";
        type Type = super::LpWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);

            klass.add_binding_action(
                gdk::Key::question,
                gdk::ModifierType::CONTROL_MASK,
                "win.show-help-overlay",
            );

            klass.install_action(
                "win.show-toast",
                Some(VariantTy::TUPLE),
                move |win, _, var| {
                    if let Some((ref toast, i)) = var.and_then(|v| v.get::<(String, i32)>()) {
                        win.show_toast(toast, adw::ToastPriority::__Unknown(i));
                    }
                },
            );
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for LpWindow {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            if config::APP_ID.ends_with(".Devel") {
                obj.add_css_class("devel");
            }

            // Limit effect of modal dialogs to this window
            // and keeps the others usable
            gtk::WindowGroup::new().add_window(&*obj);

            glib::timeout_add_local_once(
                std::time::Duration::from_millis(SHOW_WINDOW_AFTER),
                glib::clone!(
                    #[weak]
                    obj,
                    move || if !obj.is_visible() {
                        obj.present()
                    }
                ),
            );

            obj.connect_map(|win| {
                win.resize_default();
            });
        }
    }

    impl WidgetImpl for LpWindow {}
    impl WindowImpl for LpWindow {}
    impl ApplicationWindowImpl for LpWindow {}
    impl AdwApplicationWindowImpl for LpWindow {}
}

glib::wrapper! {
    pub struct LpWindow(ObjectSubclass<imp::LpWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionMap, gio::ActionGroup, gtk::Native;
}

impl LpWindow {
    pub fn new<A: IsA<gtk::Application>>(app: &A) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    pub fn toggle_fullscreen(&self, fullscreen: bool) {
        self.set_fullscreened(fullscreen);
    }

    pub fn resize_default(&self) {
        let imp = self.imp();

        if imp
            .window_image
            .image_view()
            .current_image()
            .is_some_and(|img| img.image_size_available())
        {
            let shows_properties = imp.window_image.properties_button().is_active();

            let (_, window_natural_width, _, _) = self.measure(gtk::Orientation::Horizontal, -1);
            let (_, window_natural_height, _, _) = self.measure(gtk::Orientation::Vertical, -1);

            // These have to be in sync with the "conditions" for the "overlay-properties"
            // breakpoint
            let min_width_for_overlay = adw::LengthUnit::Sp.to_px(590., None).ceil() as i32;
            let min_height_for_overlay = adw::LengthUnit::Sp.to_px(550., None).ceil() as i32;

            let (width, height) = if shows_properties
                && window_natural_width < min_width_for_overlay
                && window_natural_height < min_height_for_overlay
            {
                // Avoid overlaying bottom sheet being triggered for the image properties by
                // using a window wide enough to allow for a sidebar
                (min_width_for_overlay.saturating_add(1), -1)
            } else {
                // this lets the window determine the default size from LpImage's natural size
                (-1, -1)
            };

            self.set_default_size(width, height);
        }
    }

    pub fn show_toast(&self, text: impl AsRef<str>, priority: adw::ToastPriority) {
        let imp = self.imp();

        let toast = adw::Toast::new(text.as_ref());
        toast.set_priority(priority);

        imp.toast_overlay.add_toast(toast);
    }

    pub async fn show_about(&self) {
        let about = crate::about::dialog().await;
        about.present(Some(self));
    }

    pub fn show_edit(&self) {
        self.imp().stack.set_visible_child(&*self.imp().window_edit);
    }

    pub fn image_view(&self) -> LpImageView {
        self.imp().window_image.image_view()
    }
}
