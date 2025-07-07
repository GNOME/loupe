// Copyright (c) 2023-2025 Sophie Herold
// Copyright (c) 2023 Automeris naranja
// Copyright (c) 2023 Christopher Davis
// Copyright (c) 2024 Andre Klapper
// Copyright (c) 2024 Dexter Reed
// Copyright (c) 2025 sid
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

use gettextrs::gettext;

use crate::config;

pub async fn dialog() -> adw::AboutDialog {
    // Builders are a pattern that allow you to create
    // an object and set all relevant properties very
    // easily in a way that's idiomatic to Rust.
    adw::AboutDialog::builder()
        .application_name(gettext("Image Viewer"))
        .application_icon(config::APP_ID)
        .version(config::VERSION)
        .developer_name(gettext("The GNOME Project"))
        .website("https://apps.gnome.org/Loupe/")
        .issue_url("https://gitlab.gnome.org/GNOME/loupe/-/issues/")
        .support_url("https://discourse.gnome.org/tag/loupe")
        .developers([
            "Christopher Davis <christopherdavis@gnome.org>",
            "Sophie Herold <sophieherold@gnome.org>",
        ])
        .designers(["Allan Day", "Jakub Steiner", "Tobias Bernard"])
        // Translators: Replace "translator-credits" with your names, one name per line
        .translator_credits(gettext("translator-credits"))
        .copyright(gettext("Copyright © 2020–2024 Christopher Davis et al."))
        .license_type(gtk::License::Gpl30)
        .debug_info(debug_info().await)
        .build()
}

async fn etc() -> std::path::PathBuf {
    if ashpd::is_sandboxed().await {
        std::path::PathBuf::from("/run/host/etc")
    } else {
        std::path::PathBuf::from("/etc")
    }
}

async fn os_release() -> String {
    let os_release = etc().await.join("os-release");
    std::fs::read_to_string(os_release).unwrap_or_default()
}

async fn debug_info() -> String {
    [
        format!("- Version: {}", config::VERSION),
        format!("- App ID: {}", config::APP_ID),
        format!(
            "- Sandboxed: {} {}",
            ashpd::is_sandboxed().await,
            std::env::var("container").unwrap_or_default()
        ),
        format!("\n##### OS Information\n```\n{}\n```", os_release().await),
    ]
    .join("\n")
}
