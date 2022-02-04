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

use crate::deps::*;

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::clone;
use gtk::subclass::prelude::*;
use gtk_macros::*;

use crate::config;
use crate::window::LpWindow;

mod imp {
    use super::*;

    // The basic struct that holds our
    // state and widgets
    #[derive(Default, Debug)]
    pub struct LpApplication {}

    // Sets up the basics for the GObject
    // The `#[glib::object_subclass] macro implements
    // some boilerplate code for the object setup, e.g. get_type()
    #[glib::object_subclass]
    impl ObjectSubclass for LpApplication {
        const NAME: &'static str = "LpApplication";
        type Type = super::LpApplication;
        type ParentType = adw::Application;
    }

    // Overrides GObject vfuncs
    impl ObjectImpl for LpApplication {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            // Force dark theme
            obj.style_manager()
                .set_color_scheme(adw::ColorScheme::PreferDark);

            // Set up the actions
            obj.setup_actions();
        }
    }

    // Overrides GApplication vfuncs
    impl ApplicationImpl for LpApplication {
        fn activate(&self, application: &Self::Type) {
            let window = LpWindow::new(application);
            window.present();
        }

        // Handles opening files from the command line or other applications
        fn open(&self, application: &Self::Type, files: &[gio::File], _hint: &str) {
            for file in files {
                let win = LpWindow::new(application);
                win.set_image_from_file(file, true);
                win.show();
            }
        }
    }

    // This is empty, but we still need to provide an
    // empty implementation for each type we subclass.
    impl GtkApplicationImpl for LpApplication {}
    impl AdwApplicationImpl for LpApplication {}
}

// Creates a wrapper struct that inherits the functions
// from objects listed as @extends or interfaces it @implements.
// This is what allows us to do e.g. application.quit() on
// LpApplication without casting.
glib::wrapper! {
    pub struct LpApplication(ObjectSubclass<imp::LpApplication>)
        @extends gio::Application, gtk::Application, adw::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

// This is where the member functions of LpApplication go.
#[allow(clippy::new_without_default)]
impl LpApplication {
    pub fn new() -> Self {
        glib::Object::new(&[
            ("application-id", &config::APP_ID.to_string()),
            ("flags", &gio::ApplicationFlags::HANDLES_OPEN),
            ("resource-base-path", &"/org/gnome/Loupe/".to_string()),
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
                let win = LpWindow::new(&app);
                win.show();
            })
        );

        self.set_accels_for_action("app.help", &["F1"]);
        self.set_accels_for_action("app.quit", &["<Primary>Q"]);
        self.set_accels_for_action("app.new-window", &["<Primary>N"]);
        self.set_accels_for_action("win.open", &["<Primary>O"]);
        self.set_accels_for_action("win.print", &["<Primary>P"]);
        self.set_accels_for_action("win.copy", &["<Primary>C"]);
        self.set_accels_for_action("win.show-help-overlay", &["<Primary>question"]);
        self.set_accels_for_action("win.toggle-fullscreen", &["F11"]);
        self.set_accels_for_action("window.close", &["<Primary>W"]);
    }

    pub fn show_about(&self) {
        // Builders are a pattern that allow you to create
        // an object and set all relevant properties very
        // easily in a way that's idiomatic to Rust.
        let dialog = gtk::AboutDialog::builder()
            .authors(vec![String::from(
                "Christopher Davis <christopherdavis@gnome.org>",
            )])
            .artists(vec![
                String::from("Allan Day"),
                String::from("Jakub Steiner"),
                String::from("Tobias Bernard"),
            ])
            .copyright("© 2021 The GNOME Project")
            .license_type(gtk::License::Gpl30)
            .program_name("Image Viewer")
            .logo_icon_name(config::APP_ID)
            .version(config::VERSION)
            .build();

        if let Some(window) = self.active_window() {
            dialog.set_modal(true);
            dialog.set_transient_for(Some(&window));
        }

        dialog.show();
    }

    pub fn show_help(&self) {
        gtk::show_uri(self.active_window().as_ref(), "help:loupe", 0);
    }
}
