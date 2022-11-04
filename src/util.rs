use crate::deps::*;

use gio::prelude::*;
use glib::translate::*;

use anyhow::Context;

use std::fmt::Write;

pub fn utf8_collate_key_for_filename(filename: &str) -> String {
    unsafe {
        from_glib_full(glib::ffi::g_utf8_collate_key_for_filename(
            filename.to_glib_none().0,
            filename.len() as isize,
        ))
    }
}

pub fn get_file_display_name(file: &gio::File) -> Option<String> {
    let info = query_attributes(file, vec![&gio::FILE_ATTRIBUTE_STANDARD_DISPLAY_NAME]).ok()?;

    Some(info.display_name().to_string())
}

pub fn query_attributes(file: &gio::File, attributes: Vec<&str>) -> anyhow::Result<gio::FileInfo> {
    let mut attributes = attributes;
    let mut attr_str = String::from(attributes.remove(0));
    for attr in attributes {
        write!(attr_str, ",{}", attr)?;
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
        write!(attr_str, ",{}", attr)?;
    }

    file.query_info_future(
        &attr_str,
        gio::FileQueryInfoFlags::empty(),
        glib::Priority::default(),
    )
    .await
    .context("Failed to query attributes")
}

pub fn compare_by_name(file_a: &gio::File, file_b: &gio::File) -> std::cmp::Ordering {
    let name_a = get_file_display_name(file_a).unwrap_or_default();
    let name_b = get_file_display_name(file_b).unwrap_or_default();

    utf8_collate_key_for_filename(&name_a).cmp(&utf8_collate_key_for_filename(&name_b))
}
