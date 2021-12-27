use crate::deps::*;

use gio::prelude::*;
use glib::translate::*;

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

pub fn query_attributes(
    file: &gio::File,
    attributes: Vec<&str>,
) -> Result<gio::FileInfo, glib::Error> {
    let mut attributes = attributes;
    let mut attr_str = String::from(attributes.remove(0));
    for attr in attributes {
        attr_str.push_str(&format!(",{}", attr));
    }

    file.query_info(
        &attr_str,
        gio::FileQueryInfoFlags::empty(),
        gio::Cancellable::NONE,
    )
}
