// Copyright (c) 2024-2025 Sophie Herold
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

use std::ops::Deref;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gio::glib::VariantTy;
use gtk::CompositeTemplate;
use strum::IntoEnumIterator;

use super::LpErrorDetails;
use crate::config;
use crate::deps::*;
use crate::util::gettext::*;
use crate::util::{Direction, ErrorType};
use crate::widgets::{LpEditWindow, LpImageView, LpImageWindow};

/// Show window after X milliseconds even if image dimensions are not known yet
const SHOW_WINDOW_AFTER: u64 = 2000;

mod imp {
    use std::cell::{Cell, RefCell};

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
    #[derive(Default, Debug, CompositeTemplate, glib::Properties)]
    #[template(file = "window.ui")]
    #[properties(wrapper_type=super::LpWindow)]
    pub struct LpWindow {
        #[template_child]
        pub(super) toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub(super) stack: TemplateChild<adw::ViewStack>,

        #[template_child]
        pub(super) image_window: TemplateChild<LpImageWindow>,
        #[template_child]
        pub(super) edit_window_child: TemplateChild<adw::Bin>,

        #[property(get, set)]
        narrow_layout: Cell<bool>,
        #[property(get, set)]
        wide_layout: Cell<bool>,
        #[property(get, set)]
        layout_name: RefCell<String>,
        #[property(get, set)]
        not_fullscreened: Cell<bool>,
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

            WindowAction::init_actions_and_bindings(klass);

