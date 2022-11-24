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
use crate::i18n::*;

use adw::prelude::*;
use adw::subclass::prelude::*;

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
        fn constructed(&self) {
            let obj = self.instance();

            self.parent_constructed();

            // Force dark theme
            obj.style_manager()
                .set_color_scheme(adw::ColorScheme::PreferDark);

            // Set up the actions
            obj.setup_actions();
        }
    }

    // Overrides GApplication vfuncs
    impl ApplicationImpl for LpApplication {
        fn activate(&self) {
            let application = self.instance();
            let window = LpWindow::new(&*application);
            window.present();
        }

        // Handles opening files from the command line or other applications
        fn open(&self, files: &[gio::File], _hint: &str) {
            let application = self.instance();
            for file in files {
                let win = LpWindow::new(&*application);
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
        glib::Object::builder()
            .property("application-id", config::APP_ID)
            .property("flags", gio::ApplicationFlags::HANDLES_OPEN)
            .property("resource-base-path", "/org/gnome/Loupe")
            .build()
    }

    pub fn setup_actions(&self) {
        // gio::ActionEntryBuilder allows us to build and store an action on an object
        // that implements gio::ActionMap. Here we build the application's actions and
        // add them with add_action_entries().
        let actions = [
            gio::ActionEntryBuilder::new("about")
                .activate(|app: &Self, _, _| app.show_about())
                .build(),
            gio::ActionEntryBuilder::new("help")
                .activate(|app: &Self, _, _| app.show_help())
                .build(),
            gio::ActionEntryBuilder::new("quit")
                .activate(|app: &Self, _, _| app.quit())
                .build(),
            gio::ActionEntryBuilder::new("new-window")
                .activate(|app: &Self, _, _| {
                    let win = LpWindow::new(app);
                    win.show();
                })
                .build(),
        ];

        self.add_action_entries(actions).unwrap();

        self.set_accels_for_action("app.help", &["F1"]);
        self.set_accels_for_action("app.quit", &["<Primary>Q"]);
        self.set_accels_for_action("app.new-window", &["<Primary>N"]);
        self.set_accels_for_action("win.open", &["<Primary>O"]);
        self.set_accels_for_action("win.print", &["<Primary>P"]);
        self.set_accels_for_action("win.copy", &["<Primary>C"]);
        self.set_accels_for_action("win.show-help-overlay", &["<Primary>question"]);
        self.set_accels_for_action("win.toggle-fullscreen", &["F11"]);
        self.set_accels_for_action("window.close", &["<Primary>W"]);
        self.set_accels_for_action("win.zoom-to(1.0)", &["1"]);
        self.set_accels_for_action("win.zoom-to(2.0)", &["2"]);
    }

    pub fn show_about(&self) {
        // Builders are a pattern that allow you to create
        // an object and set all relevant properties very
        // easily in a way that's idiomatic to Rust.
        let about = adw::AboutWindow::builder()
            .application_name(&i18n("Loupe"))
            .application_icon(config::APP_ID)
            .version(config::VERSION)
            .developer_name(&i18n("Christopher Davis"))
            .website("https://gitlab.gnome.org/BrainBlasted/loupe")
            .issue_url("https://gitlab.gnome.org/BrainBlasted/loupe/-/issues/new")
            .developers(vec![
                String::from("Christopher Davis <christopherdavis@gnome.org>"),
                String::from("Sophie Herold <sophieherold@gnome.org>"),
            ])
            .designers(vec![
                String::from("Allan Day"),
                String::from("Jakub Steiner"),
                String::from("Tobias Bernard"),
            ])
            .copyright(&i18n("Copyright © 2020–2022 Christopher Davis et al."))
            .license_type(gtk::License::Gpl30)
            .build();

        if let Some(window) = self.active_window() {
            about.set_transient_for(Some(&window));
        }

        about.show();
    }

    pub fn show_help(&self) {
        gtk::show_uri(self.active_window().as_ref(), "help:loupe", 0);
    }
}
