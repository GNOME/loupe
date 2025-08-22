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

use std::cell::{Cell, OnceCell};

use adw::gtk::graphene;
use adw::prelude::*;
use adw::subclass::prelude::*;
use adw::{glib, gtk};

use super::crop::{LpAspectRatio, LpEditCrop, LpOrientation};
use crate::deps::*;
use crate::widgets::LpImage;

const MIN_SELECTION_SIZE: f32 = 80.;

#[derive(Debug, Clone, Copy)]
struct InResize {
    handle: RectHandle,
    initial_selection: graphene::Rect,
}

#[derive(Debug, Clone, Copy)]
struct InMove {
    initial_selection: graphene::Rect,
}

/// Corners and edges of a rectangle
#[derive(Debug, Clone, Copy)]
enum RectHandle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

#[derive(Debug, Clone, Copy)]
enum HEdge {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy)]
enum VEdge {
    Top,
    Bottom,
}

impl RectHandle {
    fn cursor_name(&self) -> &str {
        match self {
            Self::TopLeft => "nw-resize",
            Self::Top => "n-resize",
            Self::TopRight => "ne-resize",
            Self::Right => "e-resize",
            Self::BottomRight => "se-resize",
            Self::Bottom => "s-resize",
            Self::BottomLeft => "sw-resize",
            Self::Left => "w-resize",
        }
    }

    fn h_edge(&self) -> Option<HEdge> {
        match self {
            Self::TopLeft => Some(HEdge::Left),
            Self::Top => None,
            Self::TopRight => Some(HEdge::Right),
            Self::Right => Some(HEdge::Right),
            Self::BottomRight => Some(HEdge::Right),
            Self::Bottom => None,
            Self::BottomLeft => Some(HEdge::Left),
            Self::Left => Some(HEdge::Left),
        }
    }

    fn v_edge(&self) -> Option<VEdge> {
        match self {
            Self::TopLeft => Some(VEdge::Top),
            Self::Top => Some(VEdge::Top),
            Self::TopRight => Some(VEdge::Top),
            Self::Right => None,
            Self::BottomRight => Some(VEdge::Bottom),
            Self::Bottom => Some(VEdge::Bottom),
            Self::BottomLeft => Some(VEdge::Bottom),
            Self::Left => None,
        }
    }
}

impl LpAspectRatio {
    fn frac(&self) -> Option<(u32, u32)> {
        match self {
            Self::Free => None,
            Self::Original => None,
            Self::Square => Some((1, 1)),
            Self::R5to4 => Some((5, 4)),
            Self::R4to3 => Some((4, 3)),
            Self::R3to2 => Some((3, 2)),
            Self::R16to9 => Some((16, 9)),
        }
    }

    fn num(&self, crop: &LpEditCropSelection) -> Option<f32> {
        let (x, y) = if matches!(self, Self::Original) {
            let (width, height) = crop.image().image_size();
            let (width, height) = (width as u32, height as u32);

            if width > height {
                (width, height)
            } else {
                (height, width)
            }
        } else {
            self.frac()?
        };

        let result = match crop.orientation() {
            LpOrientation::Landscape => x as f32 / y as f32,
            LpOrientation::Portrait => y as f32 / x as f32,
        };

        Some(result)
    }
}

trait RectExt: Into<graphene::Rect> {
    /// Gives the position and sizes as tuple
    fn into_tuple(self) -> (f32, f32, f32, f32) {
        let rect: graphene::Rect = self.into();

        (rect.x(), rect.y(), rect.width(), rect.height())
    }
}

impl RectExt for graphene::Rect {}

mod imp {
    use super::*;
    use crate::widgets::LpImage;

    #[derive(Debug, Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::LpEditCropSelection)]
    #[template(file = "crop_selection.ui")]
    pub struct LpEditCropSelection {
        #[template_child]
        pub(super) space_top: TemplateChild<adw::Bin>,
        #[template_child]
        pub(super) space_right: TemplateChild<adw::Bin>,
        #[template_child]
        pub(super) space_bottom: TemplateChild<adw::Bin>,
        #[template_child]
        pub(super) space_left: TemplateChild<adw::Bin>,
        #[template_child]
        pub(super) selection: TemplateChild<gtk::Grid>,

