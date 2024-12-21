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

use std::cell::{Cell, OnceCell};
use std::marker::PhantomData;

use adw::gtk::graphene;
use adw::prelude::*;
use adw::subclass::prelude::*;
use adw::{glib, gtk};

use super::crop::{LpAspectRatio, LpEditCrop, LpOrientation};

const MIN_SELECTION_SIZE: f32 = 50.;

#[derive(Debug, Clone, Copy)]
struct InResize {
    corner: Corner,
    initial_selection: graphene::Rect,
}

#[derive(Debug, Clone, Copy)]
struct InMove {
    initial_selection: graphene::Rect,
}

#[derive(Debug, Clone, Copy)]
enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
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

impl Corner {
    fn cursor_name(&self) -> &str {
        match self {
            Self::TopLeft => "nw-resize",
            Self::TopRight => "ne-resize",
            Self::BottomLeft => "sw-resize",
            Self::BottomRight => "se-resize",
        }
    }

    fn h_edge(&self) -> HEdge {
        match self {
            Self::TopLeft => HEdge::Left,
            Self::TopRight => HEdge::Right,
            Self::BottomLeft => HEdge::Left,
            Self::BottomRight => HEdge::Right,
        }
    }

    fn v_edge(&self) -> VEdge {
        match self {
            Self::TopLeft => VEdge::Top,
            Self::TopRight => VEdge::Top,
            Self::BottomLeft => VEdge::Bottom,
            Self::BottomRight => VEdge::Bottom,
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

    fn num(&self, crop: &LpEditCrop) -> Option<f32> {
        let (x, y) = if matches!(self, Self::Original) {
            let width = crop.selection().total_width() as u32;
            let height = crop.selection().total_height() as u32;

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
    fn coordiantes(self) -> (f32, f32, f32, f32) {
        let rect: graphene::Rect = self.into();

        (rect.x(), rect.y(), rect.width(), rect.height())
    }
}

impl RectExt for graphene::Rect {}

mod imp {
    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::LpEditCropSelection)]
    #[template(file = "crop_selection.ui")]
    pub struct LpEditCropSelection {
        #[template_child]
        pub(super) space_top_left: TemplateChild<adw::Bin>,
        #[template_child]
        pub(super) selection: TemplateChild<gtk::Grid>,

        #[template_child]
        handle_top_left: TemplateChild<adw::Bin>,
        #[template_child]
        handle_top_right: TemplateChild<adw::Bin>,
        #[template_child]
        handle_bottom_right: TemplateChild<adw::Bin>,
        #[template_child]
        handle_bottom_left: TemplateChild<adw::Bin>,

        #[template_child]
        selection_move: TemplateChild<gtk::GestureDrag>,

        /// Set while in drag gesture for changing crop area
        pub(super) selection_in_resize: Cell<Option<InResize>>,
        /// Set while in drag gesture for moving crop area
        pub(super) selection_in_move: Cell<Option<InMove>>,

        // Animates changes between different fixed aspect ratios
        aspect_ratio_animation: OnceCell<adw::TimedAnimation>,

        #[property(get, construct_only)]
        crop: OnceCell<LpEditCrop>,

        #[property(get=Self::total_width, set=Self::set_total_width)]
        total_width: PhantomData<i32>,
        #[property(get=Self::total_height, set=Self::set_total_height)]
        total_height: PhantomData<i32>,

        pub(super) initialized: Cell<bool>,

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
        type ParentType = gtk::Grid;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            klass.set_css_name("lpcrop");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for LpEditCropSelection {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();

            obj.set_direction(gtk::TextDirection::Ltr);

            // List of widgets that are drag handles
            let drag_corners = std::collections::BTreeMap::from([
                (self.handle_top_left.clone(), Corner::TopLeft),
                (self.handle_top_right.clone(), Corner::TopRight),
                (self.handle_bottom_left.clone(), Corner::BottomLeft),
                (self.handle_bottom_right.clone(), Corner::BottomRight),
            ]);

            // Set cursor style for each corner
            for (handle_widget, corner) in drag_corners.iter() {
                handle_widget.set_cursor_from_name(Some(corner.cursor_name()));
            }

            self.selection.set_cursor_from_name(Some("move"));

            obj.crop()
                .connect_aspect_ratio_notify(|x| x.selection().imp().on_aspect_ratio_changed());

            obj.crop()
                .connect_orientation_notify(|x| x.selection().imp().on_aspect_ratio_changed());

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
                    // what corner it is
                    if let Some(corner) = hovered_widget
                        .as_ref()
                        .and_then(|x| drag_corners.get(x).copied())
                    {
                        gesture.set_state(gtk::EventSequenceState::Claimed);

                        let crop_area = imp.crop_area();

                        imp.selection_in_resize.set(Some(InResize {
                            corner,
                            initial_selection: crop_area,
                        }));
                    } else if hovered_widget.map_or(false, |x| x == *imp.selection) {
                        gesture.set_state(gtk::EventSequenceState::Claimed);

                        let crop_area = imp.crop_area();
                        imp.selection_in_move.set(Some(InMove {
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
                    } else if let Some(drag) = imp.selection_in_resize.get() {
                        let coord = (x, y);

                        let new_area = if let Some(aspect_ratio) = imp.aspect_ratio() {
                            let u =
                                imp.new_crop_area_aspect_ratio(&drag, true, coord, aspect_ratio);
                            let v =
                                imp.new_crop_area_aspect_ratio(&drag, false, coord, aspect_ratio);

                            if u.area() > v.area() {
                                u
                            } else {
                                v
                            }
                        } else {
                            imp.new_crop_area_aspect_free(&drag, coord)
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
                    obj.imp().selection_in_resize.set(None);
                    obj.imp().selection_in_move.set(None);
                }
            ));
        }
    }

    impl WidgetImpl for LpEditCropSelection {}
    impl GridImpl for LpEditCropSelection {}

    impl LpEditCropSelection {
        /// Current crop area
        fn crop_area(&self) -> graphene::Rect {
            graphene::Rect::new(
                self.space_top_left.width_request() as f32,
                self.space_top_left.height_request() as f32,
                self.selection.width_request() as f32,
                self.selection.height_request() as f32,
            )
        }

        fn new_crop_area(&self, resize: &InResize, (x, y): (f64, f64)) -> graphene::Rect {
            let (x_offset, width_offset) = match resize.corner.h_edge() {
                // Change the crop x position and reduce width accordingly
                HEdge::Left => (x, -x),
                // Just change the crop width
                HEdge::Right => (0., x),
            };

            let (y_offset, height_offset) = match resize.corner.v_edge() {
                VEdge::Top => (y, -y),
                VEdge::Bottom => (0., y),
            };

            let mut x = resize.initial_selection.x() + x_offset as f32;
            let mut y = resize.initial_selection.y() + y_offset as f32;

            let mut width = resize.initial_selection.width() + width_offset as f32;
            let mut height = resize.initial_selection.height() + height_offset as f32;

            if width < 0. {
                if matches!(resize.corner.h_edge(), HEdge::Left) {
                    x += width;
                }
                width = 0.;
            }

            if height < 0. {
                if matches!(resize.corner.v_edge(), VEdge::Top) {
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
            let obj = self.obj();

            let (mut x, mut y, width, height) = shift.initial_selection.coordiantes();

            x += x_shift as f32;
            y += y_shift as f32;

            if x < 0. {
                x = 0.;
            }

            if y < 0. {
                y = 0.;
            }

            if x + width > obj.width_request() as f32 {
                x = obj.width_request() as f32 - width;
            }

            if y + height > obj.height_request() as f32 {
                y = obj.height_request() as f32 - height;
            }

            graphene::Rect::new(x, y, width, height)
        }

        fn new_crop_area_aspect_free(
            &self,
            resize: &InResize,
            (x, y): (f64, f64),
        ) -> graphene::Rect {
            let obj = self.obj();

            let (mut x, mut y, mut width, mut height) =
                self.new_crop_area(resize, (x, y)).coordiantes();

            if width < MIN_SELECTION_SIZE {
                let x_diff = width - MIN_SELECTION_SIZE;
                width = MIN_SELECTION_SIZE;

                if matches!(resize.corner.h_edge(), HEdge::Left) {
                    x += x_diff;
                }
            }

            if height < MIN_SELECTION_SIZE {
                let y_diff = height - MIN_SELECTION_SIZE;
                height = MIN_SELECTION_SIZE;

                if matches!(resize.corner.v_edge(), VEdge::Top) {
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

            if x + width > obj.width_request() as f32 {
                width = obj.width_request() as f32 - x;
            }

            if y + height > obj.height_request() as f32 {
                height = obj.height_request() as f32 - y;
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

            let (mut x, mut y, mut width, mut height) = new_area.coordiantes();

            if vertical {
                width = new_area.height() * aspect_ratio;
                if matches!(resize.corner.h_edge(), HEdge::Left) {
                    x += new_area.width() - width;
                }
            } else {
                height = new_area.width() / aspect_ratio;
                if matches!(resize.corner.v_edge(), VEdge::Top) {
                    y += new_area.height() - height;
                }
            }

            let mut rect = graphene::Rect::new(x, y, width, height);

            self.rect_limit_top(&mut rect, aspect_ratio, resize.corner.v_edge());
            self.rect_limit_left(&mut rect, aspect_ratio, resize.corner.h_edge());
            self.rect_limit_bottom(&mut rect, aspect_ratio, resize.corner.h_edge());
            self.rect_limit_right(&mut rect, aspect_ratio, resize.corner.v_edge());

            self.rect_limit_minumum_size(&mut rect, aspect_ratio, resize.corner);

            (x, y, width, height) = rect.coordiantes();
            graphene::Rect::new(x, y, width, height)
        }

        /// Ensure selection is not smaller than minimum size
        fn rect_limit_minumum_size(
            &self,
            rect: &mut graphene::Rect,
            aspect_ratio: f32,
            corner: Corner,
        ) {
            let (mut x, mut y, mut width, mut height) = rect.coordiantes();

            if width < MIN_SELECTION_SIZE {
                let x_diff = width - MIN_SELECTION_SIZE;
                width = MIN_SELECTION_SIZE;
                let y_diff = height - MIN_SELECTION_SIZE / aspect_ratio;
                height = MIN_SELECTION_SIZE / aspect_ratio;

                if matches!(corner.v_edge(), VEdge::Top) {
                    y += y_diff;
                }
                if matches!(corner.h_edge(), HEdge::Left) {
                    x += x_diff;
                }
            }

            if height < MIN_SELECTION_SIZE {
                let x_diff = width - MIN_SELECTION_SIZE * aspect_ratio;
                width = MIN_SELECTION_SIZE * aspect_ratio;
                let y_diff = height - MIN_SELECTION_SIZE;
                height = MIN_SELECTION_SIZE;

                if matches!(corner.v_edge(), VEdge::Top) {
                    y += y_diff;
                }
                if matches!(corner.h_edge(), HEdge::Left) {
                    x += x_diff;
                }
            }

            *rect = graphene::Rect::new(x, y, width, height);
        }

        /// Make sure Rect is inside image area by moving it if necessary
        fn rect_make_contained(&self, rect: &mut graphene::Rect) {
            let (mut x, mut y, width, height) = rect.coordiantes();

            if x + width > self.total_width() as f32 {
                x = self.total_width() as f32 - width;
            }

            if y + height > self.total_height() as f32 {
                y = self.total_height() as f32 - height;
            }

            *rect = graphene::Rect::new(x, y, width, height);
        }

        /// Limit Rect to not leave image area to the left
        fn rect_limit_top(&self, rect: &mut graphene::Rect, aspect_ratio: f32, edge: VEdge) {
            let (mut x, mut y, mut width, mut height) = rect.coordiantes();

            if x < 0. {
                width += x;
                x = 0.;
                let old_height = height;
                height = width / aspect_ratio;
                if matches!(edge, VEdge::Top) {
                    y += old_height - height;
                }
            }

            *rect = graphene::Rect::new(x, y, width, height);
        }

        /// Limit Rect to not leave image area at to the left
        fn rect_limit_left(&self, rect: &mut graphene::Rect, aspect_ratio: f32, edge: HEdge) {
            let (mut x, mut y, mut width, mut height) = rect.coordiantes();

            if y < 0. {
                height += y;
                y = 0.;
                let old_width = width;
                width = height * aspect_ratio;

                if matches!(edge, HEdge::Left) {
                    x += old_width - width;
                }

                *rect = graphene::Rect::new(x, y, width, height);
            }
        }

        /// Limit Rect to not leave image area at the bottom
        fn rect_limit_bottom(&self, rect: &mut graphene::Rect, aspect_ratio: f32, edge: HEdge) {
            let obj = self.obj();
            let (mut x, y, mut width, mut height) = rect.coordiantes();

            if y + height > obj.height_request() as f32 {
                let overshoot = y + height - obj.height_request() as f32;

                height -= overshoot;
                let old_width = width;
                width = height * aspect_ratio;

                if matches!(edge, HEdge::Left) {
                    x += old_width - width;
                }

                *rect = graphene::Rect::new(x, y, width, height);
            }
        }

        fn rect_limit_right(&self, rect: &mut graphene::Rect, aspect_ratio: f32, edge: VEdge) {
            let obj = self.obj();
            let (x, mut y, mut width, mut height) = rect.coordiantes();

            if x + width > obj.width_request() as f32 {
                let overshoot = x + width - obj.width_request() as f32;

                width -= overshoot;
                let old_height = height;
                height = width / aspect_ratio;

                if matches!(edge, VEdge::Top) {
                    y += old_height - height;
                }

                *rect = graphene::Rect::new(x, y, width, height);
            }
        }

        fn on_aspect_ratio_changed(&self) {
            let Some(aspect_ratio) = self.aspect_ratio() else {
                return;
            };

            let obj = self.obj();

            // Store current state as reference for animation
            let current_area = self.crop_area();
            let (_, y, _, height) = current_area.coordiantes();

            let width = current_area.height() * aspect_ratio;
            let x = f32::max(0., current_area.x() + (current_area.width() - width) / 2.);

            let mut new_area = graphene::Rect::new(x, y, width, height);

            self.rect_limit_top(&mut new_area, aspect_ratio, VEdge::Bottom);
            self.rect_limit_left(&mut new_area, aspect_ratio, HEdge::Right);
            self.rect_limit_bottom(&mut new_area, aspect_ratio, HEdge::Left);
            self.rect_limit_right(&mut new_area, aspect_ratio, VEdge::Top);

            self.rect_limit_minumum_size(&mut new_area, aspect_ratio, Corner::BottomLeft);

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
            let crop = self.obj().crop();
            crop.aspect_ratio().num(&crop)
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

            self.crop_x.set(x);
            self.space_top_left.set_width_request(x.round() as i32);
            self.obj().notify_crop_x();
        }

        /// Set y coordinate of crop selection rectangle origin
        pub(super) fn set_crop_y(&self, y: f32) {
            if y < 0. {
                log::error!("Tried to set y coordinate to {y}");
                return;
            }

            self.crop_y.set(y);
            self.space_top_left.set_height_request(y.round() as i32);
            self.obj().notify_crop_y();
        }

        /// Set width of crop selection rectangle
        pub(super) fn set_crop_width(&self, width: f32) {
            if width < 0. {
                eprintln!("Tried to set width to {width}");
                return;
            }

            self.crop_width.set(width);
            self.selection.set_width_request(width.round() as i32);
            self.obj().notify_crop_width();
        }

        /// Set height of crop selection rectangle
        pub(super) fn set_crop_height(&self, height: f32) {
            if height < 0. {
                eprintln!("Tried to set height to {height}");
                return;
            }

            self.crop_height.set(height);
            self.selection.set_height_request(height.round() as i32);
            self.obj().notify_crop_height();
        }

        /// Width of area in which the crop selection can exist
        fn total_width(&self) -> i32 {
            self.obj().width_request()
        }

        fn set_total_width(&self, width: i32) {
            self.obj().set_width_request(width);
        }

        /// Height of area in which the crop selection can exist
        fn total_height(&self) -> i32 {
            self.obj().height_request()
        }

        fn set_total_height(&self, width: i32) {
            self.obj().set_height_request(width);
        }

        fn aspect_ratio_animation(&self) -> &adw::TimedAnimation {
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
    }
}

glib::wrapper! {
    pub struct LpEditCropSelection(ObjectSubclass<imp::LpEditCropSelection>)
        @extends gtk::Widget;
}

impl LpEditCropSelection {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn ensure_initialized(&self, x: f64, y: f64, width: f64, height: f64) {
        let imp = self.imp();

        if imp.initialized.get() {
            return;
        }

        imp.initialized.set(true);

        self.reset(x, y, width, height);
    }

    pub fn reset(&self, x: f64, y: f64, width: f64, height: f64) {
        self.set_image_area(x, y, width, height);
        self.set_crop_size(0., 0., width, height);
    }

    pub fn set_image_area(&self, x: f64, y: f64, width: f64, height: f64) {
        self.set_margin_start(x as i32);
        self.set_margin_top(y as i32);
        self.set_total_width(width as i32);
        self.set_total_height(height as i32);
    }

    pub fn set_crop_size(&self, x: f64, y: f64, width: f64, height: f64) {
        self.set_crop_x(x as f32);
        self.set_crop_y(y as f32);
        self.set_crop_width(width as f32);
        self.set_crop_height(height as f32);
    }

    pub fn is_cropped(&self) -> bool {
        let untouched = self.crop_x() == 0.
            && self.crop_y() == 0.
            && self.crop_width() as i32 == self.total_width()
            && self.crop_height() as i32 == self.total_height();
        !untouched
    }

    pub fn is_in_user_change(&self) -> bool {
        self.imp().selection_in_resize.get().is_some()
            || self.imp().selection_in_move.get().is_some()
    }
}
