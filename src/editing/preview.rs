use crate::deps::*;
use glycin::{Operation, Operations};
use gtk::prelude::*;
use gufo_common::orientation::Rotation;

pub fn apply_operations(
    node: gsk::RenderNode,
    width: u32,
    height: u32,
    operations: &Operations,
    snapshot: &gtk::Snapshot,
    scale: f64,
) -> Result<(u32, u32), EditingError> {
    let os = OperationsSnapshot::new(node, width, height, scale as f32);
    os.apply(operations, snapshot)
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

    fn apply(
        mut self,
        operations: &Operations,
        snapshot: &gtk::Snapshot,
    ) -> Result<(u32, u32), EditingError> {
        for operation in operations.operations() {
            let operation_result = match operation {
                Operation::Clip(rect) => self.clip(*rect),
                Operation::Rotate(rotation) => self.rotate(*rotation),
                Operation::MirrorHorizontally => self.mirror_horizontally(),
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

        snapshot.append_node(self.current_node);

        Ok((self.width, self.height))
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

    fn rotate(
        &mut self,
        rotation: gufo_common::orientation::Rotation,
    ) -> Result<(), OperationError> {
        let snapshot = gtk::Snapshot::new();

        let x_center = self.width as f32 * self.scale / 2.;
        let y_center = self.height as f32 * self.scale / 2.;

        snapshot.translate(&graphene::Point::new(y_center, x_center));
        snapshot.rotate(rotation.degrees() as f32);
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
