// Copyright (c) 2023-2024 Sophie Herold
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

use crate::deps::*;

const ATTRIBUTE_HOST_PATH: &str = "xattr::document-portal.host-path";

#[derive(Debug)]
pub struct FileInfo {
    pub(super) display_name: glib::GString,
    pub(super) file_size: Option<u64>,
    pub(super) mime_type: Option<glib::GString>,
    pub(super) created: Option<glib::DateTime>,
    pub(super) modified: Option<glib::DateTime>,
    pub(super) host_path: Option<glib::GString>,
}

impl FileInfo {
    pub async fn new(file: &gio::File) -> anyhow::Result<Self> {
        let file_info = crate::util::query_attributes_future(
            file,
            vec![
                gio::FILE_ATTRIBUTE_STANDARD_DISPLAY_NAME,
                gio::FILE_ATTRIBUTE_STANDARD_SIZE,
                gio::FILE_ATTRIBUTE_STANDARD_CONTENT_TYPE,
                gio::FILE_ATTRIBUTE_TIME_CREATED,
                gio::FILE_ATTRIBUTE_TIME_MODIFIED,
                ATTRIBUTE_HOST_PATH,
            ],
        )
        .await?;

        let size = file_info.size();
        let file_size = if size > 0 { Some(size as u64) } else { None };

        Ok(Self {
            display_name: file_info.display_name(),
            file_size,
            mime_type: file_info
                .content_type()
                .and_then(|x| gio::content_type_get_mime_type(&x)),
            created: file_info.creation_date_time(),
            modified: file_info.modification_date_time(),
            host_path: file_info.attribute_as_string(ATTRIBUTE_HOST_PATH),
        })
    }
}
