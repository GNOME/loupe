// Copyright (c) 2021-2023 Christopher Davis
// Copyright (c) 2022-2024 Sophie Herold
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

pub mod gettext;
pub mod root;

use std::ffi::CStr;
use std::fmt::{Debug, Write};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use anyhow::{anyhow, bail, Context};
use gio::prelude::*;

use crate::deps::*;
use crate::util::gettext::*;

/// Returns localized date + time format
pub fn datetime_fmt(datetime: &glib::DateTime) -> Option<String> {
    // Translators: This is the date and time format we use in metadata output etc.
    // The format has to follow <https://docs.gtk.org/glib/method.DateTime.format.html>
    // The default is already translated. Don't change if you are not sure what to
    // use.
    let datetime_format = gettext("%x %X");

    let local_datetime = datetime.to_local();

    let fmt = local_datetime
        .as_ref()
        .unwrap_or(datetime)
        .format(&datetime_format);

    if let Err(err) = &fmt {
        log::error!("Could not format DateTime with '{datetime_format}': {err}");
    }

    fmt.ok().map(|x| x.to_string())
}

pub fn get_file_display_name(file: &gio::File) -> Option<String> {
    let info = query_attributes(file, vec![gio::FILE_ATTRIBUTE_STANDARD_DISPLAY_NAME]).ok()?;

    Some(info.display_name().to_string())
}

pub fn query_attributes(file: &gio::File, attributes: Vec<&str>) -> anyhow::Result<gio::FileInfo> {
    let mut attributes = attributes;
    let mut attr_str = String::from(attributes.remove(0));
    for attr in attributes {
        write!(attr_str, ",{attr}")?;
    }

    file.query_info(
        &attr_str,
        gio::FileQueryInfoFlags::empty(),
        gio::Cancellable::NONE,
    )
    .context("Failed to query string")
}

pub async fn query_attributes_future(
    file: &gio::File,
    attributes: Vec<&str>,
) -> anyhow::Result<gio::FileInfo> {
    let mut attr_str = String::from(*attributes.first().context("No attributes")?);

    for attr in &attributes[1..] {
        write!(attr_str, ",{attr}")?;
    }

    file.query_info_future(
        &attr_str,
        gio::FileQueryInfoFlags::empty(),
        glib::Priority::default(),
    )
    .await
    .context("Failed to query attributes")
}

pub fn compare_by_name(name_a: &glib::GString, name_b: &glib::GString) -> std::cmp::Ordering {
    let key_a = glib::FilenameCollationKey::from(name_a);
    let key_b = glib::FilenameCollationKey::from(name_b);

    key_a.cmp(&key_b)
}

static FILE_ATTRIBUTE_TRASH: LazyLock<String> = LazyLock::new(|| {
    [
        gio::FILE_ATTRIBUTE_STANDARD_NAME.as_str(),
        gio::FILE_ATTRIBUTE_TRASH_ORIG_PATH.as_str(),
    ]
    .join(",")
});

/// Recover file from trash
///
/// This is based on Nautilus' implementation
/// <https://gitlab.gnome.org/GNOME/glib/-/issues/845>
pub async fn untrash(path: &Path) -> anyhow::Result<()> {
    let trash = gio::File::for_uri("trash:///");

    let enumerator = trash
        .enumerate_children_future(
            &FILE_ATTRIBUTE_TRASH,
            gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
            glib::Priority::default(),
        )
        .await?;

    let mut error = Err(anyhow!("Image not found in trash"));

    while let Ok(info) = enumerator
        .next_files_future(1, glib::Priority::default())
        .await
    {
        let Some(file_info) = info.first() else {
            break;
        };

        let Some(original_path) = file_info
            .attribute_byte_string(gio::FILE_ATTRIBUTE_TRASH_ORIG_PATH)
            .as_ref()
            .map(PathBuf::from)
        else {
            break;
        };

        if original_path == path {
            let trash_file = trash.child(file_info.name());
            let original_file = gio::File::for_path(original_path);
            let mut target_file = original_file.clone();

            // Find available filename if original is used
            for i in 1.. {
                if !target_file.query_exists(gio::Cancellable::NONE) {
                    break;
                }

                // Construct new name of the form "<filename> (i).<ext>"
                let Some(path) = original_file.path() else {
                    bail!("File without path")
                };
                let mut name = path
                    .file_stem()
                    .map(|x| x.to_os_string())
                    .unwrap_or_default();
                name.push(format!(" ({i})"));

                // Construct new path
                let mut new_path = path.clone();
                new_path.set_file_name(name);
                if let Some(ext) = path.extension() {
                    new_path.set_extension(ext);
                }

                target_file = gio::File::for_path(new_path);
            }

            error = trash_file
                .move_future(
                    &target_file,
                    gio::FileCopyFlags::NOFOLLOW_SYMLINKS,
                    glib::Priority::default(),
                )
                .0
                .await
                .context(gettext("Failed to restore image from trash"));

            break;
        }
    }

    error
}

#[derive(Debug, Clone, Copy)]
pub enum Position {
    First,
    Last,
}

#[derive(Debug, Clone, Copy)]
pub enum Direction {
    Back,
    Forward,
}

#[derive(Copy, Clone, Debug)]
pub enum Gesture {
    /// Rotate with threshold offset
    Rotate(f64),
    Scale,
}

fn srgb_linear(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// Relative luminance <https://www.w3.org/WAI/WCAG21/Understanding/contrast-minimum.html#dfn-relative-luminance>
fn relative_luminance(color: &gdk::RGBA) -> f32 {
    0.2126 * srgb_linear(color.red())
        + 0.7152 * srgb_linear(color.green())
        + 0.0722 * srgb_linear(color.blue())
}

/// Contrast ratio <https://www.w3.org/WAI/WCAG21/Understanding/contrast-minimum.html#dfn-contrast-ratio>
pub fn contrast_ratio(color1: &gdk::RGBA, color2: &gdk::RGBA) -> f32 {
    let la = relative_luminance(color1);
    let lb = relative_luminance(color2);
    let l1 = f32::max(la, lb);
    let l2 = f32::min(la, lb);

    (l1 + 0.05) / (l2 + 0.05)
}

#[derive(Default)]
pub struct LocaleSettings {
    pub decimal_point: Option<String>,
    pub thousands_sep: Option<String>,
}

pub fn locale_settings() -> LocaleSettings {
    unsafe {
        let lconv = libc::localeconv();
        let mut locale_settings = LocaleSettings::default();

        if lconv.is_null() {
            return locale_settings;
        }

        let lconv = *lconv;

        if !(lconv).decimal_point.is_null() {
            let s = CStr::from_ptr(lconv.decimal_point);
            locale_settings.decimal_point = s.to_str().map(|x| x.to_string()).ok();
        }

        if !lconv.thousands_sep.is_null() {
            let s = CStr::from_ptr(lconv.thousands_sep);
            locale_settings.thousands_sep = s.to_str().map(|x| x.to_string()).ok();
        }

        locale_settings
    }
}

pub enum ErrorType {
    Loader,
    General,
}
