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

use gettextrs::*;
use gtk::gio::{self, prelude::*};

mod application;
mod config;
mod thumbnail;
mod util;
mod widgets;
mod window;

mod deps {
    pub use gtk::{gdk, gdk_pixbuf, gio, glib, graphene};
}

use application::LpApplication;

fn main() {
    pretty_env_logger::init();

    setlocale(LocaleCategory::LcAll, "");
    bindtextdomain("loupe", config::LOCALEDIR).unwrap();
    textdomain("loupe").unwrap();

    let res = gio::Resource::load(config::PKGDATADIR.to_owned() + "/loupe.gresource")
        .expect("Could not load resources");
    gio::resources_register(&res);

    let app = LpApplication::new();
    let ret = app.run();
    std::process::exit(ret);
}
