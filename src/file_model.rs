// file_model.rs
//
// Copyright 2022 Christopher Davis <christopherdavis@gnome.org>
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

use crate::deps::*;
use crate::i18n::*;
use crate::util;

use std::path::{Path, PathBuf};

use anyhow::Context;
use gio::prelude::*;
use gio::subclass::prelude::*;
use indexmap::IndexSet;
use once_cell::sync::{Lazy, OnceCell};
use std::cell::RefCell;

mod imp {
    use super::*;
    use gtk::glib::subclass::Signal;

    #[derive(Debug, Default)]
    pub struct LpFileModel {
        /// Use and IndexSet such that we can put the elements into an arbitrary order
        /// while still having fast hash access via `PathBuf`.
        pub(super) files: RefCell<IndexSet<PathBuf>>,
        pub(super) directory: OnceCell<PathBuf>,
        /// Track file changes.
        pub(super) monitor: OnceCell<gio::FileMonitor>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpFileModel {
        const NAME: &'static str = "LpFileModel";
        type Type = super::LpFileModel;
    }

    impl ObjectImpl for LpFileModel {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> =
                Lazy::new(|| vec![Signal::builder("changed").build()]);
            SIGNALS.as_ref()
        }
    }
}

glib::wrapper! {
    pub struct LpFileModel(ObjectSubclass<imp::LpFileModel>);
}

impl Default for LpFileModel {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl LpFileModel {
    /// Create with a single element
    pub fn from_path(path: &Path) -> Self {
        let model = Self::default();

        {
            let mut vec = model.imp().files.borrow_mut();
            vec.insert(path.to_path_buf());
        }

        model
    }

    /// Load all file from the given directory
    ///
    /// This can be used if there are already files present
    pub async fn load_directory(&self, directory: PathBuf) -> anyhow::Result<()> {
        self.imp().directory.set(directory.clone()).unwrap();

        let monitor = gio::File::for_path(&directory)
            .monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)?;

        monitor.connect_changed(
            glib::clone!(@weak self as obj => move |_monitor, file_a, file_b, event| {
                obj.file_monitor_cb(event, file_a, file_b);
            }),
        );
        self.imp().monitor.set(monitor).unwrap();

        let new_files_result = gio::spawn_blocking(move || {
            let mut files = IndexSet::new();

            let enumerator = gio::File::for_path(&directory)
                .enumerate_children(
                    &format!(
                        "{},{}",
                        gio::FILE_ATTRIBUTE_STANDARD_NAME,
                        gio::FILE_ATTRIBUTE_STANDARD_CONTENT_TYPE
                    ),
                    gio::FileQueryInfoFlags::NONE,
                    gio::Cancellable::NONE,
                )
                .context(i18n("Could not list other files in directory."))?;

            enumerator.for_each(|info| {
                if let Ok(info) = info {
                    if let Some(content_type) = info.content_type().map(|t| t.to_string()) {
                        // Filter out non-images; For now we support "all" image types.
                        if content_type.starts_with("image/") {
                            let path = PathBuf::from_iter([directory.clone(), info.name()]);
                            log::debug!("{path:?} is an image, adding to the list");
                            files.insert(path);
                        }
                    }
                }
            });

            Ok::<_, anyhow::Error>(files)
        })
        .await;

        let Ok(new_files) = new_files_result else {
            log::debug!("Thread listing directory canceled.");
            return Ok(());
        };

        let mut new_files = new_files?;

        {
            // Here we use a nested scope so that the mutable borrow only lasts as long as we need it

            let mut files = self.imp().files.borrow_mut();
            for path in files.iter() {
                new_files.insert(path.clone());
            }
            *files = new_files;
            // Then sort by name.
            Self::sort(&mut files);
        }

        Ok(())
    }

    pub fn directory(&self) -> Option<PathBuf> {
        self.imp().directory.get().cloned()
    }

    pub fn index_of(&self, path: &Path) -> Option<usize> {
        self.imp().files.borrow().get_index_of(path)
    }