            ActionPartGlobal::init_actions_and_bindings(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
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
                        log::debug!("Showing window after timeout");
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
        @implements gio::ActionMap, gio::ActionGroup, gtk::Native, gtk::Root, gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
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

        if self
            .image_view()
            .current_image()
            .is_some_and(|img| img.image_size_available())
        {
            let shows_properties = imp.image_window.properties_button().is_active();

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

    pub fn add_toast(&self, toast: adw::Toast) {
        self.imp().toast_overlay.add_toast(toast);
    }

    pub fn show_toast(&self, text: &str, priority: adw::ToastPriority) {
        let toast = adw::Toast::new(text);
        toast.set_priority(priority);

        self.add_toast(toast);
    }

    pub fn show_error(&self, stub: &str, details: &str, type_: ErrorType) {
        let action = match type_ {
            ErrorType::General => WindowAction::ShowError,
            ErrorType::Loader => WindowAction::ShowLoaderError,
        };

        let toast = adw::Toast::builder()
            .title(stub)
            .priority(adw::ToastPriority::High)
            .button_label(gettext("Show Details"))
            .action_name(action.to_string())
            .action_target(&details.to_variant())
            .build();

        self.add_toast(toast);
    }

    pub fn show_error_details(&self, details: &str) {
        LpErrorDetails::new(self, details, ErrorType::General);
    }

    pub fn show_loader_error_details(&self, details: &str) {
        LpErrorDetails::new(self, details, ErrorType::Loader);
    }

    pub async fn show_about(&self) {
        let about = crate::about::dialog().await;
        log::debug!("Showing about dialog");
        about.present(Some(self));
    }

    pub fn show_image(&self) {
        self.imp()
            .stack
            .set_visible_child(&*self.imp().image_window);
    }

    pub fn show_specific_image(&self, file: gio::File) {
        log::debug!("Showing specific image: {}", file.uri());
        self.image_view().set_images_from_files(vec![file]);
        self.show_image();
    }

    pub fn show_edit(&self) {
        if let Some(image) = self.image_view().current_image() {
            let edit_child = &*self.imp().edit_window_child;

            edit_child.set_child(Some(&LpEditWindow::new(image)));
            self.imp().stack.set_visible_child(edit_child);
            self.set_fullscreened(false);
        } else {
            log::error!("Can't open image editor since no current image exists");
        }
    }

    pub fn image_view(&self) -> LpImageView {
        self.imp().image_window.image_view()
    }

    pub fn image_window(&self) -> LpImageWindow {
        self.imp().image_window.clone()
    }
}

/// Actions that need some global accels
///
/// The app level accels are needed for arrow key bindings. Because the arrow
/// keys are usually used for widget navigation, it's not possible to overwrite
/// them on key-binding level.
///
/// These have to be registered at window level to be callable from
/// GtkApplication.
#[derive(strum::Display, strum::AsRefStr, strum::EnumIter)]
pub enum ActionPartGlobal {
    #[strum(to_string = "win.image-left-instant")]
    ImageLeftInstant,
    #[strum(to_string = "win.image-right-instant")]
    ImageRightInstant,
    // Pan
    #[strum(to_string = "win.pan-up")]
    PanUp,
    #[strum(to_string = "win.pan-right")]
    PanRight,
    #[strum(to_string = "win.pan-down")]
    PanDown,
    #[strum(to_string = "win.pan-left")]
    PanLeft,
}

impl Deref for ActionPartGlobal {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl ActionPartGlobal {
    pub fn add_accels(application: &gtk::Application) {
        for action in Self::iter() {
            match action {
                Self::ImageRightInstant => application.set_accels_for_action(&action, &["Right"]),
                Self::ImageLeftInstant => application.set_accels_for_action(&action, &["Left"]),
                Self::PanUp => application.set_accels_for_action(&action, &["<Ctrl>Up"]),
                Self::PanRight => application.set_accels_for_action(&action, &["<Ctrl>Right"]),
                Self::PanDown => application.set_accels_for_action(&action, &["<Ctrl>Down"]),
                Self::PanLeft => application.set_accels_for_action(&action, &["<Ctrl>Left"]),
            }
        }
    }

    pub fn remove_accels(application: &gtk::Application) {
        for action in Self::iter() {
            application.set_accels_for_action(&action, &[]);
        }
    }

    pub fn init_actions_and_bindings(klass: &mut <imp::LpWindow as ObjectSubclass>::Class) {
        for action in Self::iter() {
            match action {
                ActionPartGlobal::ImageLeftInstant => {
                    klass.install_action(&action, None, move |win, _, _| {
                        if win.direction() == gtk::TextDirection::Rtl {
                            win.image_view().navigate(Direction::Forward, false);
                        } else {
                            win.image_view().navigate(Direction::Back, false);
                        }
                    });
                    klass.add_binding_action(
                        gdk::Key::Page_Down,
                        gdk::ModifierType::empty(),
                        &action,
                    );
                }

                ActionPartGlobal::ImageRightInstant => {
                    klass.install_action(&action, None, move |win, _, _| {
                        if win.direction() == gtk::TextDirection::Rtl {
                            win.image_view().navigate(Direction::Back, false);
                        } else {
                            win.image_view().navigate(Direction::Forward, false);
                        }
                    });
                    klass.add_binding_action(
                        gdk::Key::Page_Up,
                        gdk::ModifierType::empty(),
                        &action,
                    );
                }

                // Pan
                ActionPartGlobal::PanUp => {
                    klass.install_action(&action, None, move |win, _, _| {
                        win.image_window().pan(&gtk::PanDirection::Up);
                    });
                }
                ActionPartGlobal::PanRight => {
                    klass.install_action(&action, None, move |win, _, _| {
                        win.image_window().pan(&gtk::PanDirection::Right);
                    });
                }
                ActionPartGlobal::PanDown => {
                    klass.install_action(&action, None, move |win, _, _| {
                        win.image_window().pan(&gtk::PanDirection::Down);
                    });
                }
                ActionPartGlobal::PanLeft => {
                    klass.install_action(&action, None, move |win, _, _| {
                        win.image_window().pan(&gtk::PanDirection::Left);
                    });
                }
            }
        }
    }
}

#[derive(strum::Display, strum::AsRefStr, strum::EnumIter)]
enum WindowAction {
    #[strum(to_string = "win.show-error")]
    ShowError,
    #[strum(to_string = "win.show-loader-error")]
    ShowLoaderError,
}

impl Deref for WindowAction {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl WindowAction {
    pub fn init_actions_and_bindings(klass: &mut <imp::LpWindow as ObjectSubclass>::Class) {
        for action in Self::iter() {
            match action {
                Self::ShowError => {
                    klass.install_action(
                        &action,
                        Some(VariantTy::STRING),
                        move |win, _, variant| {
                            if let Some(message) = variant.and_then(String::from_variant) {
                                win.show_error_details(&message);
                            }
                        },
                    );
                }
                Self::ShowLoaderError => {
                    klass.install_action(
                        &action,
                        Some(VariantTy::STRING),
                        move |win, _, variant| {
                            if let Some(message) = variant.and_then(String::from_variant) {
                                win.show_loader_error_details(&message);
                            }
                        },
                    );
                }
            }
        }
    }
}
