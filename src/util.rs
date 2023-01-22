use crate::deps::*;

use gio::prelude::*;

use anyhow::Context;

use std::fmt::{Debug, Write};

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

    let key_a = glib::FilenameCollationKey::from(name_a);
    let key_b = glib::FilenameCollationKey::from(name_b);

    key_a.cmp(&key_b)
}

pub async fn spawn<T: Debug + Send + 'static>(
    name: &str,
    f: impl Fn() -> T + Send + 'static,
) -> async_std::io::Result<T> {
    log::trace!("Starting thread '{name}'");

    Ok(async_std::task::Builder::new()
        .name(name.to_string())
        .spawn(async_global_executor::spawn_blocking(f))?
        .await)
}
