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

pub mod preview;

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
        let editor = glycin::Editor::new(file.clone()).edit().await?;

        let mut list = Vec::new();
        self.operations.rcu(|current_operations| {
            list.clone_from(&(**current_operations));
            Vec::new()
        });

        let operations = Operations::new(list);
        let edit_instruction = editor.apply_sparse(&operations).await?;

        if edit_instruction.apply_to(file).await? == EditOutcome::Unchanged {
            log::warn!("Writing new files is not supported yet");
        }

        Ok(())
    }
}
