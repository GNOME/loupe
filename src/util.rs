use crate::deps::*;

use anyhow::Context;
use gio::prelude::*;

use std::fmt::{Debug, Write};
use std::path::Path;

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

pub fn compare_by_name(file_a: &Path, file_b: &Path) -> std::cmp::Ordering {
    let name_a = file_a.display().to_string();
    let name_b = file_b.display().to_string();

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
