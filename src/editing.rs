use std::sync::Arc;

use arc_swap::ArcSwap;
use glycin::{EditOutcome, Operations};

use crate::deps::*;
use crate::widgets::LpImage;

/// Queue for image edit operations that still have to be applied.
///
/// A queue is needed since further operations might be triggered while an
/// editing operation is in progress.
#[derive(Clone, Default, Debug)]
pub struct Queue {
    operations: Arc<ArcSwap<Vec<glycin::Operation>>>,
    /// Lock for in progress image operation
    lock: Arc<async_lock::Mutex<()>>,
}

impl Queue {
    pub fn push(&self, operation: glycin::Operation) {
        self.operations.rcu(move |current_operations| {
            let mut new = (**current_operations).clone();
            new.push(operation.clone());
            new
        });
    }

    pub fn write_to_image(&self, image: &LpImage) {
        let queue = self.clone();
        let image = image.clone();
        glib::spawn_future_local(async move {
            let lock: async_lock::MutexGuard<()> = queue.lock.lock().await;
            if let Err(err) = queue.apply(image).await {
                log::error!("Err: {err}");
            }
            drop(lock);
        });
    }

    async fn apply(&self, image: LpImage) -> anyhow::Result<()> {
        let file = image.file().unwrap();
        #[allow(unused_mut)]
        let mut editor = glycin::Editor::new(file.clone());
        #[cfg(feature = "disable-glycin-sandbox")]
        editor.sandbox_mechanism(Some(glycin::SandboxMechanism::NotSandboxed));

        let mut list = Vec::new();
        self.operations.rcu(|current_operations| {
            list.clone_from(&(**current_operations));
            Vec::new()
        });

        let operations = Operations::new(list);
        let edit_instruction = editor.apply_sparse(operations).await?;

        if edit_instruction.apply_to(file).await? == EditOutcome::Unchanged {
            log::error!("Writing new files is not supported yet");
        }

        Ok(())
    }
}