        #[template_child]
        handle_top_left: TemplateChild<adw::Bin>,
        #[template_child]
        handle_top: TemplateChild<adw::Bin>,
        #[template_child]
        handle_top_right: TemplateChild<adw::Bin>,
        #[template_child]
        handle_right: TemplateChild<adw::Bin>,
        #[template_child]
        handle_bottom_right: TemplateChild<adw::Bin>,
        #[template_child]
        handle_bottom: TemplateChild<adw::Bin>,
        #[template_child]
        handle_bottom_left: TemplateChild<adw::Bin>,
        #[template_child]
        handle_left: TemplateChild<adw::Bin>,

        #[template_child]
        apply_button: TemplateChild<gtk::Button>,
        #[template_child]
        apply_button_click: TemplateChild<gtk::GestureClick>,

        #[template_child]
        selection_move: TemplateChild<gtk::GestureDrag>,

        #[template_child]
        selection_overlay: TemplateChild<gtk::Overlay>,

        /// Set while in drag gesture for changing crop area
        pub(super) selection_in_resize: Cell<Option<InResize>>,
        /// Set while in drag gesture for moving crop area
        pub(super) selection_in_move: Cell<Option<InMove>>,
        /// Set while resetting the crop area
        pub(super) selection_in_reset: Cell<bool>,

        // Animates changes between different fixed aspect ratios
        aspect_ratio_animation: OnceCell<adw::TimedAnimation>,

        #[property(get, set=Self::set_aspect_ratio, builder(LpAspectRatio::default()))]
        aspect_ratio: Cell<LpAspectRatio>,
        #[property(get, set=Self::set_orientation, builder(LpOrientation::default()))]
        orientation: Cell<LpOrientation>,

        /// Last selected crop area
        crop_area_image_coord: Cell<Option<(u32, u32, u32, u32)>>,

        #[property(get)]
        cropped: Cell<bool>,

