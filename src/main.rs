// main.rs
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

/*!
# Loupe Image Viewer

Code documentation

# Widget Structure

Simplified widget arrangement including the most important widgets.

-   [`LpWindow`]
    -   [`AdwFlap`]
        -   [`LpPropertiesView`]
        -   [`LpImageView`]
            -   [`LpSlidingView`]
                -   [`LpImagePage`]
                    -   [`GtkScrolledWindow`]
                        -   [`LpImage`]
                -   …

[`AdwFlap`]: adw::Flap
[`GtkScrolledWindow`]: gtk::ScrolledWindow
[`LpImagePage`]: widgets::LpImagePage
[`LpImageView`]: widgets::LpImageView
[`LpImage`]: widgets::LpImage
[`LpPropertiesView`]: widgets::LpPropertiesView
[`LpSlidingView`]: widgets::LpSlidingView
[`LpWindow`]: window::LpWindow
*/

use gettextrs::*;
use gtk::gio::{self, prelude::*};

mod application;
mod config;
mod decoder;
mod file_model;
mod image_metadata;
mod util;
mod widgets;
mod window;

mod deps {
    pub use gtk::{cairo, gdk, gdk_pixbuf, gio, glib, graphene, gsk};
}

use application::LpApplication;
use deps::*;

fn main() -> glib::ExitCode {
    env_logger::Builder::from_default_env()
        .format_timestamp_millis()
        .init();

    setlocale(LocaleCategory::LcAll, "");
    bindtextdomain("loupe", config::LOCALEDIR).unwrap();
    textdomain("loupe").unwrap();

    let res = gio::Resource::load(config::PKGDATADIR.to_owned() + "/loupe.gresource")
        .expect("Could not load resources");
    gio::resources_register(&res);

    LpApplication::new().run()
}
