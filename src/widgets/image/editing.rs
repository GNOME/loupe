use glycin::Operations;
use tiling::RenderOptions;

use super::*;

impl imp::LpImage {
    pub(super) fn apply_operations(
        &self,
        node: gsk::RenderNode,
        snapshot: &gtk::Snapshot,
    ) -> Result<(), crate::editing::preview::EditingError> {
        if let Some(operations) = self.operations.borrow().as_ref() {
            let (width, height) = self.original_dimensions();
            let new_dimensions = crate::editing::preview::apply_operations(
                node,
                width as u32,
                height as u32,
                operations,
                snapshot,
                self.applicable_zoom(),
            )?;

            self.overwrite_dimensions.set(Some(new_dimensions));
        }

        Ok(())
    }
}

impl LpImage {
    pub fn set_operations(&self, operations: Option<glycin::Operations>) {
        let imp = self.imp();

        if operations.is_none() {
            imp.overwrite_dimensions.set(None);
        }
        imp.operations.replace(operations);

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
            imp.apply_operations(node, &gtk::Snapshot::new());
        } else {
            log::error!("Render node is empty");
        }

        imp.configure_best_fit();
        self.queue_draw();
    }

    pub fn add_operation(&self, operation: glycin::Operation) {
        let imp = self.imp();

        let mut operations = imp
            .operations
            .borrow()
            .as_ref()
            .map(|x| x.operations().to_vec())
            .unwrap_or_default();

        operations.push(operation);

        self.set_operations(Some(Operations::new(operations)));
    }
}
