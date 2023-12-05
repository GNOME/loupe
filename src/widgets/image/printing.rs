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

impl LpImage {
    pub fn print_data(&self, scale: f64) -> Option<gdk::Texture> {
        let orientation = self.metadata().orientation();

        let render_options = tiling::RenderOptions {
            scaling_filter: gsk::ScalingFilter::Trilinear,
            background_color: None,
            scale_factor: 1,
        };

        let snapshot = gtk::Snapshot::new();

        self.snapshot_rotate_mirror(
            &snapshot,
            -orientation.rotation as f32,
            orientation.mirrored,
            scale,
        );

        self.imp()
            .frame_buffer
            .load()
            .add_to_snapshot(&snapshot, scale, &render_options);

        let node = snapshot.to_node()?;
        let renderer = self.root()?.renderer();

        Some(renderer.render_texture(&node, None))
    }
}
