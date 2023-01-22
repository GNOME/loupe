// window.rs
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

use std::cell::RefCell;

use crate::config;
use crate::util;
use crate::widgets::{LpImage, LpImagePage, LpImageView, LpPropertiesView};

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
    #[template(resource = "/org/gnome/Loupe/gtk/window.ui")]
    pub struct LpWindow {
        // Template children are used with the
        // TemplateChild<T> wrapper, where T is the
        // object type of the template child.
        #[template_child]
        pub flap: TemplateChild<adw::Flap>,
        #[template_child]
        pub headerbar: TemplateChild<gtk::HeaderBar>,
        #[template_child]
        pub properties_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub menu_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub status_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub image_view: TemplateChild<LpImageView>,
        #[template_child]
        pub properties_view: TemplateChild<LpPropertiesView>,
        #[template_child]
        pub drop_target: TemplateChild<gtk::DropTarget>,

        pub watch_image_size: RefCell<Option<gtk::ExpressionWatch>>,
        pub watch_image_error: RefCell<Option<gtk::ExpressionWatch>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpWindow {
        const NAME: &'static str = "LpWindow";
        type Type = super::LpWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            // bind_template() is a function generated by the
            // CompositeTemplate macro to bind all children at once.
            Self::bind_template(klass);
            Self::Type::bind_template_callbacks(klass);

            // Set up actions
            klass.install_action("win.toggle-fullscreen", None, move |win, _, _| {
                win.toggle_fullscreen(!win.is_fullscreened());
            });

            klass.install_action("win.next", None, move |win, _, _| {
                win.imp()
                    .image_view
                    .navigate(adw::NavigationDirection::Forward);
            });

            klass.install_action("win.previous", None, move |win, _, _| {
                win.imp()
                    .image_view
                    .navigate(adw::NavigationDirection::Back);
            });

            klass.install_action("win.zoom-out", None, move |win, _, _| {
                win.zoom_out();
            });

            klass.install_action("win.zoom-in", None, move |win, _, _| {
                win.zoom_in();
            });

            klass.install_action("win.zoom-to", Some("d"), move |win, _, level| {
                win.zoom_to(level.unwrap().get().unwrap());
            });

            klass.install_action("win.zoom-best-fit", None, move |win, _, _| {
                win.zoom_best_fit();
            });

            klass.install_action("win.leave-fullscreen", None, move |win, _, _| {
                win.toggle_fullscreen(false);
            });

            klass.install_action("win.toggle-properties", None, move |win, _, _| {
                win.imp()
                    .properties_button
                    .set_active(!win.imp().properties_button.is_active());
            });

            klass.install_action_async("win.open", None, |win, _, _| async move {
                win.pick_file().await;
            });

            klass.install_action_async("win.open-with", None, |win, _, _| async move {
                win.open_with().await;
            });

            klass.install_action("win.rotate", Some("d"), move |win, _, angle| {
                win.rotate_image(angle.unwrap().get().unwrap());
            });

            klass.install_action_async("win.set-background", None, |win, _, _| async move {
                win.set_background().await;
            });

            klass.install_action("win.print", None, move |win, _, _| {
                win.print();
            });

            klass.install_action("win.copy", None, move |win, _, _| {
                win.copy();
            });

            klass.install_action("win.show-toast", Some("(si)"), move |win, _, var| {
                if let Some((ref toast, i)) = var.and_then(|v| v.get::<(String, i32)>()) {
                    win.show_toast(toast, adw::ToastPriority::__Unknown(i));
                }
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for LpWindow {
        fn constructed(&self) {
            let obj = self.instance();

            self.parent_constructed();

            if config::PROFILE == ".Devel" {
                obj.add_css_class("devel");
            }

            obj.set_actions_enabled(false);
            self.image_view
                .property_expression("current-page-strict")
                .chain_property::<LpImagePage>("image")
                .chain_property::<LpImage>("best-fit")
                .watch(
                    glib::Object::NONE,
                    // clone! is a macro from glib-rs that allows
                    // you to easily handle references in callbacks
                    // without refcycles or leaks.
                    //
                    // When you don't want the callback to keep the
                    // Object alive, pass as @weak. Otherwise, pass
                    // as @strong. Most of the time you will want
                    // to use @weak.
                    glib::clone!(@weak obj => move || {
                        let enabled = obj
                            .imp()
                            .image_view
                            .current_page()
                            .map(|page| !page.image().is_best_fit())
                            .unwrap_or_default();

                        obj.action_set_enabled("win.zoom-out", enabled);
                    }),
                );

            // disable zoom-in if at maximum zoom level
            self.image_view
                .property_expression("current-page-strict")
                .chain_property::<LpImagePage>("image")
                .chain_property::<LpImage>("is-max-zoom")
                .watch(
                    glib::Object::NONE,
                    glib::clone!(@weak obj => move || {
                        let enabled = obj
                            .imp()
                            .image_view
                            .current_page()
                            .map(|page| !page.image().is_max_zoom())
                            .unwrap_or_default();

                        obj.action_set_enabled("win.zoom-in", enabled);
                    }),
                );

            // action win.previous status
            self.image_view.connect_notify_local(
                Some("is-previous-available"),
                glib::clone!(@weak obj => move |_, _| {
                    obj.action_set_enabled(
                        "win.previous",
                        obj.imp().image_view.is_previous_available(),
                    );
                }),
            );

            // action win.next status
            self.image_view.connect_notify_local(
                Some("is-next-available"),
                glib::clone!(@weak obj => move |_, _| {
                    obj.action_set_enabled(
                        "win.next",
                        obj.imp().image_view.is_next_available(),
                    );
                }),
            );

            self.status_page
                .set_icon_name(Some(&format!("{}-symbolic", config::APP_ID)));

            // Set help overlay
            let builder = gtk::Builder::from_resource("/org/gnome/Loupe/gtk/help_overlay.ui");
            let help_overlay = builder.object("help_overlay").unwrap();
            obj.set_help_overlay(Some(&help_overlay));

            self.drop_target.set_types(&[gdk::FileList::static_type()]);

            // For callbacks, you will want to reference the GTK docs on
            // the relevant signal to see which parameters you need.
            // In this case, we need only need the GValue,
            // so we name it `value` then use `_` for the other spots.
            self.drop_target.connect_drop(
                clone!(@weak obj => @default-return false, move |_, value, _, _| {
                    // Here we use a GValue, which is a dynamic object that can hold different types,
                    // e.g. strings, numbers, or in this case objects. In order to get the GdkFileList
                    // from the GValue, we need to use the `get()` method.
                    //
                    // We've added type annotations here, and written it as `let list: gdk::FileList = ...`,
                    // but you might also see places where type arguments are used.
                    // This line could have been written as `let list = value.get::<gdk::FileList>().unwrap()`.
                    let list: gdk::FileList = match value.get() {
                        Ok(list) => list,
                        Err(err) => {
                            log::error!("Issue with drop value: {err}");
                            return false;
                        }
                    };

                    // TODO: Handle this like EOG and make a "directory" out of the given files
                    let file = list.files().get(0).unwrap().clone();
                    let info = util::query_attributes(
                        &file,
                        vec![
                            &gio::FILE_ATTRIBUTE_STANDARD_CONTENT_TYPE,
                            &gio::FILE_ATTRIBUTE_STANDARD_DISPLAY_NAME,
                        ],
                    )
                    .expect("Could not query file info");

                    if info
                        .content_type()
                        .map(|t| t.to_string())
                        .filter(|t| t.starts_with("image/"))
                        .is_some() {
                        obj.set_image_from_file(&file, false);
                    } else {
                        obj.show_toast(
                            i18n_f("“{}” is not a valid image.", &[&info.display_name()]),
                            adw::ToastPriority::High,
                        );
                    }

                    true
                }),
            );
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

#[gtk::template_callbacks]
impl LpWindow {
    pub fn new<A: IsA<gtk::Application>>(app: &A) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    fn toggle_fullscreen(&self, fullscreen: bool) {
        let imp = self.imp();

        if fullscreen {
            imp.headerbar.add_css_class("osd");
            imp.properties_view.add_css_class("osd")
        } else {
            imp.headerbar.remove_css_class("osd");
            imp.properties_view.remove_css_class("osd")
        }

        self.set_fullscreened(fullscreen);
    }

    fn zoom_out(&self) {
        self.imp().image_view.zoom_out();
    }

    fn zoom_in(&self) {
        self.imp().image_view.zoom_in();
    }

    fn zoom_to(&self, level: f64) {
        self.imp().image_view.zoom_to(level);
    }

    fn zoom_best_fit(&self) {
        if let Some(page) = self.imp().image_view.current_page() {
            page.image().zoom_best_fit();
        }
    }

    async fn pick_file(&self) {
        let filter_store = gio::ListStore::new(gtk::FileFilter::static_type());

        let filter = gtk::FileFilter::new();
        filter.set_property("name", &String::from("Supported image files"));
        filter.add_mime_type("image/*");
        filter_store.append(&filter);

        let chooser = gtk::FileDialog::builder()
            .title(&i18n("Open Image"))
            .filters(&filter_store)
            .modal(true)
            .build();

        if let Ok(file) = chooser.open_future(Some(self)).await {
            self.set_image_from_file(&file, true);
        }
    }

    async fn open_with(&self) {
        let imp = self.imp();

        if let Some(ref file) = imp.image_view.active_file() {
            let launcher = gtk::FileLauncher::new(Some(file));
            if let Err(e) = launcher.launch_future(Some(self)).await {
                if !e.matches(gtk::DialogError::Dismissed) {
                    log::error!("Could not open image in external program: {}", e);
                }
            }
        } else {
            log::error!("Could not load a path for the current image.")
        }
    }

    fn rotate_image(&self, angle: f64) {
        self.imp().image_view.rotate_image(angle)
    }

    async fn set_background(&self) {
        let imp = self.imp();

        if let Err(e) = imp.image_view.set_background().await {
            log::error!("Failed to set background: {}", e);
        }
    }

    fn print(&self) {
        let imp = self.imp();

        if let Err(e) = imp.image_view.print() {
            log::error!("Failed to print file: {}", e);
        }
    }

    fn copy(&self) {
        let imp = self.imp();

        if let Err(e) = imp.image_view.copy() {
            log::error!("Failed to copy to clipboard: {}", e);
        } else {
            self.show_toast(i18n("Image copied to clipboard"), adw::ToastPriority::High);
        }
    }

    fn show_toast(&self, text: impl AsRef<str>, priority: adw::ToastPriority) {
        let imp = self.imp();

        let toast = adw::Toast::new(text.as_ref());
        toast.set_priority(priority);

        imp.toast_overlay.add_toast(&toast);
    }

    pub fn set_image_from_file(&self, file: &gio::File, _resize: bool) {
        let imp = self.imp();

        log::debug!("Loading file: {}", file.uri().to_string());
        match imp.image_view.set_image_from_file(file) {
            Ok((/*width, height*/)) => {
                // if resize {
                    // self.resize_from_dimensions(width, height);
                // }

                imp.stack.set_visible_child(&*imp.image_view);
                imp.image_view.grab_focus();
                self.set_actions_enabled(true)
            }
            Err(e) => log::error!("Could not load file: {}", e.to_string()),
        }
    }

    pub fn set_actions_enabled(&self, enabled: bool) {
        self.action_set_enabled("win.open-with", enabled);
        self.action_set_enabled("win.set-background", enabled);
        self.action_set_enabled("win.toggle-fullscreen", enabled);
        self.action_set_enabled("win.print", enabled);
        self.action_set_enabled("win.rotate", enabled);
        self.action_set_enabled("win.copy", enabled);
        self.action_set_enabled("win.zoom-best-fit", enabled);
        self.action_set_enabled("win.zoom-to", enabled);
        self.action_set_enabled("win.toggle-properties", enabled);
    }

    pub fn image_size_ready(&self) {
        // if visible for whatever reason, don't do any resize
        if self.is_visible() {
            self.disconnect_present_watches();
            return;
        }

        let image = self
            .imp()
            .image_view
            .current_page_strict()
            .map(|page| page.image());

        if let Some(image) = image {
            if image.image_size() > (0, 0) {
                log::debug!("Showing window because image size is ready");
                // this let's the window determine the default size from LpImage's natural size
                self.set_default_size(-1, -1);
                self.disconnect_present_watches();
                self.present();
            }
        }
    }

    pub fn image_error(&self) {
        if self.is_visible() {
            self.disconnect_present_watches();
            return;
        }

        let current_page = self.imp().image_view.current_page_strict();

        if let Some(page) = current_page {
            if page.error().is_some() {
                log::debug!("Showin window because loading image failed");
                self.disconnect_present_watches();
                self.present();
            }
        }
    }

    fn disconnect_present_watches(&self) {
        if let Some(watch) = self.imp().watch_image_size.take() {
            watch.unwatch();
        }
        if let Some(watch) = self.imp().watch_image_error.take() {
            watch.unwatch();
        }
    }

    // Adapted from https://gitlab.gnome.org/GNOME/eog/-/blob/master/src/eog-window.c:eog_window_obtain_desired_size
    pub fn resize_from_dimensions(&self, img_width: i32, img_height: i32) {
        let imp = self.imp();
        let mut final_width = img_width;
        let mut final_height = img_height;

        let header_height = imp.headerbar.height();

        // Ensure the window surface exists
        if !self.is_realized() {
            WidgetExt::realize(self);
        }

        let display = gdk::Display::default().unwrap();
        let monitor = display.monitor_at_surface(&self.native().unwrap().surface());
        let monitor_geometry = monitor.geometry();

        let monitor_width = monitor_geometry.width();
        let monitor_height = monitor_geometry.height();

        if img_width > monitor_width || img_height + header_height > monitor_height {
            let width_factor = (monitor_width as f32 * 0.85) / img_width as f32;
            let height_factor =
                (monitor_height as f32 * 0.85 - header_height as f32) / img_height as f32;
            let factor = width_factor.min(height_factor);

            final_width = (final_width as f32 * factor).round() as i32;
            final_height = (final_height as f32 * factor).round() as i32;
        }

        self.set_default_size(final_width, final_height);
        log::debug!("Window resized to {} x {}", final_width, final_height);
    }

    // In the LpWindow UI file we define a `gtk::Expression`s
    // that is a closure. This closure takes the current `gio::File`
    // and processes it to return a window title.
    //
    // In this function we chain `Option`s with `and_then()` in order
    // to handle optional results with a fallback, without needing to
    // have multiple `match` or `if let` branches, and without needing
    // to unwrap.
    #[template_callback]
    fn window_title(&self, file: Option<gio::File>) -> String {
        file.and_then(|f| util::get_file_display_name(&f)) // If the file exists, get display name
            .unwrap_or_else(|| i18n("Loupe")) // Return that or the default if there's nothing
    }

    // We also have a closure that returns `adw::FlapFoldPolicy`.
    // if we aren't fullscreened, or if the properties are revealed,
    // we unfold the main flap. Otherwise we're always folded.
    #[template_callback]
    fn fold_policy(&self, fullscreened: bool, properties_revealed: bool) -> adw::FlapFoldPolicy {
        if !fullscreened || properties_revealed {
            adw::FlapFoldPolicy::Never
        } else {
            adw::FlapFoldPolicy::Always
        }
    }
}
