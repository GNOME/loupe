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

use super::*;

impl LpImage {
    /// Drag and drop content provider
    pub fn content_provider(&self) -> Option<gdk::ContentProvider> {
        let file = self.file()?;
        let list = gdk::FileList::from_array(&[file]);
        Some(gdk::ContentProvider::for_value(&list.to_value()))
    }

    /// Returns a thumbnail of the displated image
    pub fn thumbnail(&self) -> Option<gdk::Paintable> {
        let imp = self.imp();

        let (width, height) = self.image_size();
        let long_side = i32::max(width, height);
        let orientation = self.metadata().orientation();

        let scale = f32::min(1., THUMBNAIL_SIZE / long_side as f32);
        let render_options = tiling::RenderOptions {
            scaling_filter: gsk::ScalingFilter::Trilinear,
            scaling: imp.scaling(),
            background_color: Some(imp.background_color()),
        };

        let snapshot = gtk::Snapshot::new();

        imp.snapshot_rotate_mirror(
            &snapshot,
            -(orientation.rotate().degrees() as f32),
            orientation.mirror(),
            scale as f64,
        );

        imp.frame_buffer
            .load()
            .add_to_snapshot(&snapshot, scale as f64, &render_options);

        snapshot.to_paintable(None)
    }
}
