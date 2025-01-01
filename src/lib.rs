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

#![allow(clippy::new_without_default)]

/*!
# Loupe Image Viewer

Code documentation

# Widget Structure

Simplified widget arrangement including the most important widgets.

-   [`LpWindow`]
    -   [`LpImageWindow`]
        -   [`LpPropertiesView`]
        -   [`LpImageView`]
            -   [`LpSlidingView`]
                -   [`LpImagePage`]
                    -   [`GtkScrolledWindow`]
                        -   [`LpImage`]
                -   …
    -   [`LpEditWindow`]
        - [`LpEditCrop`]


[`GtkScrolledWindow`]: gtk::ScrolledWindow
[`LpEditCrop`]: widgets::edit::LpEditCrop
[`LpEditWindow`]: widgets::LpEditWindow
[`LpImagePage`]: widgets::LpImagePage
[`LpImageView`]: widgets::LpImageView
[`LpImageWindow`]: widgets::LpImageWindow
[`LpImage`]: widgets::LpImage
[`LpPropertiesView`]: widgets::LpPropertiesView
[`LpSlidingView`]: widgets::LpSlidingView
[`LpWindow`]: widgets::LpWindow
*/

use gettextrs::*;
use gtk::gio::prelude::*;
use gtk::gio::{self};

pub mod about;
pub mod application;
pub mod config;
pub mod decoder;
pub mod editing;
pub mod file_model;
pub mod metadata;
pub mod util;
pub mod widgets;

mod deps {
    pub use gtk::{cairo, gdk, gio, glib, graphene, gsk};
}

use application::LpApplication;
use deps::*;
use tracing_subscriber::layer::*;
use tracing_subscriber::util::*;

static GRESOURCE_BYTES: &[u8] =
    gvdb_macros::include_gresource_from_dir!("/org/gnome/Loupe", "data/resources");

pub fn main() -> glib::ExitCode {
    // Follow G_MESSAGES_DEBUG env variable
    let default_level =
        if !glib::log_writer_default_would_drop(glib::LogLevel::Debug, Some("loupe")) {
            tracing_subscriber::filter::LevelFilter::DEBUG
        } else {
            tracing_subscriber::filter::LevelFilter::ERROR
        };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(default_level.into())
                .from_env_lossy(),
        )
        .with(tracing_subscriber::fmt::Layer::default().compact())
        .init();

    log::debug!("Logger initialized");

    setlocale(LocaleCategory::LcAll, "");
    bindtextdomain("loupe", config::LOCALEDIR).unwrap();
    textdomain("loupe").unwrap();

    log::trace!("gettext initialized");

    gio::resources_register(
        &gio::Resource::from_data(&glib::Bytes::from_static(GRESOURCE_BYTES)).unwrap(),
    );

    log::trace!("Gio resources registered");

    LpApplication::new().run()
}