        #[property(get, set=Self::set_crop_x, explicit_notify)]
        crop_x: Cell<f32>,
        #[property(get, set=Self::set_crop_y, explicit_notify)]
        crop_y: Cell<f32>,
        #[property(get, set=Self::set_crop_width, explicit_notify)]
        crop_width: Cell<f32>,
        #[property(get, set=Self::set_crop_height, explicit_notify)]
        crop_height: Cell<f32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpEditCropSelection {
        const NAME: &'static str = "LpEditCropSelection";
        type Type = super::LpEditCropSelection;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            klass.set_css_name("lpcrop");

            klass.install_property_action("edit-crop.aspect-ratio", "aspect_ratio");
            klass.install_property_action("edit-crop.orientation", "orientation");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for LpEditCropSelection {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj().to_owned();

            obj.set_direction(gtk::TextDirection::Ltr);
            self.selection.set_direction(gtk::TextDirection::Ltr);
            self.handle_bottom_left
                .set_direction(gtk::TextDirection::Ltr);
            self.handle_bottom_right
                .set_direction(gtk::TextDirection::Ltr);
            self.handle_top_left.set_direction(gtk::TextDirection::Ltr);
            self.handle_top_right.set_direction(gtk::TextDirection::Ltr);

            // List of widgets that are drag handles
            let drag_handles = std::collections::BTreeMap::from([
                (self.handle_top_left.clone(), RectHandle::TopLeft),
                (self.handle_top.clone(), RectHandle::Top),
                (self.handle_top_right.clone(), RectHandle::TopRight),
                (self.handle_right.clone(), RectHandle::Right),
                (self.handle_bottom_right.clone(), RectHandle::BottomRight),
                (self.handle_bottom.clone(), RectHandle::Bottom),
                (self.handle_bottom_left.clone(), RectHandle::BottomLeft),
                (self.handle_left.clone(), RectHandle::Left),
            ]);

            // Set cursor style for each corner and edge
            for (handle_widget, handle) in drag_handles.iter() {
                handle_widget.set_cursor_from_name(Some(handle.cursor_name()));
            }

            self.selection.set_cursor_from_name(Some("move"));
            self.apply_button.set_cursor_from_name(Some("default"));

            // Gesture bug workaround
            self.apply_button_click.connect_begin(move |gesture, _| {
                gesture.set_state(gtk::EventSequenceState::Claimed);
            });

            self.apply_button_click.connect_released(glib::clone!(
                #[weak]
                obj,
                move |_, _, x, y| {
                    if obj.imp().apply_button.contains(x, y) {
                        let _ = obj.activate_action("edit-crop.apply-crop", None);
                    }
                }
            ));

            // Drag begin
            self.selection_move.connect_drag_begin(glib::clone!(
                #[weak]
                obj,
                move |gesture, _, _| {
                    let imp = obj.imp();

                    imp.aspect_ratio_animation().pause();

                    let hovered_widget = gesture
                        .start_point()
                        .and_then(|(x, y)| obj.pick(x, y, gtk::PickFlags::DEFAULT));

                    // Lookup if the start-point of the gesture is above a drag handle widget and
                    // what corner/edge it is
                    if let Some(handle) = hovered_widget
                        .as_ref()
                        .and_then(|x| drag_handles.get(x).copied())
                    {
                        gesture.set_state(gtk::EventSequenceState::Claimed);

                        let crop_area = imp.crop_area();

                        imp.set_selection_in_resize(Some(InResize {
                            handle,
                            initial_selection: crop_area,
                        }));
                    } else if hovered_widget.is_some_and(|x| x == *imp.selection) {
                        gesture.set_state(gtk::EventSequenceState::Claimed);

                        let crop_area = imp.crop_area();
                        imp.set_selection_in_move(Some(InMove {
                            initial_selection: crop_area,
                        }));
                    } else {
                        gesture.set_state(gtk::EventSequenceState::Denied);
                        return;
                    }
                }
            ));

            // Drag moved
            self.selection_move.connect_drag_update(glib::clone!(
                #[weak]
                obj,
                move |_, x, y| {
                    let imp = obj.imp();

                    imp.aspect_ratio_animation().pause();

                    if let Some(in_move) = imp.selection_in_move.get() {
                        let moved_area = imp.moved_crop_area(&in_move, (x, y));

                        imp.set_crop(moved_area);
                    } else if let Some(resize) = imp.selection_in_resize.get() {
                        let coord = (x, y);

                        let new_area = if let Some(aspect_ratio) = imp.aspect_ratio() {
                            // Width for height an vice versa is only needed if corner is dragged
                            // and both dimensions change
                            if resize.handle.h_edge().is_none() {
                                imp.new_crop_area_aspect_ratio(&resize, true, coord, aspect_ratio)
                            } else if resize.handle.v_edge().is_none() {
                                imp.new_crop_area_aspect_ratio(&resize, false, coord, aspect_ratio)
                            } else {
                                let u = imp.new_crop_area_aspect_ratio(
                                    &resize,
                                    true,
                                    coord,
                                    aspect_ratio,
                                );
                                let v = imp.new_crop_area_aspect_ratio(
                                    &resize,
                                    false,
                                    coord,
                                    aspect_ratio,
                                );

                                if u.area() > v.area() {
                                    u
                                } else {
                                    v
                                }
                            }
                        } else {
                            imp.new_crop_area_aspect_free(&resize, coord)
                        };

                        imp.set_crop(new_area);
                    }
                }
            ));

            // Drag finished
            self.selection_move.connect_drag_end(glib::clone!(
                #[weak]
                obj,
                move |_, _, _| {
                    obj.imp().set_selection_in_resize(None);
                    obj.imp().set_selection_in_move(None);
                }
            ));
        }

        fn dispose(&self) {
            let obj = self.obj();

            while let Some(child) = obj.last_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for LpEditCropSelection {
        fn size_allocate(&self, _width: i32, _height: i32, _baseline: i32) {
            let total_x = self.total_x() as f32;
            let total_y = self.total_y() as f32;
            let total_height = self.total_height();
            let total_width: i32 = self.total_width();

            let (x, y, width, height) = self.crop_area_widget_coord();

            self.crop_x.set(x);
            self.crop_y.set(y);
            self.crop_width.set(width);
            self.crop_height.set(height);

            self.update_apply_crop_visibility();

            if self.is_cropped() != self.cropped.get() {
                self.cropped.set(self.is_cropped());
                self.obj().notify_cropped();
            }

            // Selection area
            self.selection_overlay.allocate(
                width as i32,
                height as i32,
                -1,
                Some(
                    gsk::Transform::new()
                        .translate(&graphene::Point::new(x + total_x, y + total_y)),
                ),
            );

            // Grayed out areas
            self.space_top.allocate(
                total_width,
                y as i32,
                -1,
                Some(gsk::Transform::new().translate(&graphene::Point::new(total_x, total_y))),
            );
            self.space_right.allocate(
                total_width - x as i32 - width as i32,
                height as i32,
                -1,
                Some(
                    gsk::Transform::new()
                        .translate(&graphene::Point::new(x + width + total_x, y + total_y)),
                ),
            );
            self.space_bottom.allocate(
                total_width,
                total_height - y as i32 - height as i32,
                -1,
                Some(
                    gsk::Transform::new()
                        .translate(&graphene::Point::new(total_x, y + height + total_y)),
                ),
            );
            self.space_left.allocate(
                x as i32,
                height as i32,
                -1,
                Some(gsk::Transform::new().translate(&graphene::Point::new(total_x, y + total_y))),
            );
        }
    }
    impl GridImpl for LpEditCropSelection {}

    impl LpEditCropSelection {
        /// Current crop area
        fn crop_area(&self) -> graphene::Rect {
            let obj = self.obj();
            graphene::Rect::new(
                obj.crop_x(),
                obj.crop_y(),
                obj.crop_width(),
                obj.crop_height(),
            )
        }

        /// Naive new crop area for moving `InResize`
        fn new_crop_area(&self, resize: &InResize, (x, y): (f64, f64)) -> graphene::Rect {
            let (x_offset, width_offset) = match resize.handle.h_edge() {
                // Change the crop x position and reduce width accordingly
                Some(HEdge::Left) => (x, -x),
                // Just change the crop width
                Some(HEdge::Right) => (0., x),
                // Don't change
                None => (0., 0.),
            };

            let (y_offset, height_offset) = match resize.handle.v_edge() {
                Some(VEdge::Top) => (y, -y),
                Some(VEdge::Bottom) => (0., y),
                None => (0., 0.),
            };

            let mut x = resize.initial_selection.x() + x_offset as f32;
            let mut y = resize.initial_selection.y() + y_offset as f32;

            let mut width = resize.initial_selection.width() + width_offset as f32;
            let mut height = resize.initial_selection.height() + height_offset as f32;

            if width < 0. {
                if matches!(resize.handle.h_edge(), Some(HEdge::Left)) {
                    x += width;
                }
                width = 0.;
            }

            if height < 0. {
                if matches!(resize.handle.v_edge(), Some(VEdge::Top)) {
                    y += height;
                }
                height = 0.;
            }

            graphene::Rect::new(x, y, width, height)
        }

        fn moved_crop_area(
            &self,
            shift: &InMove,
            (x_shift, y_shift): (f64, f64),
        ) -> graphene::Rect {
            let (mut x, mut y, width, height) = shift.initial_selection.into_tuple();

            x += x_shift as f32;
            y += y_shift as f32;

            if x < 0. {
                x = 0.;
            }

            if y < 0. {
                y = 0.;
            }

            if x + width > self.total_width() as f32 {
                x = self.total_width() as f32 - width;
            }

            if y + height > self.total_height() as f32 {
                y = self.total_height() as f32 - height;
            }

            graphene::Rect::new(x, y, width, height)
        }

        fn new_crop_area_aspect_free(
            &self,
            resize: &InResize,
            (x, y): (f64, f64),
        ) -> graphene::Rect {
            let (mut x, mut y, mut width, mut height) =
                self.new_crop_area(resize, (x, y)).into_tuple();

            if width < MIN_SELECTION_SIZE {
                let x_diff = width - MIN_SELECTION_SIZE;
                width = MIN_SELECTION_SIZE;

                if matches!(resize.handle.h_edge(), Some(HEdge::Left)) {
                    x += x_diff;
                }
            }

            if height < MIN_SELECTION_SIZE {
                let y_diff = height - MIN_SELECTION_SIZE;
                height = MIN_SELECTION_SIZE;

                if matches!(resize.handle.v_edge(), Some(VEdge::Top)) {
                    y += y_diff;
                }
            }

            if x < 0. {
                width += x;
                x = 0.;
            }

            if y < 0. {
                height += y;
                y = 0.;
            }

            if x + width > self.total_width() as f32 {
                width = self.total_width() as f32 - x;
            }

            if y + height > self.total_height() as f32 {
                height = self.total_height() as f32 - y;
            }

            graphene::Rect::new(x, y, width, height)
        }

        fn new_crop_area_aspect_ratio(
            &self,
            resize: &InResize,
            vertical: bool,
            (x, y): (f64, f64),
            aspect_ratio: f32,
        ) -> graphene::Rect {
            let new_area = self.new_crop_area(resize, (x, y));

            let (mut x, mut y, mut width, mut height) = new_area.into_tuple();

            if vertical {
                // Height for width mode
                width = new_area.height() * aspect_ratio;
                if matches!(resize.handle.h_edge(), Some(HEdge::Left)) {
                    x += new_area.width() - width;
                }
            } else {
                // Width for height mode
                height = new_area.width() / aspect_ratio;
                if matches!(resize.handle.v_edge(), Some(VEdge::Top)) {
                    y += new_area.height() - height;
                }
            }

            let mut rect = graphene::Rect::new(x, y, width, height);

            self.rect_limit_top(&mut rect, aspect_ratio, resize.handle.v_edge());
            self.rect_limit_left(&mut rect, aspect_ratio, resize.handle.h_edge());
            self.rect_limit_bottom(&mut rect, aspect_ratio, resize.handle.h_edge());
            self.rect_limit_right(&mut rect, aspect_ratio, resize.handle.v_edge());

            self.rect_limit_minumum_size(&mut rect, aspect_ratio, resize.handle);

            (x, y, width, height) = rect.into_tuple();
            graphene::Rect::new(x, y, width, height)
        }

        /// Ensure selection is not smaller than minimum size
        fn rect_limit_minumum_size(
            &self,
            rect: &mut graphene::Rect,
            aspect_ratio: f32,
            handle: RectHandle,
        ) {
            let (mut x, mut y, mut width, mut height) = rect.into_tuple();

            if width < MIN_SELECTION_SIZE {
                let x_diff = width - MIN_SELECTION_SIZE;
                width = MIN_SELECTION_SIZE;
                let y_diff = height - MIN_SELECTION_SIZE / aspect_ratio;
                height = MIN_SELECTION_SIZE / aspect_ratio;

                if matches!(handle.v_edge(), Some(VEdge::Top)) {
                    y += y_diff;
                }
                if matches!(handle.h_edge(), Some(HEdge::Left)) {
                    x += x_diff;
                }
            }

            if height < MIN_SELECTION_SIZE {
                let x_diff = width - MIN_SELECTION_SIZE * aspect_ratio;
                width = MIN_SELECTION_SIZE * aspect_ratio;
                let y_diff = height - MIN_SELECTION_SIZE;
                height = MIN_SELECTION_SIZE;

                if matches!(handle.v_edge(), Some(VEdge::Top)) {
                    y += y_diff;
                }
                if matches!(handle.h_edge(), Some(HEdge::Left)) {
                    x += x_diff;
                }
            }

            *rect = graphene::Rect::new(x, y, width, height);
        }

        /// Make sure Rect is inside image area by moving it if necessary
        fn rect_make_contained(&self, rect: &mut graphene::Rect) {
            let (mut x, mut y, width, height) = rect.into_tuple();

            if x + width > self.total_width() as f32 {
                x = self.total_width() as f32 - width;
            }

            if y + height > self.total_height() as f32 {
                y = self.total_height() as f32 - height;
            }

            *rect = graphene::Rect::new(x, y, width, height);
        }

        /// Limit Rect to not leave image area to the left
        fn rect_limit_top(
            &self,
            rect: &mut graphene::Rect,
            aspect_ratio: f32,
            edge: Option<VEdge>,
        ) {
            let (mut x, mut y, mut width, mut height) = rect.into_tuple();

            if x < 0. {
                width += x;
                x = 0.;
                let old_height = height;
                height = width / aspect_ratio;
                if matches!(edge, Some(VEdge::Top)) {
                    y += old_height - height;
                }
            }

            *rect = graphene::Rect::new(x, y, width, height);
        }

        /// Limit Rect to not leave image area at to the left
        fn rect_limit_left(
            &self,
            rect: &mut graphene::Rect,
            aspect_ratio: f32,
            edge: Option<HEdge>,
        ) {
            let (mut x, mut y, mut width, mut height) = rect.into_tuple();

            if y < 0. {
                height += y;
                y = 0.;
                let old_width = width;
                width = height * aspect_ratio;

                if matches!(edge, Some(HEdge::Left)) {
                    x += old_width - width;
                }

                *rect = graphene::Rect::new(x, y, width, height);
            }
        }

        /// Limit Rect to not leave image area at the bottom
        fn rect_limit_bottom(
            &self,
            rect: &mut graphene::Rect,
            aspect_ratio: f32,
            edge: Option<HEdge>,
        ) {
            let (mut x, y, mut width, mut height) = rect.into_tuple();

            if y + height > self.total_height() as f32 {
                let overshoot = y + height - self.total_height() as f32;

                height -= overshoot;
                let old_width = width;
                width = height * aspect_ratio;

                if matches!(edge, Some(HEdge::Left)) {
                    x += old_width - width;
                }

                *rect = graphene::Rect::new(x, y, width, height);
            }
        }

        fn rect_limit_right(
            &self,
            rect: &mut graphene::Rect,
            aspect_ratio: f32,
            edge: Option<VEdge>,
        ) {
            let (x, mut y, mut width, mut height) = rect.into_tuple();

            if x + width > self.total_width() as f32 {
                let overshoot = x + width - self.total_width() as f32;

                width -= overshoot;
                let old_height = height;
                height = width / aspect_ratio;

                if matches!(edge, Some(VEdge::Top)) {
                    y += old_height - height;
                }

                *rect = graphene::Rect::new(x, y, width, height);
            }
        }

        pub(super) fn aspect_ratio_changed(&self) {
            let Some(aspect_ratio) = self.aspect_ratio() else {
                return;
            };

            let obj = self.obj();

            // Store current state as reference for animation
            let current_area = self.crop_area();
            let (_, y, _, height) = current_area.into_tuple();

            let width = current_area.height() * aspect_ratio;
            let x = f32::max(0., current_area.x() + (current_area.width() - width) / 2.);

            let mut new_area = graphene::Rect::new(x, y, width, height);

            self.rect_limit_top(&mut new_area, aspect_ratio, Some(VEdge::Bottom));
            self.rect_limit_left(&mut new_area, aspect_ratio, Some(HEdge::Right));
            self.rect_limit_bottom(&mut new_area, aspect_ratio, Some(HEdge::Left));
            self.rect_limit_right(&mut new_area, aspect_ratio, Some(VEdge::Top));

            self.rect_limit_minumum_size(&mut new_area, aspect_ratio, RectHandle::BottomLeft);

            self.rect_make_contained(&mut new_area);

            self.aspect_ratio_animation()
                .set_target(&adw::CallbackAnimationTarget::new(glib::clone!(
                    #[weak]
                    obj,
                    move |progress| {
                        let imp = obj.imp();

                        // Linear interpolate between old and target value
                        imp.set_crop(current_area.interpolate(&new_area, progress));
                    }
                )));

            self.aspect_ratio_animation().play();
        }

        /// Fixed aspect ratio of selection if set
        fn aspect_ratio(&self) -> Option<f32> {
            self.obj().aspect_ratio().num(&self.obj())
        }

        /// Set all coordinates of crop selection
        fn set_crop(&self, rect: graphene::Rect) {
            let rect = rect.round_extents();

            self.set_crop_x(rect.x());
            self.set_crop_y(rect.y());
            self.set_crop_width(rect.width());
            self.set_crop_height(rect.height());
        }

        /// Set x coordinate of crop selection rectangle origin
        pub(super) fn set_crop_x(&self, x: f32) {
            if x < 0. {
                log::error!("Tried to set x coordinate to {x}");
                return;
            }

            let obj = self.obj();

            self.crop_x.set(x);

            obj.notify_crop_x();
            self.update_crop_area_image_coord();

            self.update_apply_crop_visibility();
            self.obj().queue_allocate();
        }

        /// Set y coordinate of crop selection rectangle origin
        pub(super) fn set_crop_y(&self, y: f32) {
            if y < 0. {
                log::error!("Tried to set y coordinate to {y}");
                return;
            }

            let obj = self.obj();

            self.crop_y.set(y);
            self.update_crop_area_image_coord();

            obj.notify_crop_y();
            self.update_apply_crop_visibility();
            self.obj().queue_allocate();
        }

        /// Set width of crop selection rectangle
        pub(super) fn set_crop_width(&self, width: f32) {
            if width < 0. {
                eprintln!("Tried to set width to {width}");
                return;
            }

            self.crop_width.set(width);
            self.update_crop_area_image_coord();

            self.obj().notify_crop_width();
            self.update_apply_crop_visibility();
            self.obj().queue_allocate();
        }

        /// Set height of crop selection rectangle
        pub(super) fn set_crop_height(&self, height: f32) {
            if height < 0. {
                eprintln!("Tried to set height to {height}");
                return;
            }

            self.crop_height.set(height);
            self.update_crop_area_image_coord();

            self.obj().notify_crop_height();
            self.update_apply_crop_visibility();
            self.obj().queue_allocate();
        }

        fn update_crop_area_image_coord(&self) {
            let crop_area_image_coord = self.crop_area_image_coord();
            if self.is_in_user_change() {
                log::trace!("Setting crop are in image coordinates to {crop_area_image_coord:?}");
                self.crop_area_image_coord.replace(crop_area_image_coord);
            }
        }

        fn total_x(&self) -> i32 {
            self.image().image_rendering_x() as i32
        }

        fn total_y(&self) -> i32 {
            self.image().image_rendering_y() as i32
        }

        pub(super) fn total_width(&self) -> i32 {
            self.image().image_rendering_width() as i32
        }

        pub(super) fn total_height(&self) -> i32 {
            self.image().image_rendering_height() as i32
        }

        pub(super) fn image(&self) -> LpImage {
            let crop = self
                .obj()
                .ancestor(LpEditCrop::static_type())
                .unwrap()
                .downcast::<LpEditCrop>()
                .unwrap();
            crop.image()
        }

        pub(super) fn aspect_ratio_animation(&self) -> &adw::TimedAnimation {
            self.aspect_ratio_animation.get_or_init(|| {
                adw::TimedAnimation::builder()
                    .duration(200)
                    .value_from(0.)
                    .value_to(1.)
                    .easing(adw::Easing::EaseOutSine)
                    .widget(&*self.obj())
                    .target(&adw::CallbackAnimationTarget::new(|_| {}))
                    .build()
            })
        }

        fn set_selection_in_resize(&self, in_resize: Option<InResize>) {
            self.selection_in_resize.replace(in_resize);
            self.update_apply_crop_visibility();
        }

        fn set_selection_in_move(&self, in_move: Option<InMove>) {
            self.selection_in_move.replace(in_move);
            self.update_apply_crop_visibility();
        }

        fn update_apply_crop_visibility(&self) {
            let visibile = self.is_cropped()
                && self.selection_in_resize.get().is_none()
                && self.selection_in_move.get().is_none();

            // Don't use `set_visble` since it queues a resize
            if visibile {
                self.apply_button.set_opacity(1.);
                self.apply_button.set_can_target(true);
                self.apply_button.set_can_focus(true);
            } else {
                self.apply_button.set_opacity(0.);
                self.apply_button.set_can_target(false);
                self.apply_button.set_can_focus(false);
            }
        }

        pub(super) fn crop_area_image_coord(&self) -> Option<(u32, u32, u32, u32)> {
            let obj = self.obj();

            if !self.is_cropped() {
                return None;
            }

            let image = self.image();

            let (x1, y1) = image.widget_to_img_coord((
                image.image_rendering_x() + obj.crop_x() as f64,
                image.image_rendering_y() + obj.crop_y() as f64,
            ));
            let (x2, y2) = image.widget_to_img_coord((
                image.image_rendering_x() + obj.crop_x() as f64 + obj.crop_width() as f64,
                image.image_rendering_y() + obj.crop_y() as f64 + obj.crop_height() as f64,
            ));

            Some((
                x1.round() as u32,
                y1.round() as u32,
                (x2 - x1).round() as u32,
                (y2 - y1).round() as u32,
            ))
        }

        fn crop_area_widget_coord(&self) -> (f32, f32, f32, f32) {
            let image = self.image();

            let size = image.image_size();
            let (x, y, w, h) =
                self.crop_area_image_coord
                    .get()
                    .unwrap_or((0, 0, size.0 as u32, size.1 as u32));
            let (x, y) = image.img_to_draw_coord((x as f64, y as f64));
            let (w, h) = image.img_to_draw_coord((w as f64, h as f64));

            (
                x.round() as f32,
                y.round() as f32,
                w.round() as f32,
                h.round() as f32,
            )
        }

        fn set_aspect_ratio(&self, aspect_ratio: LpAspectRatio) {
            self.aspect_ratio.replace(aspect_ratio);
            self.aspect_ratio_changed();
        }

        fn set_orientation(&self, orientation: LpOrientation) {
            self.orientation.replace(orientation);
            self.aspect_ratio_changed();
        }

        fn is_in_user_change(&self) -> bool {
            self.selection_in_resize.get().is_some()
                || self.selection_in_move.get().is_some()
                || self.selection_in_reset.get()
                || self.aspect_ratio_animation().state() == adw::AnimationState::Playing
        }

        pub(super) fn is_cropped(&self) -> bool {
            let obj = self.obj();

            let untouched: bool = obj.crop_x() == 0.
                && obj.crop_y() == 0.
                && obj.crop_width() as i32 == self.total_width()
                && obj.crop_height() as i32 == self.total_height();

            !untouched
        }

        pub(super) fn reset(&self) {
            let width = self.total_width() as f32;
            let height = self.total_height() as f32;

            self.set_crop(graphene::Rect::new(0., 0., width, height));
            self.crop_area_image_coord.set(None);
        }
    }
}

glib::wrapper! {
    pub struct LpEditCropSelection(ObjectSubclass<imp::LpEditCropSelection>)
        @extends gtk::Widget, gtk::Grid,
        @implements gtk::Buildable, gtk::Accessible, gtk::ConstraintTarget, gtk::Orientable;
}

impl LpEditCropSelection {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn reset(&self) {
        self.imp().reset();
    }

    pub fn crop_area_image_coord(&self) -> Option<(u32, u32, u32, u32)> {
        self.imp().crop_area_image_coord()
    }

    pub fn image(&self) -> LpImage {
        self.imp().image()
    }
}
