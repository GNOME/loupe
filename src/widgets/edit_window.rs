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
        pub(crate) save: TemplateChild<gtk::MenuButton>,
        #[template_child]
        saving_revealer: TemplateChild<gtk::Revealer>,
        #[template_child]
        saving_status: TemplateChild<gtk::Label>,
        #[template_child]
        saving_info: TemplateChild<gtk::Revealer>,
        #[template_child]
        shortcut_controller: TemplateChild<gtk::ShortcutController>,

        #[property(get, construct_only)]
        original_image: OnceCell<LpImage>,
        #[property(get, construct_only)]
        pub edit_crop: OnceCell<LpEditCrop>,

        pub(super) operations: RefCell<Option<Arc<Operations>>>,
        save_cancellable: RefCell<Option<gio::Cancellable>>,
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

            klass.install_action("edit.cancel", None, move |obj, _, _| {
                obj.imp().cancel();
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

            glib::spawn_future_local(glib::clone!(
                #[weak]
                obj,
                async move {
                    let can_trash = obj.original_image().can_trash().await;
                    obj.action_set_enabled("edit.save-overwrite", can_trash);
                }
            ));

            self.shortcut_controller.add_shortcut(gtk::Shortcut::new(
                Some(gtk::KeyvalTrigger::new(
                    gdk::Key::S,
                    gdk::ModifierType::CONTROL_MASK,
                )),
                Some(gtk::NamedAction::new("edit.save-overwrite")),
            ));

            self.shortcut_controller.add_shortcut(gtk::Shortcut::new(
                Some(gtk::KeyvalTrigger::new(
                    gdk::Key::S,
                    gdk::ModifierType::CONTROL_MASK.union(gdk::ModifierType::SHIFT_MASK),
                )),
                Some(gtk::NamedAction::new("edit.save-copy")),
            ));

            self.shortcut_controller.add_shortcut(gtk::Shortcut::new(
                Some(gtk::KeyvalTrigger::new(
                    gdk::Key::Escape,
                    gdk::ModifierType::NO_MODIFIER_MASK,
                )),
                Some(gtk::NamedAction::new("edit.cancel")),
            ));
        }
    }

    impl WidgetImpl for LpEditWindow {
        fn root(&self) {
            self.parent_root();

            let obj = self.obj();

            obj.edit_crop()
                .selection()
                .connect_cropped_notify(glib::clone!(
                    #[weak]
                    obj,
                    move |_| obj.imp().update_save_state(false)
                ));

            self.update_save_state(false);
        }
    }
    impl BinImpl for LpEditWindow {}

    impl LpEditWindow {
        fn cancel(&self) {
            let obj = self.obj();

            if let Some(cancellable) = obj.imp().save_cancellable.replace(None) {
                cancellable.cancel();
            }
            obj.window().show_image();
        }

        async fn save_copy(&self) {
            let obj = self.obj();
            self.save.popdown();

            if let Some(current_file) = obj.original_image().file() {
                let cancellable = gio::Cancellable::new();
                if let Some(old_cancellable) =
                    self.save_cancellable.replace(Some(cancellable.clone()))
                {
                    old_cancellable.cancel();
                }

                let file_dialog = gtk::FileDialog::new();

                let suggested_file = if let Some(path) = current_file.path() {
                    if let Some(basename) = path.file_stem() {
                        let mut suggested_file = None;
                        for i in 1..20 {
                            let mut new_path = path.clone();
                            let mut new_filename = basename.to_os_string();

                            if i == 1 {
                                // Translators: Filename suffix
                                new_filename.push(gettext(" (Edited)"));
                            } else {
                                // Translators: Filename suffix, {} is replaced by a number
                                new_filename.push(ngettext(" (Edited)", " (Edited {})", i));
                            }

                            if let Some(ext) = path.extension() {
                                new_filename.push(".");
                                new_filename.push(ext);
                            }

                            new_path.set_file_name(new_filename);

                            let file = gio::File::for_path(&new_path);
                            let file_exists = file
                                .query_info_future(
                                    gio::FILE_ATTRIBUTE_STANDARD_NAME,
                                    gio::FileQueryInfoFlags::NONE,
                                    glib::Priority::DEFAULT,
                                )
                                .await
                                .is_ok();

                            if !file_exists {
                                suggested_file = Some(file);
                                break;
                            }
                        }

                        suggested_file.unwrap_or_else(|| current_file.clone())
                    } else {
                        current_file.clone()
                    }
                } else {
                    current_file.clone()
                };

                file_dialog.set_initial_file(Some(&suggested_file));

                match file_dialog.save_future(Some(&obj.window())).await {
                    Err(err) => {
                        log::error!("{}", err);
                    }
                    Ok(new_file) => {
                        if self
                            .save(current_file, new_file.clone(), cancellable.clone())
                            .await
                        {
                            obj.window().show_specific_image(new_file);
                        }
                    }
                }
            }
        }

        async fn save_overwrite(&self) {
            let obj = self.obj();
            self.save.popdown();

            if let Some(original_file) = obj.original_image().file() {
                if let Some(current_path) = original_file.path() {
                    if let Some(mut file_stem) = current_path.file_stem().map(|x| x.to_os_string())
                    {
                        let mut tmp_path = current_path.clone();
                        file_stem.push(".tmp");
                        if let Some(ext) = current_path.extension() {
                            file_stem.push(".");
                            file_stem.push(ext);
                        }
                        tmp_path.set_file_name(file_stem);

                        let tmp_file = gio::File::for_path(&tmp_path);

                        let cancellable = gio::Cancellable::new();
                        if let Some(old_cancellable) =
                            self.save_cancellable.replace(Some(cancellable.clone()))
                        {
                            old_cancellable.cancel();
                        }

                        let written = self
                            .save(original_file.clone(), tmp_file.clone(), cancellable.clone())
                            .await;

                        if written {
                            log::debug!("Moving image to trash '{}'", original_file.uri());
                            let trash_result = gio::CancellableFuture::new(
                                original_file.trash_future(glib::Priority::DEFAULT),
                                cancellable.clone(),
                            )
                            .await;

                            if trash_result.is_err() {
                                // Canceled
                                log::debug!("Trashing image canceled");
                                return;
                            } else if let Ok(Err(err)) = trash_result {
                                obj.window().show_error(
                                    &gettext("Failed to save image."),
                                    &format!("Failed to move image {current_path:?} to trash and therefore couldn't save image: {err}"),
                                    ErrorType::General,
                                );
                                return;
                            }

                            log::debug!("Moving '{}' to '{}'", tmp_file.uri(), original_file.uri());
                            let move_result = gio::CancellableFuture::new(
                                tmp_file
                                    .move_future(
                                        &original_file,
                                        gio::FileCopyFlags::NONE,
                                        glib::Priority::DEFAULT,
                                    )
                                    .0,
                                cancellable.clone(),
                            )
                            .await;

                            if move_result.is_err() {
                                // Canceled
                                log::debug!("Moving image canceled");
                                return;
                            } else if let Ok(Err(err)) = move_result {
                                obj.window().show_error(
                                        &gettext("Failed to save image."),
                                        &format!(
                                            "Failed to move image image to {current_path:?}: {err} move {:?} to {:?}", tmp_file.path().unwrap(), original_file.path().unwrap()
                                        ),
                                        ErrorType::General,
                                    );
                                return;
                            }

                            obj.window().show_specific_image(original_file);
                        }
                    }
                }
            }
        }

        #[must_use]
        /// Saves image with operations applies
        ///
        /// Returns `true` if editing and saving was successful
        async fn save(
            &self,
            current_file: gio::File,
            new_file: gio::File,
            cancellable: gio::Cancellable,
        ) -> bool {
            let obj = self.obj();

            obj.edit_crop().apply_crop();

            self.set_saving_status(Some(gettext("Editing Image")));
            let mut editor = glycin::Editor::new(current_file);
            editor.cancellable(cancellable.clone());

            if let Some(operations) = obj.operations() {
                log::debug!("Computing edited image.");
                let result = match editor.edit().await {
                    Ok(editable_image) => editable_image.apply_complete(&operations).await,
                    Err(err) => Err(err),
                };
                match result {
                    Err(err) if matches!(err.error(), glycin::Error::Canceled(_)) => {
                        log::debug!("Computing edited image canceled");
                    }
                    Err(err) => {
                        log::warn!("Failed to edit image: {err}");
                        obj.window().show_error(
                            &gettext("Failed to edit image."),
                            &format!("Failed to edit image:\n\n{err}\n\n{operations:#?}"),
                            ErrorType::Loader,
                        )
                    }
                    Ok(edit) => {
                        log::debug!("Saving edited image to '{}'", new_file.uri());
                        match edit.data().get() {
                            Err(err) => {
                                obj.window().show_error(
                                    &gettext("Failed to Save Image"),
                                    &format!("Failed to get binary data: {err}"),
                                    ErrorType::General,
                                );
                            }
                            Ok(data) => {
                                self.set_saving_status(Some(gettext("Saving Image")));

                                let save_result = gio::CancellableFuture::new(
                                    new_file.replace_contents_future(
                                        data,
                                        None,
                                        true,
                                        gio::FileCreateFlags::NONE,
                                    ),
                                    cancellable,
                                )
                                .await;

                                if save_result.is_err() {
                                    log::debug!("Saving image canceled");
                                } else if let Ok(Err(err)) = save_result {
                                    obj.window().show_error(
                                        &gettext("Failed to Save Image"),
                                        &format!("Failed to write file:\n\n{err:?}"),
                                        ErrorType::General,
                                    );
                                } else {
                                    log::debug!("Image saved");
                                    return true;
                                }
                            }
                        }
                    }
                }
            }

            self.set_saving_status(None);
            false
        }

        fn set_saving_status(&self, message: Option<String>) {
            let is_saving = message.is_some();
            self.saving_status.set_label(&message.unwrap_or_default());
            self.saving_revealer.set_reveal_child(is_saving);
            self.update_save_state(is_saving);

            if is_saving {
                // Show text and spinner delayed
                glib::timeout_add_local_once(
                    std::time::Duration::from_millis(
                        self.saving_revealer.transition_duration().into(),
                    ),
                    glib::clone!(
                        #[weak(rename_to=imp)]
                        self,
                        move || {
                            imp.saving_info.set_reveal_child(true);
                        }
                    ),
                );
            } else {
                self.saving_info.set_reveal_child(false);
            }
        }

        fn is_save_sensitive(&self) -> bool {
            let obj = self.obj();

            self.operations
                .borrow()
                .as_ref()
                .is_some_and(|x| !x.operations().is_empty())
                || obj.edit_crop().selection().cropped()
        }

        pub fn update_save_state(&self, force_disabled: bool) {
            let obj = self.obj();
            let enabled = self.is_save_sensitive() && !force_disabled;

            self.save.set_sensitive(enabled);
            obj.action_set_enabled("edit.save-overwrite", enabled);
            obj.action_set_enabled("edit.save-copy", enabled);
        }
    }
}

glib::wrapper! {
    pub struct LpEditWindow(ObjectSubclass<imp::LpEditWindow>)
    @extends gtk::Widget, adw::Bin,
    @implements gtk::Buildable, gtk::Accessible, gtk::ConstraintTarget;
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
        imp.update_save_state(false);
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
