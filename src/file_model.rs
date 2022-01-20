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
use crate::util;

use gio::prelude::*;
use gio::subclass::prelude::*;

use once_cell::sync::OnceCell;

use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct LpFileModel {
        pub(super) inner: RefCell<Vec<gio::File>>,
        pub(super) directory: OnceCell<gio::File>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpFileModel {
        const NAME: &'static str = "LpFileModel";
        type Type = super::LpFileModel;
        type Interfaces = (gio::ListModel,);
    }

    impl ObjectImpl for LpFileModel {}

    impl ListModelImpl for LpFileModel {
        fn item_type(&self, _list_model: &Self::Type) -> glib::Type {
            gio::File::static_type()
        }

        fn n_items(&self, _list_model: &Self::Type) -> u32 {
            self.inner.borrow().len() as u32
        }

        fn item(&self, _list_model: &Self::Type, position: u32) -> Option<glib::Object> {
            self.inner
                .borrow()
                .get(position as usize)
                .map(|f| f.clone().upcast())
        }
    }
}

glib::wrapper! {
    pub struct LpFileModel(ObjectSubclass<imp::LpFileModel>) @implements gio::ListModel;
}

impl LpFileModel {
    pub fn from_directory(directory: &gio::File) -> Self {
        let model = glib::Object::new::<Self>(&[]).expect("Could not create LpFileModel");

        {
            // Here we use a nested scope so that the mutable borrow only lasts as long as we need it
            let mut vec = model.imp().inner.borrow_mut();

            let enumerator = directory
                .enumerate_children(
                    &format!(
                        "{},{}",
                        *gio::FILE_ATTRIBUTE_STANDARD_NAME,
                        *gio::FILE_ATTRIBUTE_STANDARD_CONTENT_TYPE
                    ),
                    gio::FileQueryInfoFlags::NONE,
                    gio::Cancellable::NONE,
                )
                .unwrap();

            // Filter out non-images; For now we support "all" image types.
            enumerator.for_each(|info| {
                if let Ok(info) = info {
                    if let Some(content_type) = info.content_type().map(|t| t.to_string()) {
                        if content_type.starts_with("image/") {
                            let name = info.name();
                            log::debug!("{:?} is an image, adding to the list", name);
                            vec.push(directory.resolve_relative_path(&name));
                        }
                    }
                }
            });

            // Then sort by name.
            vec.sort_by(|file_a, file_b| {
                let name_a = util::get_file_display_name(file_a).unwrap_or_default();
                let name_b = util::get_file_display_name(file_b).unwrap_or_default();

                util::utf8_collate_key_for_filename(&name_a)
                    .cmp(&util::utf8_collate_key_for_filename(&name_b))
            });

            model.imp().directory.set(directory.clone()).unwrap();
        }

        model
    }

    pub fn previous(&self, current: &gio::File) -> Option<gio::File> {
        let imp = self.imp();

        let vec = imp.inner.borrow();
        let index = vec.iter().position(|f| f.equal(current))?;

        vec.get(index.checked_sub(1)?).cloned()
    }

    pub fn next(&self, current: &gio::File) -> Option<gio::File> {
        let imp = self.imp();

        let vec = imp.inner.borrow();
        let index = vec.iter().position(|f| f.equal(current))?;

        vec.get(index + 1).cloned()
    }

    pub fn directory(&self) -> Option<gio::File> {
        self.imp().directory.get().cloned()
    }

    pub fn index_of(&self, file: &gio::File) -> Option<usize> {
        let imp = self.imp();
        let vec = imp.inner.borrow();
        vec.iter().position(|f| f.equal(file))
    }
}
