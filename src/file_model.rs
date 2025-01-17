// Copyright (c) 2022-2025 Sophie Herold
// Copyright (c) 2022 Christopher Davis
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

use std::cell::{OnceCell, RefCell};
use std::collections::VecDeque;
use std::sync::LazyLock;

use anyhow::Context;
use gio::prelude::*;
use gio::subclass::prelude::*;
use glib::subclass::Signal;
use glib::GString;
use indexmap::IndexMap;

use crate::deps::*;
use crate::util;
use crate::util::gettext::*;

#[derive(Debug, Clone)]
struct Entry {
    /// Determines the sort order
    sort: VecDeque<glib::FilenameCollationKey>,
    file: gio::File,
}

impl Entry {
    async fn new(file: gio::File) -> Self {
        let mut sort = VecDeque::new();
        let mut frac_file = file.clone();

        // Determine sort order for all path/uri segments one by one
        loop {
            match frac_file
                .query_info_future(
                    gio::FILE_ATTRIBUTE_STANDARD_DISPLAY_NAME,
                    gio::FileQueryInfoFlags::NONE,
                    glib::Priority::LOW,
                )
                .await
            {
                Ok(parent) => {
                    let key = glib::FilenameCollationKey::from(parent.display_name());
                    sort.push_front(key);
                }

                Err(err) => {
                    log::error!(
                        "Failed to obtain information for sorting '{}': {err}",
                        frac_file.uri()
                    );
                    break;
                }
            }

            if let Some(parent) = frac_file.parent() {
                frac_file = parent;
            } else {
                break;
            }
        }

        Self { sort, file }
    }
}

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct LpFileModel {
        /// Use and IndexMap such that we can put the elements into an arbitrary
        /// order while still having fast hash access via file URI.
        pub(super) files: RefCell<IndexMap<GString, Entry>>,
        pub(super) directory: OnceCell<gio::File>,
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
            static SIGNALS: LazyLock<Vec<Signal>> = LazyLock::new(|| {
                vec![Signal::builder("changed")
                    .param_types([glib::Type::VARIANT])
                    .build()]
            });
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
    pub async fn from_file(file: gio::File) -> Self {
        Self::from_files(vec![file]).await
    }

    pub async fn from_files(files: Vec<gio::File>) -> Self {
        let model = Self::default();

        let mut stored_files: IndexMap<GString, Entry> = IndexMap::new();
        for file in files {
            stored_files.insert(file.uri(), Entry::new(file).await);
        }

        *model.imp().files.borrow_mut() = stored_files;

        model
    }

    /// Load all file from the given directory
    ///
    /// This can be used if there are already files present
    pub async fn load_directory(&self, directory: gio::File) -> anyhow::Result<()> {
        self.imp().directory.set(directory.clone()).unwrap();

        let monitor =
            directory.monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE);
        match monitor {
            Ok(monitor) => {
                monitor.connect_changed(glib::clone!(
                    #[weak(rename_to = obj)]
                    self,
                    move |_monitor, file_a, file_b, event| {
                        obj.file_monitor_cb(event, file_a, file_b);
                    }
                ));
                self.imp().monitor.set(monitor).unwrap();
            }
            Err(err) => {
                log::info!("Cannot monitor directory: {err}");
            }
        }

        let mut new_files = IndexMap::new();

        let enumerator = directory
            .enumerate_children_future(
                &format!(
                    "{},{},{},{}",
                    gio::FILE_ATTRIBUTE_STANDARD_NAME,
                    gio::FILE_ATTRIBUTE_STANDARD_CONTENT_TYPE,
                    gio::FILE_ATTRIBUTE_STANDARD_FAST_CONTENT_TYPE,
                    gio::FILE_ATTRIBUTE_STANDARD_IS_HIDDEN,
                ),
                gio::FileQueryInfoFlags::NONE,
                glib::Priority::LOW,
            )
            .await
            .context(gettext("Could not list other files in directory."))?;

        loop {
            let info = enumerator.next_files_future(1, glib::Priority::LOW).await;
            match info {
                Err(err) => {
                    log::warn!("Unreadable entry in directory: {err}");
                    break;
                }
                Ok(info) => {
                    if let Some(info) = info.first() {
                        // GVfs smb does not provide a CONTENT_TYPE if the content type is
                        // ambiguous. This happens for png/apng. Since we
                        // only need to know if something is probably an
                        // image, we can use the FAST_CONTENT_TYPE in these cases.
                        let content_type = if info
                            .has_attribute(gio::FILE_ATTRIBUTE_STANDARD_CONTENT_TYPE)
                        {
                            info.content_type()
                        } else {
                            info.attribute_string(gio::FILE_ATTRIBUTE_STANDARD_FAST_CONTENT_TYPE)
                        };

                        if let Some(content_type) =
                            content_type.and_then(|x| gio::content_type_get_mime_type(&x))
                        {
                            // Filter out non-images types. The final decision if images are
                            // supported/kept will later be done by glycin when starting to load the
                            // images. Usually by inspecting the magic bytes.
                            if content_type.starts_with("image/") && !info.is_hidden() {
                                let file = directory.child(info.name());
                                new_files.insert(file.uri(), Entry::new(file).await);
                            }
                        }
                    } else {
                        break;
                    }
                }
            }
        }

        {
            // Here we use a nested scope so that the mutable borrow only lasts as long as
            // we need it

            let mut files = self.imp().files.borrow_mut();
            for (uri, file) in files.iter() {
                new_files.insert(uri.clone(), file.clone());
            }
            *files = new_files;
            // Then sort by name.
            Self::sort(&mut files);
        }

        Ok(())
    }

    pub fn directory(&self) -> Option<gio::File> {
        self.imp().directory.get().cloned()
    }

    pub fn index_of(&self, file: &gio::File) -> Option<usize> {
        self.imp().files.borrow().get_index_of(&file.uri())
    }

    pub fn n_files(&self) -> usize {
        self.imp().files.borrow().len()
    }

    pub fn contains_file(&self, file: &gio::File) -> bool {
        self.imp().files.borrow().contains_key(&file.uri())
    }

    pub fn before(&self, file: &gio::File) -> Option<gio::File> {
        let index = self.index_of(file)?;

        self.imp()
            .files
            .borrow()
            .get_index(index.checked_sub(1)?)
            .map(|x| &x.1.file)
            .cloned()
    }

    pub fn after(&self, file: &gio::File) -> Option<gio::File> {
        let index = self.index_of(file)?;

        self.imp()
            .files
            .borrow()
            .get_index(index.checked_add(1)?)
            .map(|x| &x.1.file)
            .cloned()
    }

    /// Returns `n` elements before and after given file
    pub fn files_around(&self, file: &gio::File, n: usize) -> IndexMap<GString, gio::File> {
        let Some(index) = self.index_of(file) else {
            log::error!("URI not in model: {}", file.uri());
            return IndexMap::new();
        };

        let reduce = n.saturating_sub(index);

        self.imp()
            .files
            .borrow()
            .iter()
            .skip(index.saturating_sub(n))
            .take(2 * n + 1 - reduce)
            .map(|(k, v)| (k.clone(), v.file.clone()))
            .collect()
    }

    /// Return first file
    pub fn first(&self) -> Option<gio::File> {
        self.imp()
            .files
            .borrow()
            .first()
            .map(|x| &x.1.file)
            .cloned()
    }

    /// Returns last file
    pub fn last(&self) -> Option<gio::File> {
        self.imp().files.borrow().last().map(|x| &x.1.file).cloned()
    }

    pub fn remove(&self, file: &gio::File) -> Option<gio::File> {
        self.imp()
            .files
            .borrow_mut()
            .shift_remove(&file.uri())
            .map(|x| x.file)
    }

    /// Currently sorts by name
    fn sort(files: &mut IndexMap<GString, Entry>) {
        files.sort_by(|_, x, _, y| x.sort.cmp(&y.sort));
    }

    /// Determines if a file is added
    fn is_image(info: &gio::FileInfo) -> bool {
        info.content_type()
            .map(|t| t.to_string())
            .filter(|t| t.starts_with("image/"))
            .is_some()
    }

    fn is_image_file(file: &gio::File) -> bool {
        let Ok(info) =
            util::query_attributes(file, vec![gio::FILE_ATTRIBUTE_STANDARD_CONTENT_TYPE])
        else {
            log::warn!("Could not query file info: {}", file.uri());
            return false;
        };

        Self::is_image(&info)
    }

    /// Signal that notifies about added/removed files
    pub fn connect_changed(&self, f: impl Fn(&FileEvent) + 'static) {
        self.connect_local("changed", false, move |args| {
            if let Some(file_event) = args
                .get(1)
                .and_then(|x| x.get().ok())
                .and_then(|x| FileEvent::from_variant(&x))
            {
                f(&file_event);
            } else {
                log::error!("Failed to read arguments from 'changed' signal.");
            }

            None
        });
    }

    pub fn emmit_changed(&self, file_event: &FileEvent) {
        self.emit_by_name::<()>("changed", &[&file_event.to_variant()]);
    }

    /// Insert a file
    async fn insert(&self, file: gio::File) -> bool {
        let obj = self.clone();
        let entry = Entry::new(file.clone()).await;
        let mut files = obj.imp().files.borrow_mut();
        let changed = files.insert(file.uri(), entry).is_none();
        if changed {
            Self::sort(&mut files);
            drop(files);
        }
        changed
    }

    fn file_monitor_cb(
        &self,
        event: gio::FileMonitorEvent,
        file_a: &gio::File,
        file_b: Option<&gio::File>,
    ) {
        let uri_a = file_a.uri();
        match event {
            gio::FileMonitorEvent::Created
            | gio::FileMonitorEvent::MovedIn
            | gio::FileMonitorEvent::ChangesDoneHint
                if Self::is_image_file(file_a) =>
            {
                // ^^^^
                // Changing file content could theoretically make it an image
                // by adding a magic byte

                log::debug!("File added: {uri_a}");

                glib::spawn_future_local(glib::clone!(
                    #[strong(rename_to=obj)]
                    self,
                    #[strong]
                    file_a,
                    async move {
                        let changed = obj.insert(file_a.clone()).await;
                        if changed {
                            obj.emmit_changed(&FileEvent::New(uri_a.to_string()));
                        }
                    }
                ));
            }
            gio::FileMonitorEvent::Deleted
            | gio::FileMonitorEvent::MovedOut
            | gio::FileMonitorEvent::Unmounted => {
                log::debug!("File removed: {}", file_a.uri());
                let removed = self
                    .imp()
                    .files
                    .borrow_mut()
                    .shift_remove(&file_a.uri())
                    .is_some();
                if removed {
                    self.emmit_changed(&FileEvent::Removed(file_a.uri().to_string()));
                }
            }
            gio::FileMonitorEvent::Renamed => {
                if let Some(file_b) = file_b {
                    {
                        let uri_b = file_b.uri();
                        log::debug!("File moved from '{uri_a}' to '{uri_b}'");
                        let mut changed =
                            self.imp().files.borrow_mut().shift_remove(&uri_a).is_some();

                        if Self::is_image_file(file_b) {
                            glib::spawn_future_local(glib::clone!(
                                #[strong(rename_to=obj)]
                                self,
                                #[strong]
                                file_b,
                                async move {
                                    changed |= obj.insert(file_b.clone()).await;

                                    if changed {
                                        obj.emmit_changed(&FileEvent::Moved(
                                            uri_a.to_string(),
                                            uri_b.to_string(),
                                        ));
                                    }
                                }
                            ));
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

#[derive(Debug, glib::Variant)]
pub enum FileEvent {
    New(String),
    Removed(String),
    Moved(String, String),
}
