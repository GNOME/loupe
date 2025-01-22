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

use std::cell::{OnceCell, RefCell};
use std::sync::Arc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use glycin::Operations;

use super::edit::LpEditCrop;
use super::LpImage;
use crate::deps::*;
use crate::util::gettext::*;
use crate::util::root::ParentWindow;
use crate::util::ErrorType;

mod imp {
    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::LpEditWindow)]
    #[template(file = "edit_window.ui")]
    pub struct LpEditWindow {
        #[template_child]
        toolbar_view: TemplateChild<adw::ToolbarView>,
        #[template_child]
        cancel: TemplateChild<gtk::Button>,
        #[template_child]
        pub(crate) done: TemplateChild<gtk::MenuButton>,

        #[property(get, construct_only)]
        original_image: OnceCell<LpImage>,
        #[property(get, construct_only)]
        pub edit_crop: OnceCell<LpEditCrop>,

        pub(super) operations: RefCell<Option<Arc<Operations>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpEditWindow {
        const NAME: &'static str = "LpEditWindow";
        type Type = super::LpEditWindow;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);

            klass.install_action("edit.save-copy", None, |obj, _, _| {
                glib::spawn_future_local(glib::clone!(
                    #[weak]
                    obj,
                    async move { obj.imp().save_copy().await }
                ));
            });

            klass.install_action("edit.save-overwrite", None, |obj, _, _| {
                glib::spawn_future_local(glib::clone!(
                    #[weak]
                    obj,
                    async move { obj.imp().save_overwrite().await }
                ));
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for LpEditWindow {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            self.cancel.connect_clicked(glib::clone!(
                #[weak]
                obj,
                move |_| {
                    obj.window().show_image();
                }
            ));

            glib::spawn_future_local(glib::clone!(
                #[weak]
                obj,
                async move {
                    let can_trash = obj.original_image().can_trash().await;
                    obj.action_set_enabled("edit.save-overwrite", can_trash);
                }
            ));
        }
    }

    impl WidgetImpl for LpEditWindow {
        fn root(&self) {
            self.parent_root();

            let obj = self.obj();

            obj.edit_crop().connect_cropped_notify(glib::clone!(
                #[weak]
                obj,
                move |_| obj.imp().done.set_sensitive(obj.is_done_sensitive())
            ));
        }
    }
    impl BinImpl for LpEditWindow {}

    impl LpEditWindow {
        async fn save_copy(&self) {
            let obj = self.obj();
            self.done.popdown();

            if let Some(current_file) = obj.original_image().file() {
                let file_dialog = gtk::FileDialog::new();

                file_dialog.set_initial_file(Some(&current_file));

                match file_dialog.save_future(Some(&obj.window())).await {
                    Err(err) => {
                        log::error!("{}", err);
                    }
                    Ok(new_file) => {
                        if self.save(current_file, new_file.clone()).await {
                            obj.window().show_specific_image(new_file);
                        }
                    }
                }
            }
        }

        async fn save_overwrite(&self) {
            let obj = self.obj();
            self.done.popdown();

            if let Some(current_file) = obj.original_image().file() {
                if let Some(current_path) = current_file.path() {
                    if let Some(mut file_stem) = current_path.file_stem().map(|x| x.to_os_string())
                    {
                        let mut tmp_path = current_path.clone();
                        file_stem.push(".tmp");
                        if let Some(ext) = current_path.extension() {
                            file_stem.push(".");
                            file_stem.push(ext);
                        }
                        tmp_path.set_file_name(file_stem);

                        let new_file = gio::File::for_path(&tmp_path);
                        let written = self.save(current_file.clone(), new_file.clone()).await;

                        if written {
                            if let Err(err) =
                                current_file.trash_future(glib::Priority::DEFAULT).await
                            {
                                obj.window().show_error(
                                    &gettext("Failed to save image."),
                                    &format!("Failed to move image {current_path:?} to trash and therefore couldn't save image: {err}"),
                                    ErrorType::General,
                                );
                            } else if let Err(err) = new_file
                                .move_future(
                                    &current_file,
                                    gio::FileCopyFlags::NONE,
                                    glib::Priority::DEFAULT,
                                )
                                .0
                                .await
                            {
                                obj.window().show_error(
                                        &gettext("Failed to save image."),
                                        &format!(
                                            "Failed to move image image to {current_path:?}: {err} move {:?} to {:?}", new_file.path().unwrap(), current_file.path().unwrap()
                                        ),
                                        ErrorType::General,
                                    );
                            } else {
                                obj.window().show_specific_image(current_file);
                            }
                        }
                    }
                }
            }
        }

        #[must_use]
        /// Saves image with operations applies
        ///
        /// Returns `true` if editing and saving was successful
        async fn save(&self, current_file: gio::File, new_file: gio::File) -> bool {
            let obj = self.obj();

            obj.edit_crop().apply_crop();

            let editor = glycin::Editor::new(current_file);
            if let Some(operations) = obj.operations() {
                log::debug!("Computing edited image.");
                let result = editor.apply_complete(&operations).await;
                match result {
                    Err(err) => {
                        log::warn!("Failed to edit image: {err}");
                        obj.window().show_error(
                            &gettext("Failed to edit image."),
                            &format!("Failed to edit image:\n\n{err}\n\n{operations:#?}"),
                            ErrorType::General,
                        )
                    }
                    Ok(binary_data) => {
                        log::debug!("Saving edited image to '{}'", new_file.uri());
                        match binary_data.get() {
                            Err(err) => {
                                obj.window().show_error(
                                    &gettext("Failed to Save Image"),
                                    &format!("Failed to get binary data: {err}"),
                                    ErrorType::General,
                                );
                            }
                            Ok(data) => {
                                if let Err(err) = new_file
                                    .replace_contents_future(
                                        data,
                                        None,
                                        true,
                                        gio::FileCreateFlags::NONE,
                                    )
                                    .await
                                {
                                    obj.window().show_error(
                                        &gettext("Failed to Save Image"),
                                        &format!("Failed to write file:\n\n{err:?}"),
                                        ErrorType::General,
                                    );
                                } else {
                                    return true;
                                }
                            }
                        }
                    }
                }
            }

            false
        }
    }
}

glib::wrapper! {
    pub struct LpEditWindow(ObjectSubclass<imp::LpEditWindow>)
    @extends gtk::Widget, adw::Bin;
}

impl LpEditWindow {
    pub const REQUIRED_OPERATIONS: &[glycin::OperationId] = &[
        glycin::OperationId::Clip,
        glycin::OperationId::MirrorHorizontally,
        glycin::OperationId::Rotate,
    ];

    pub fn new(image: LpImage) -> Self {
        glib::Object::builder()
            .property("original_image", image.clone())
            .property("edit_crop", LpEditCrop::new(image))
            .build()
    }

    pub fn operations(&self) -> Option<Arc<glycin::Operations>> {
        self.imp().operations.borrow().clone()
    }

    pub fn set_operations(&self, operations: Option<Arc<glycin::Operations>>) {
        let imp = self.imp();
        imp.operations.replace(operations);
        imp.done.set_sensitive(self.is_done_sensitive());
    }

    pub fn is_done_sensitive(&self) -> bool {
        self.imp()
            .operations
            .borrow()
            .as_ref()
            .is_some_and(|x| !x.operations().is_empty())
            || self.edit_crop().cropped()
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

        self.set_operations(Some(Arc::new(Operations::new(operations))));
    }
}
