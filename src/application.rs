// Copyright (c) 2020-2023 Christopher Davis
// Copyright (c) 2022-2024 Sophie Herold
// Copyright (c) 2023 Matteo Nardi
// Copyright (c) 2023 Julian Hofer
// Copyright (c) 2023 Huan Nguyen
// Copyright (c) 2024 Balló György
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

use crate::config;
use crate::deps::*;
use crate::widgets::LpWindow;

mod imp {
    use super::*;
    use crate::widgets::LpShyBin;

    // The basic struct that holds our
    // state and widgets
    #[derive(Debug)]
    pub struct LpApplication {
        pub settings: gio::Settings,
    }

    impl Default for LpApplication {
        fn default() -> Self {
            Self {
                settings: gio::Settings::new(config::APP_ID),
            }
        }
    }

    // Sets up the basics for the GObject
    // The `#[glib::object_subclass] macro implements
    // some boilerplate code for the object setup, e.g. get_type()
    #[glib::object_subclass]
    impl ObjectSubclass for LpApplication {
        const NAME: &'static str = "LpApplication";
        type Type = super::LpApplication;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for LpApplication {
        fn constructed(&self) {
            let obj = self.obj();

            self.parent_constructed();

            // Set up the actions
            obj.setup_actions();
        }
    }

    impl ApplicationImpl for LpApplication {
        fn startup(&self) {
            log::trace!("Startup");
            self.parent_startup();

            // Force dark theme
            self.obj()
                .style_manager()
                .set_color_scheme(adw::ColorScheme::PreferDark);

            LpShyBin::ensure_type();
            gtk::Window::set_default_icon_name(config::APP_ID);
        }

        fn activate(&self) {
            log::debug!("Showing window via 'activate'");
            let application = self.obj();
            let window = LpWindow::new(&*application);
            window.present();
        }

        // Handles opening files from the command line or other applications
        fn open(&self, files: &[gio::File], _hint: &str) {
            log::trace!("Open {} file(s)", files.len());
            let application = self.obj();
            let win = LpWindow::new(&*application);
            win.image_view().set_images_from_files(files.to_vec());
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
            gio::ActionEntryBuilder::new("help")
                .activate(|app: &Self, _, _| app.show_help())
                .build(),
            gio::ActionEntryBuilder::new("quit")
                .activate(|app: &Self, _, _| app.quit())
                .build(),
            gio::ActionEntryBuilder::new("new-window")
                .activate(|app: &Self, _, _| {
                    let win = LpWindow::new(app);
                    log::debug!("Showing new window");
                    win.present();
                })
                .build(),
        ];

        self.add_action_entries(actions);

        self.set_accels_for_action("app.help", &["F1"]);
        self.set_accels_for_action("app.quit", &["<Ctrl>Q"]);
        self.set_accels_for_action("window.close", &["<Ctrl>W"]);
        self.set_accels_for_action("app.new-window", &["<Ctrl>N"]);
    }

    pub fn show_help(&self) {
        let context = self
            .active_window()
            .map(|w| gtk::prelude::WidgetExt::display(&w).app_launch_context());
        glib::spawn_future_local(async move {
            if let Err(e) =
                gio::AppInfo::launch_default_for_uri_future("help:loupe", context.as_ref()).await
            {
                log::error!("Failed to launch help: {}", e.message());
            }
        });
    }

    pub fn settings(&self) -> gio::Settings {
        self.imp().settings.clone()
    }
}

impl Default for LpApplication {
    fn default() -> Self {
        gio::Application::default()
            .unwrap()
            .downcast::<LpApplication>()
            .unwrap()
    }
}