    pub fn n_files(&self) -> usize {
        self.imp().files.borrow().len()
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.imp().files.borrow().contains(path)
    }

    pub fn before(&self, path: &Path) -> Option<PathBuf> {
        let index = self.index_of(path)?;

        self.imp()
            .files
            .borrow()
            .get_index(index.checked_sub(1)?)
            .cloned()
    }

    pub fn after(&self, path: &Path) -> Option<PathBuf> {
        let index = self.index_of(path)?;

        self.imp()
            .files
            .borrow()
            .get_index(index.checked_add(1)?)
            .cloned()
    }

    /// Returns `n` elements before and after given path
    pub fn files_around(&self, path: &Path, n: usize) -> IndexSet<PathBuf> {
        let Some(index) = self.index_of(path) else {
            log::error!("Path not in model: {path:?}");
            return IndexSet::new();
        };

        let reduce = if index < n { n - index } else { 0 };

        self.imp()
            .files
            .borrow()
            .iter()
            .skip(index.saturating_sub(n))
            .take(2 * n + 1 - reduce)
            .cloned()
            .collect()
    }

    /// Return first path
    pub fn first(&self) -> Option<PathBuf> {
        self.imp().files.borrow().first().cloned()
    }

    /// Returns last path
    pub fn last(&self) -> Option<PathBuf> {
        self.imp().files.borrow().last().cloned()
    }

    /// Currently sorts by name
    fn sort(files: &mut IndexSet<PathBuf>) {
        files.sort_by(|x, y| util::compare_by_name(x, y));
    }

    /// Determines if a file is added
    fn is_image(info: &gio::FileInfo) -> bool {
        info.content_type()
            .map(|t| t.to_string())
            .filter(|t| t.starts_with("image/"))
            .is_some()
    }

    fn is_image_file(file: &gio::File) -> bool {
        let Ok(info) = util::query_attributes(file, vec![gio::FILE_ATTRIBUTE_STANDARD_CONTENT_TYPE]) else {
            log::warn!("Could not query file info: {:?}", file.path());
            return false;
        };

        Self::is_image(&info)
    }

    /// Signal that notifies about added/removed files
    pub fn connect_changed(&self, f: impl Fn() + 'static) {
        self.connect_local("changed", false, move |_| {
            f();
            None
        });
    }

    fn file_monitor_cb(
        &self,
        event: gio::FileMonitorEvent,
        file_a: &gio::File,
        file_b: Option<&gio::File>,
    ) {
        let Some(path_a) = file_a.path() else {
            log::error!("File has no path: {event}");
            return;
        };
        let path_b = file_b.and_then(|x| x.path());

        let changed = match event {
            gio::FileMonitorEvent::Created
            | gio::FileMonitorEvent::MovedIn
            // Changing file content could theoretically make it an image
            // by adding a magic byte
            | gio::FileMonitorEvent::ChangesDoneHint
                if Self::is_image_file(file_a) =>
            {
                let mut files = self.imp().files.borrow_mut();
                let changed = files.insert(path_a);
                if changed {
                    Self::sort(&mut files);
                }
                changed
            }
            gio::FileMonitorEvent::Deleted | gio::FileMonitorEvent::MovedOut | gio::FileMonitorEvent::Unmounted => {
                let mut files = self.imp().files.borrow_mut();
                let changed = files.remove(&path_a);
                if changed {
                    Self::sort(&mut files);
                }
                changed
            }
            gio::FileMonitorEvent::Renamed => {
                if let (Some(path_b), Some(file_b)) = (path_b, file_b) {
                    let mut changed = false;
                    {
                        let mut files = self.imp().files.borrow_mut();
                        changed |= files.remove(&path_a);
                        if Self::is_image_file(file_b) {
                            changed |= files.insert(path_b);
                            if changed {
                                Self::sort(&mut files);
                            }
                        }
                    }
                    changed
                } else {
                    false
                }
            }
            _ => false,
        };

        if changed {
            self.emit_by_name::<()>("changed", &[]);
        }
    }
}
