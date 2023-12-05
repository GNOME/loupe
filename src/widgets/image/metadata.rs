// Copyright (c) 2023 Sophie Herold
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

use super::*;
use crate::metadata;

impl LpImage {
    /// Image size of original image with EXIF rotation applied
    pub fn image_size(&self) -> (i32, i32) {
        let orientation = self.imp().metadata.borrow().orientation();
        if orientation.rotation.abs() == 90. || orientation.rotation.abs() == 270. {
            let (x, y) = self.original_dimensions();
            (y, x)
        } else {
            self.original_dimensions()
        }
    }

    pub(super) fn original_dimensions(&self) -> (i32, i32) {
        if let Some((width, height)) = self.imp().frame_buffer.load().original_dimensions() {
            (width as i32, height as i32)
        } else {
            (0, 0)
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
        let (width, height) = self.original_dimensions();

        let rotated = self.rotation().to_radians().sin().abs();

        ((1. - rotated) * width as f64 + rotated * height as f64) * zoom
    }

    pub fn image_height(&self, zoom: f64) -> f64 {
        let (width, height) = self.original_dimensions();

        let rotated = self.rotation().to_radians().sin().abs();

        ((1. - rotated) * height as f64 + rotated * width as f64) * zoom
    }

    pub fn dimension_details(&self) -> decoder::ImageDimensionDetails {
        self.imp().dimension_details.borrow().clone()
    }

    pub fn file(&self) -> Option<gio::File> {
        self.imp().file.borrow().clone()
    }

    pub fn metadata(&self) -> impl Deref<Target = Metadata> + '_ {
        self.imp().metadata.borrow()
    }

    pub fn connect_changed(&self, f: impl Fn() + 'static) {
        self.connect_local("metadata-changed", false, move |_| {
            f();
            None
        });
    }

    pub(super) fn emmit_metadata_changed(&self) {
        self.emit_by_name::<()>("metadata-changed", &[]);
    }

    pub(super) async fn reload_file_info(&self) {
        let imp = self.imp();

        if let Some(file) = self.file() {
            let file_info = metadata::FileInfo::new(&file).await;
            match file_info {
                Ok(file_info) => imp.metadata.borrow_mut().set_file_info(file_info),
                Err(err) => log::warn!("Failed to load file information: {err}"),
            }
            self.emmit_metadata_changed();
        }
    }
}
