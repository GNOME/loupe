// application.rs
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

use gio::prelude::*;
use glib::clone;
use glib::subclass::prelude::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk_macros::*;

use crate::config;
use crate::window::IvWindow;

mod imp {
    use super::*;

    // The basic struct that holds our
    // state and widgets
    #[derive(Debug)]
    pub struct IvApplication {
        pub gtk_settings: gtk::Settings,
    }

    // Sets up the basics for the GObject
    impl ObjectSubclass for IvApplication {
        const NAME: &'static str = "IvApplication";
        type Type = super::IvApplication;
        type ParentType = gtk::Application;
        type Instance = glib::subclass::simple::InstanceStruct<Self>;
        type Class = glib::subclass::simple::ClassStruct<Self>;

        // This macro implements some boilerplate code
        // for the object setup, e.g. get_type()
        glib::object_subclass!();

        // Initialize with default values
        fn new() -> Self {
            let gtk_settings =
                gtk::Settings::get_default().expect("Could not get default GTK settings");

            Self { gtk_settings }
        }
    }

    // Overrides GObject vfuncs
    impl ObjectImpl for IvApplication {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            // Set up the CSS and force dark theme
            let display = gdk::Display::get_default().unwrap();
            let provider = gtk::CssProvider::new();
            provider.load_from_resource("/org/gnome/ImageViewer/image-viewer.css");
            gtk::StyleContext::add_provider_for_display(&display, &provider, 600);
            self.gtk_settings
                .set_property_gtk_application_prefer_dark_theme(true);

            // Set up the actions
            obj.setup_actions();
        }
    }

    // Overrides GApplication vfuncs
    impl ApplicationImpl for IvApplication {
        fn activate(&self, application: &Self::Type) {
            let window = IvWindow::new(application);

            application.add_window(&window);
            window.present();
        }

        // Handles opening files from the command line or other applications
        fn open(&self, application: &Self::Type, files: &[gio::File], _hint: &str) {
            for file in files {
                let win = IvWindow::new(application);
                win.set_image_from_file(file);
                win.show();
            }
        }
    }

    // This is empty, but we still need to provide an
    // empty implementation for each type we subclass.
    impl GtkApplicationImpl for IvApplication {}
}

// Creates a wrapper struct that inherits the functions
// from objects listed as @extends or interfaces it @implements.
// This is what allows us to do e.g. application.quit() on
// IvApplication without casting.
glib::wrapper! {
    pub struct IvApplication(ObjectSubclass<imp::IvApplication>)
        @extends gio::Application, gtk::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

// This is where the member functions of IvApplication go.
impl IvApplication {
    pub fn new() -> Self {
        glib::Object::new(&[
            ("application-id", &config::APP_ID.to_string()),
            ("flags", &gio::ApplicationFlags::HANDLES_OPEN),
        ])
        .unwrap()
    }

    pub fn setup_actions(&self) {
        // action! is a macro from gtk_macros
        // that creates a GSimpleAction with a callback.
        // clone! is a macro from glib-rs that allows
        // you to easily handle references in callbacks
        // without refcycles or leaks.
        //
        // When you don't want the callback to keep the
        // Object alive, pass as @weak. Otherwise, pass
        // as @strong. Most of the time you will want
        // to use @weak.
        action!(
            self,
            "about",
            clone!(@weak self as app => move |_, _| {
                app.show_about();
            })
        );

        action!(
            self,
            "help",
            clone!(@weak self as app => move |_, _| {
                app.show_help();
            })
        );

        action!(
            self,
            "quit",
            clone!(@weak self as app  => move |_, _| {
                app.quit();
            })
        );

        action!(
            self,
            "new-window",
            clone!(@weak self as app => move |_, _| {
                let win = IvWindow::new(&app);
                win.show();
            })
        );

        self.set_accels_for_action("app.help", &["F1"]);
        self.set_accels_for_action("app.quit", &["<Primary>Q"]);
        self.set_accels_for_action("app.new-window", &["<Primary>N"]);
        self.set_accels_for_action("win.open", &["<Primary>O"]);
        self.set_accels_for_action("win.print", &["<Primary>P"]);
        self.set_accels_for_action("win.show-help-overlay", &["<Primary>F1"]);
        self.set_accels_for_action("win.toggle-fullscreen", &["F11"]);
        self.set_accels_for_action("window.close", &["<Primary>W"]);
    }

    pub fn show_about(&self) {
        // Builders are a pattern that allow you to create
        // an object and set all relevant properties very
        // easily in a way that's idiomatic to Rust.
        let dialog = gtk::AboutDialogBuilder::new()
            .authors(vec![String::from(
                "Christopher Davis <christopherdavis@gnome.org>",
            )])
            .artists(vec![
                String::from("Allan Day"),
                String::from("Jakub Steiner"),
                String::from("Tobias Bernard"),
            ])
            .copyright("© 2020 The GNOME Project")
            .license_type(gtk::License::Gpl30)
            .program_name("Image Viewer")
            .logo_icon_name(config::APP_ID)
            .version(config::VERSION)
            .build();

        if let Some(window) = self.get_active_window() {
            dialog.set_modal(true);
            dialog.set_transient_for(Some(&window));
        }

        dialog.show();
    }

    pub fn show_help(&self) {
        gtk::show_uri(gtk::NONE_WINDOW, "help:gnome-image-viewer", 0);
    }
}
