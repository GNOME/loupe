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
    let info = file
        .query_info(
            *gio::FILE_ATTRIBUTE_STANDARD_DISPLAY_NAME,
            gio::FileQueryInfoFlags::empty(),
            gio::Cancellable::NONE,
        )
        .ok()?;

    Some(info.display_name().to_string())
}
