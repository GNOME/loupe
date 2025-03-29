// Copyright (c) 2024-2025 Sophie Herold
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

use glycin::Operations;
use tiling::RenderOptions;

use super::*;
use crate::editing::preview::EditingError;

impl imp::LpImage {
    pub(super) fn apply_operations(
        &self,
        node: gsk::RenderNode,
        snapshot: &gtk::Snapshot,
    ) -> Result<(), crate::editing::preview::EditingError> {
        let (width, height) = self.original_dimensions();

        let initial_operations = if let Some(orientation) = self.original_orientation.get() {
            Operations::new_orientation(orientation)
        } else {
            Operations::new(Vec::new())
        };

        let new_dimensions = crate::editing::preview::apply_operations(
            node,
            width as u32,
            height as u32,
            self.operations.borrow().as_ref().map(|x| x.as_ref()),
            snapshot,
            self.applicable_zoom(),
            &initial_operations,
        )?;

        self.overwrite_dimensions.set(Some(new_dimensions));

        Ok(())
    }
}

impl LpImage {
    pub fn set_operations(
        &self,
        operations: Option<Arc<glycin::Operations>>,
    ) -> Result<(), EditingError> {
        let imp = self.imp();

        imp.overwrite_dimensions.set(None);
        imp.operations.replace(operations);

        // Let dimension rewrite by calculating the new image
        let tmp_snapshot = gtk::Snapshot::new();
        imp.active_frame_buffer().add_to_snapshot(
            &tmp_snapshot,
            1.,
            &RenderOptions {
                background_color: None,
                scaling: 1.,
                scaling_filter: gsk::ScalingFilter::Nearest,
            },
        );

        if let Some(node) = tmp_snapshot.to_node() {
            imp.apply_operations(node, &gtk::Snapshot::new())?;
        } else {
            log::error!("Render node is empty");
        }

        imp.configure_best_fit();
        self.queue_draw();

        Ok(())
    }
}
