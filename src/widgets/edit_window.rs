// Copyright (c) 2024 Sophie Herold
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
use super::{LpImage, LpWindow};
use crate::deps::*;
use crate::util::gettext::*;

mod imp {

    use super::*;
    use crate::widgets::error_details::ErrorType;

    #[derive(Debug, Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::LpEditWindow)]
    #[template(file = "edit_window.ui")]
    pub struct LpEditWindow {
        #[template_child]
        toolbar_view: TemplateChild<adw::ToolbarView>,
        #[template_child]
        cancel: TemplateChild<gtk::Button>,
        #[template_child]
        save: TemplateChild<gtk::Button>,

        #[property(get, construct_only)]
        window: OnceCell<LpWindow>,
        #[property(get, construct_only)]
        original_image: OnceCell<LpImage>,

        pub(super) operations: RefCell<Option<Arc<Operations>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpEditWindow {
        const NAME: &'static str = "LpEditWindow";
        type Type = super::LpEditWindow;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
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

            self.toolbar_view
                .set_content(Some(&LpEditCrop::new(obj.to_owned())));

            self.cancel.connect_clicked(glib::clone!(
                #[weak]
                obj,
                move |_| {
                    obj.window().show_image();
                }
            ));

            self.save.connect_clicked(glib::clone!(
                #[weak]
                obj,
                move |_| {
                    glib::spawn_future_local(async move { obj.imp().save_image().await });
                }
            ));
        }
    }

    impl WidgetImpl for LpEditWindow {}
    impl BinImpl for LpEditWindow {}

    impl LpEditWindow {
        async fn save_image(&self) {
            let obj = self.obj();

            if let Some(current_file) = obj.original_image().file() {
                let file_dialog = gtk::FileDialog::new();

                file_dialog.set_initial_file(Some(&current_file));

                match file_dialog.save_future(Some(&obj.window())).await {
                    Err(err) => {
                        log::error!("{}", err);
                    }
                    Ok(new_file) => {
                        let editor = glycin::Editor::new(current_file);
                        if let Some(operations) = obj.operations() {
                            log::debug!("Computing edited image.");
                            let result = editor.apply_complete(&operations).await;
                            match result {
                                Err(err) => {
                                    log::warn!("Failed to edit image: {err}");
                                    obj.window().show_error(
                                        &gettext("Failed to edit image."),
                                        &format!(
                                            "Failed to edit image:\n\n{err}\n\n{operations:#?}"
                                        ),
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
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

glib::wrapper! {
    pub struct LpEditWindow(ObjectSubclass<imp::LpEditWindow>)
    @extends gtk::Widget, adw::Bin;
}

impl LpEditWindow {
    pub fn new(window: LpWindow, image: LpImage) -> Self {
        glib::Object::builder()
            .property("window", window)
            .property("original_image", image)
            .build()
    }

    pub fn operations(&self) -> Option<Arc<glycin::Operations>> {
        self.imp().operations.borrow().clone()
    }

    pub fn set_operations(&self, operations: Option<Arc<glycin::Operations>>) {
        let imp = self.imp();

        imp.operations.replace(operations);
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
