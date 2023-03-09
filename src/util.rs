use crate::deps::*;
use crate::i18n::*;

use anyhow::{anyhow, bail, Context};
use gio::prelude::*;
use once_cell::sync::Lazy;

use std::fmt::{Debug, Write};
use std::path::{Path, PathBuf};

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

pub fn compare_by_name(file_a: &Path, file_b: &Path) -> std::cmp::Ordering {
    let name_a = file_a.display().to_string();
    let name_b = file_b.display().to_string();

    let key_a = glib::FilenameCollationKey::from(name_a);
    let key_b = glib::FilenameCollationKey::from(name_b);

    key_a.cmp(&key_b)
}

static FILE_ATTRIBUTE_TRASH: Lazy<String> = Lazy::new(|| {
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
        let Some(file_info) = info.first()
            else { break; };

        let Some(original_path) = file_info.attribute_byte_string(gio::FILE_ATTRIBUTE_TRASH_ORIG_PATH).as_ref().map(PathBuf::from)
            else { break; };

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
                let Some(path) = original_file.path() else { bail!("File without path") };
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
                .context(i18n("Failed to restore image from trash"));

            break;
        }
    }

    error
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
