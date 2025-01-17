// Copyright (c) 2023-2025 Sophie Herold
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

use gufo::common::orientation::Rotation;

use super::*;
use crate::metadata;

impl imp::LpImage {
    pub(super) fn original_dimensions(&self) -> (i32, i32) {
        if let Some((width, height)) = self.frame_buffer.load().original_dimensions() {
            (width as i32, height as i32)
        } else {
            (0, 0)
        }
    }

    pub(super) fn untransformed_dimensions(&self) -> (i32, i32) {
        if let Some((width, height)) = self.overwrite_dimensions.get() {
            (width as i32, height as i32)
        } else {
            self.original_dimensions()
        }
    }

    /// Image width with current zoom factor and rotation
    ///
    /// During rotation it is an interpolated size that does not
    /// represent the actual size. The size returned might well be
    /// larger than what can actually be displayed within the widget.
    pub fn image_displayed_width(&self) -> f64 {
        self.image_width(self.applicable_zoom())
    }

    pub fn image_displayed_height(&self) -> f64 {
        self.image_height(self.applicable_zoom())
    }

    pub fn image_width(&self, zoom: f64) -> f64 {
        let (width, height) = self.untransformed_dimensions();

        let rotated = self.obj().rotation().to_radians().sin().abs();

        ((1. - rotated) * width as f64 + rotated * height as f64) * zoom
    }

    pub fn image_height(&self, zoom: f64) -> f64 {
        let (width, height) = self.untransformed_dimensions();

        let rotated = self.obj().rotation().to_radians().sin().abs();

        ((1. - rotated) * height as f64 + rotated * width as f64) * zoom
    }

    pub fn connect_changed(&self, f: impl Fn() + 'static) {
        self.obj()
            .connect_local("metadata-changed", false, move |_| {
                f();
                None
            });
    }

    pub(super) fn emmit_metadata_changed(&self) {
        self.obj().emit_by_name::<()>("metadata-changed", &[]);
    }

    pub(super) async fn reload_file_info(&self) {
        let obj = self.obj();

        if let Some(file) = obj.file() {
            let file_info = metadata::FileInfo::new(&file).await;
            match file_info {
                Ok(file_info) => self.metadata.borrow_mut().set_file_info(file_info),
                Err(err) => log::warn!("Failed to load file information: {err}"),
            }
            self.emmit_metadata_changed();
        }
    }

    pub(super) fn image_size_available(&self) -> bool {
        self.obj().image_size() > (0, 0)
    }
}

impl LpImage {
    pub fn file(&self) -> Option<gio::File> {
        self.imp().file.borrow().clone()
    }

    pub fn metadata(&self) -> impl Deref<Target = Metadata> + '_ {
        self.imp().metadata.borrow()
    }

    /// Image size of original image with EXIF rotation applied
    pub fn image_size(&self) -> (i32, i32) {
        let orientation = self.imp().metadata.borrow().orientation();
        if orientation.rotate() == Rotation::_90 || orientation.rotate() == Rotation::_270 {
            let (x, y) = self.imp().original_dimensions();
            (y, x)
        } else {
            self.imp().untransformed_dimensions()
        }
    }

    pub async fn can_trash(&self) -> bool {
        let Some(file) = self.file() else {
            return false;
        };

        if file.path().is_none() {
            // The trash portal only supports fds anyway
            return false;
        }

        let file_info = file
            .query_info_future(
                gio::FILE_ATTRIBUTE_ACCESS_CAN_TRASH,
                gio::FileQueryInfoFlags::NONE,
                glib::Priority::DEFAULT,
            )
            .await;

        match file_info {
            Err(err) => {
                log::error!(
                    "Failed to obtain CAN_TRASH attribute for '{}': {err}",
                    file.uri()
                );
                // Assume we can't trash if we don't have any infos
                false
            }
            Ok(file_info) => {
                file_info.has_attribute(gio::FILE_ATTRIBUTE_ACCESS_CAN_TRASH)
                    && file_info.boolean(gio::FILE_ATTRIBUTE_ACCESS_CAN_TRASH)
            }
        }
    }
}
