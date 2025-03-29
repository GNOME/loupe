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

use glycin::{Operation, Operations};
use gtk::prelude::*;
use gufo_common::orientation::Rotation;

use crate::deps::*;

pub fn apply_operations(
    node: gsk::RenderNode,
    width: u32,
    height: u32,
    operations: Option<&Operations>,
    snapshot: &gtk::Snapshot,
    scale: f64,
    initial_operations: &Operations,
) -> Result<(u32, u32), EditingError> {
    let mut os = OperationsSnapshot::new(node, width, height, scale as f32);

    os.apply(initial_operations)?;

    if let Some(operations) = operations {
        os.apply_and_commit(operations, snapshot)
    } else {
        os.apply_and_commit(&Operations::new(Vec::new()), snapshot)
    }
}

struct OperationsSnapshot {
    width: u32,
    height: u32,
    current_node: gsk::RenderNode,
    scale: f32,
}

impl OperationsSnapshot {
    fn new(node: gsk::RenderNode, width: u32, height: u32, scale: f32) -> Self {
        Self {
            width,
            height,
            current_node: node,
            scale,
        }
    }

    fn apply_and_commit(
        mut self,
        operations: &Operations,
        snapshot: &gtk::Snapshot,
    ) -> Result<(u32, u32), EditingError> {
        self.apply(operations)?;

        snapshot.append_node(self.current_node);

        Ok((self.width, self.height))
    }

    fn apply(&mut self, operations: &Operations) -> Result<(), EditingError> {
        for operation in operations.operations() {
            let operation_result = match operation {
                Operation::Clip(rect) => self.clip(*rect),
                Operation::MirrorHorizontally => self.mirror_horizontally(),
                Operation::MirrorVertically => self.mirror_vertically(),
                Operation::Rotate(rotation) => self.rotate(*rotation),
                unsupported_operation => {
                    return Err(EditingError::UnsupportedOperation(
                        unsupported_operation.clone(),
                    ));
                }
            };

            if let Err(err) = operation_result {
                match err {
                    OperationError::EmptyNode => {
                        return Err(EditingError::EmptyNode(operation.clone()))
                    }
                }
            }
        }

        Ok(())
    }

    fn clip(&mut self, (x, y, width, height): (u32, u32, u32, u32)) -> Result<(), OperationError> {
        let snapshot = gtk::Snapshot::new();

        self.width = width;
        self.height = height;

        let x = x as f32 * self.scale;
        let y = y as f32 * self.scale;
        let width = width as f32 * self.scale;
        let height = height as f32 * self.scale;

        snapshot.translate(&graphene::Point::new(-x, -y));
        snapshot.push_clip(&graphene::Rect::new(x, y, width, height));

        snapshot.append_node(self.current_node.clone());

        snapshot.pop();

        self.current_node = snapshot.to_node().ok_or(OperationError::EmptyNode)?;

        Ok(())
    }

    fn mirror_horizontally(&mut self) -> Result<(), OperationError> {
        let snapshot = gtk::Snapshot::new();

        snapshot.translate(&graphene::Point::new(self.width as f32 * self.scale, 0.));
        snapshot.scale(-1., 1.);
        snapshot.append_node(self.current_node.clone());

        self.current_node = snapshot.to_node().ok_or(OperationError::EmptyNode)?;

        Ok(())
    }

    fn mirror_vertically(&mut self) -> Result<(), OperationError> {
        let snapshot = gtk::Snapshot::new();

        snapshot.translate(&graphene::Point::new(0., self.height as f32 * self.scale));
        snapshot.scale(1., -1.);
        snapshot.append_node(self.current_node.clone());

        self.current_node = snapshot.to_node().ok_or(OperationError::EmptyNode)?;

        Ok(())
    }

    fn rotate(
        &mut self,
        rotation: gufo_common::orientation::Rotation,
    ) -> Result<(), OperationError> {
        let snapshot = gtk::Snapshot::new();

        let x_center = self.width as f32 * self.scale / 2.;
        let y_center = self.height as f32 * self.scale / 2.;

        if rotation == Rotation::_90 || rotation == Rotation::_270 {
            snapshot.translate(&graphene::Point::new(y_center, x_center));
        } else {
            snapshot.translate(&graphene::Point::new(x_center, y_center));
        }
        snapshot.rotate(-(rotation.degrees() as f32));
        snapshot.translate(&graphene::Point::new(-x_center, -y_center));

        snapshot.append_node(self.current_node.clone());

        self.current_node = snapshot.to_node().ok_or(OperationError::EmptyNode)?;

        if rotation == Rotation::_90 || rotation == Rotation::_270 {
            std::mem::swap(&mut self.width, &mut self.height);
        }

        Ok(())
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum EditingError {
    #[error("Node empty after the following operation: {0:?}")]
    EmptyNode(Operation),
    #[error("The following editing operation is not supported: {0:?}")]
    UnsupportedOperation(Operation),
}

#[derive(Debug, Clone, thiserror::Error)]
enum OperationError {
    #[error("Empty node")]
    EmptyNode,
}
