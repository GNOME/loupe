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

//! Shows an resizable cropping selection

use std::cell::{Cell, OnceCell};

use adw::prelude::*;
use adw::subclass::prelude::*;
use adw::{glib, gtk};
use glycin::Operation;

use crate::editing::preview::EditingError;
use crate::util::gettext::*;
use crate::util::root::ParentWindow;
use crate::util::ErrorType;
use crate::widgets::edit::LpEditCropSelection;
use crate::widgets::{LpEditWindow, LpImage};

/// Aspect ratio modes that can be selected
#[derive(Debug, Clone, Copy, Default, glib::Enum)]
#[enum_type(name = "LpAspectRatio")]
pub enum LpAspectRatio {
    #[default]
    Free,
    Original,
    /// 1.0
    Square,
    /// 1.25
    R5to4,
    /// 1.33
    R4to3,
    /// 1.5
    R3to2,
    /// 1.77
    R16to9,
}

#[derive(Debug, Clone, Copy, Default, glib::Enum)]
#[enum_type(name = "LpOrientation")]
pub enum LpOrientation {
    #[default]
    Landscape,
    Portrait,
}

mod imp {
    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::LpEditCrop)]
    #[template(file = "crop.ui")]
    pub struct LpEditCrop {
        #[template_child]
        image: TemplateChild<LpImage>,
        #[template_child]
        pub(super) selection: TemplateChild<LpEditCropSelection>,
        #[template_child]
        pub apply_crop: TemplateChild<gtk::Button>,

        #[property(get, set, builder(LpAspectRatio::default()))]
        aspect_ratio: Cell<LpAspectRatio>,
        #[property(get, set, builder(LpOrientation::default()))]
        orientation: Cell<LpOrientation>,

        #[property(get, construct_only)]
        original_image: OnceCell<LpImage>,
        #[property(get, construct_only)]
        edit_window: OnceCell<LpEditWindow>,

        #[property(get, set)]
        child: OnceCell<gtk::Widget>,

        /// Last selected crop area
        crop_area_image_coord: Cell<Option<(u32, u32, u32, u32)>>,

        last_allocation: Cell<(i32, i32, i32)>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpEditCrop {
        const NAME: &'static str = "LpEditCrop";
        type Type = super::LpEditCrop;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);

            klass.install_property_action("edit-crop.aspect-ratio", "aspect_ratio");
            klass.install_property_action("edit-crop.orientation", "orientation");
            klass.install_action("edit-crop.mirror-horizontally", None, |obj, _, _| {
                obj.imp().apply_mirror_horizontally();
            });
            klass.install_action("edit-crop.mirror-vertically", None, |obj, _, _| {
                obj.imp().apply_mirror_vertically()
            });
            klass.install_action("edit-crop.rotate-cw", None, |obj, _, _| {
                obj.imp().apply_rotate_cw()
            });
            klass.install_action("edit-crop.rotate-ccw", None, |obj, _, _| {
                obj.imp().apply_rotate_ccw()
            });
            klass.install_action("edit-crop.reset", None, |obj, _, _| {
                obj.handle_error(obj.imp().apply_reset())
            });
            klass.install_action("edit-crop.apply-crop", None, |obj, _, _| {
                obj.imp().apply_crop()
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for LpEditCrop {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = &*self.obj();

            obj.child().set_parent(obj);

            self.image.duplicate_from(&obj.original_image());

            obj.action_set_enabled("edit-crop.reset", false);

            // Selection changed notifications

            self.selection.connect_crop_x_notify(glib::clone!(
                #[weak]
                obj,
                move |_| {
                    obj.imp().selection_changed();
                }
            ));

            self.selection.connect_crop_y_notify(glib::clone!(
                #[weak]
                obj,
                move |_| {
                    obj.imp().selection_changed();
                }
            ));

            self.selection.connect_crop_width_notify(glib::clone!(
                #[weak]
                obj,
                move |_| {
                    obj.imp().selection_changed();
                }
            ));

            self.selection.connect_crop_height_notify(glib::clone!(
                #[weak]
                obj,
                move |_| {
                    obj.imp().selection_changed();
                }
            ));
        }

        fn dispose(&self) {
            self.obj().child().unparent();
        }
    }

    impl WidgetImpl for LpEditCrop {
        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            let obj = self.obj();

            let image = &self.image;

            obj.child().allocate(width, height, baseline, None);

            // Adjust position and size for crop selection after widget resize
            if self.last_allocation.get() != (width, height, baseline) {
                self.last_allocation.replace((width, height, baseline));

                let (x, y, width, height) = (
                    image.image_rendering_x(),
                    image.image_rendering_y(),
                    image.image_rendering_width(),
                    image.image_rendering_height(),
                );

                self.selection.ensure_initialized(x, y, width, height);
                self.selection.set_image_area(x, y, width, height);

                let (x, y, width, height) = self.crop_area_widget_coord();
                self.selection.set_crop_size(x, y, width, height);
            }
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            self.obj().child().measure(orientation, for_size)
        }
    }
    impl BinImpl for LpEditCrop {}

    impl LpEditCrop {
        fn selection_changed(&self) {
            let crop_area_image_coord = self.crop_area_image_coord();
            if self.selection.is_in_user_change() {
                self.crop_area_image_coord.replace(crop_area_image_coord);
            }

            let apply_sensitive = self.selection.is_cropped();
            self.apply_crop.set_visible(apply_sensitive);
        }

        fn crop_area_image_coord(&self) -> Option<(u32, u32, u32, u32)> {
            if !self.selection.is_cropped() {
                return None;
            }

            let (x1, y1) = self.image.widget_to_img_coord((
                self.image.image_rendering_x() + self.selection.crop_x() as f64,
                self.image.image_rendering_y() + self.selection.crop_y() as f64,
            ));
            let (x2, y2) = self.image.widget_to_img_coord((
                self.image.image_rendering_x()
                    + self.selection.crop_x() as f64
                    + self.selection.crop_width() as f64,
                self.image.image_rendering_y()
                    + self.selection.crop_y() as f64
                    + self.selection.crop_height() as f64,
            ));

            Some((
                x1.round() as u32,
                y1.round() as u32,
                (x2 - x1).round() as u32,
                (y2 - y1).round() as u32,
            ))
        }

        fn crop_area_widget_coord(&self) -> (f64, f64, f64, f64) {
            let size = self.image.image_size();
            let (x, y, w, h) =
                self.crop_area_image_coord
                    .get()
                    .unwrap_or((0, 0, size.0 as u32, size.1 as u32));
            let (x, y) = self.image.img_to_draw_coord((x as f64, y as f64));
            let (w, h) = self.image.img_to_draw_coord((w as f64, h as f64));

            (x.round(), y.round(), w.round(), h.round())
        }

        fn reset_selection(&self) {
            let image = &self.image;

            let (x, y, width, height) = (
                image.image_rendering_x(),
                image.image_rendering_y(),
                image.image_rendering_width(),
                image.image_rendering_height(),
            );
            self.selection.reset(x, y, width, height);
        }

        fn apply_crop(&self) {
            if let Some(crop) = self.crop_area_image_coord.get() {
                self.add_operation(Operation::Clip(crop));

                self.reset_selection();
            }
        }

        fn apply_mirror_horizontally(&self) {
            self.add_operation(Operation::MirrorHorizontally);

            self.reset_selection();
        }

        fn apply_mirror_vertically(&self) {
            self.add_operation(Operation::MirrorVertically);

            self.reset_selection();
        }

        fn apply_rotate_cw(&self) {
            self.add_operation(Operation::Rotate(gufo_common::orientation::Rotation::_270));

            self.reset_selection();
        }

        fn apply_rotate_ccw(&self) {
            self.add_operation(Operation::Rotate(gufo_common::orientation::Rotation::_90));

            self.reset_selection();
        }

        fn apply_reset(&self) -> Result<(), EditingError> {
            let obj = self.obj();

            self.image.set_operations(None)?;
            obj.edit_window().set_operations(None);
            self.reset_selection();

            self.obj().action_set_enabled("edit-crop.reset", false);

            Ok(())
        }

        fn add_operation(&self, operation: Operation) {
            let obj = self.obj();

            let edit_window = obj.edit_window();
            let previous_operiatons = edit_window.operations();
            edit_window.add_operation(operation);
            let res = self.image.set_operations(edit_window.operations());

            if res.is_err() {
                obj.handle_error(res);
                // Reset to set of hopefully working operations
                edit_window.set_operations(previous_operiatons);
            } else {
                obj.action_set_enabled("edit-crop.reset", true);
            }
        }
    }
}

glib::wrapper! {
    pub struct LpEditCrop(ObjectSubclass<imp::LpEditCrop>)
    @extends gtk::Widget, adw::Bin;
}

impl LpEditCrop {
    pub fn new(edit_window: LpEditWindow) -> Self {
        glib::Object::builder()
            .property("original_image", edit_window.original_image())
            .property("edit_window", edit_window)
            .build()
    }

    pub fn selection(&self) -> LpEditCropSelection {
        self.imp().selection.clone()
    }

    fn handle_error(&self, res: Result<(), EditingError>) {
        if let Err(err) = res {
            self.edit_window().window().show_error(
                &gettext("Failed to Edit Image"),
                &format!("Failed to edit image: {err}"),
                ErrorType::General,
            );
        }
    }
}
