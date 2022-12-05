//
// Copyright 2021 Christopher Davis <christopherdavis@gnome.org>
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

use adw::{prelude::*, subclass::prelude::*};

use once_cell::sync::Lazy;

use once_cell::unsync::OnceCell;
use std::cell::{Cell, RefCell};

use crate::image_metadata::ImageMetadata;

const ZOOM_ANIMATION_DURATION: u32 = 200;
const ROTATION_ANIMATION_DURATION: u32 = 200;

const ZOOM_FACTOR_BUTTON: f64 = 1.5;
const ZOOM_FACTOR_WHEEL: f64 = 1.3;
const ZOOM_FACTOR_WHEEL_HI_RES: f64 = 0.1;

const MAX_ZOOM_LEVEL: f64 = 20.0;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct LpImage {
        pub file: RefCell<Option<gio::File>>,
        pub texture: RefCell<Option<gdk::Texture>>,
        /// Rotation final value (can differ from `rotation` during animation)
        pub rotation_target: Cell<f64>,
        /// Rotated presentation of original image in degrees clockwise
        pub rotation: Cell<f64>,
        // Animates the `rotation` property
        pub rotation_animation: OnceCell<adw::TimedAnimation>,
        /// Mirrored presentation of original image
        pub mirrored: Cell<bool>,

        /// Displayed zoom level
        pub zoom: Cell<f64>,
        pub zoom_animation: OnceCell<adw::TimedAnimation>,
        /// Targeted zoom level, might differ from `zoom` when animation is running
        pub zoom_target: Cell<f64>,
        /// Current animation is transitioning from having horizontal scrollbars
        /// to not having them or vice versa.
        pub zoom_hscrollbar_transition: Cell<bool>,
        /// Same but vor vertical
        pub zoom_vscrollbar_transition: Cell<bool>,

        /// Always fit image into window, causes `zoom` to change automatically
        pub best_fit: Cell<bool>,
        /// Max zoom level is reached, stored to only send signals on change
        pub max_zoom: Cell<bool>,

        /// Horizontal scrolling
        pub hadjustment: RefCell<Option<gtk::Adjustment>>,
        /// Vertical scrolling
        pub vadjustment: RefCell<Option<gtk::Adjustment>>,

        /// Currently EXIF data
        pub image_metadata: RefCell<ImageMetadata>,

        /// Current pointer position
        pub pointer_position: Cell<Option<(f64, f64)>>,

        /// Position of fingers during zoom gesture
        ///
        /// Required for calculating delta when moving window on touchscreen.
        /// On touchpad this is only the initial value used as the zoom target.
        pub zoom_gesture_center: Cell<Option<(f64, f64)>>,
        /// Required for calculating delta while moving window around
        pub last_drag_value: Cell<Option<(f64, f64)>>,

        widget_dimensions: Cell<(i32, i32)>,

        scale_factor: Cell<i32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpImage {
        const NAME: &'static str = "LpImage";
        type ParentType = gtk::Widget;
        type Type = super::LpImage;
        type Interfaces = (gtk::Scrollable,);
    }

    impl ObjectImpl for LpImage {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecObject::builder::<gio::File>("file")
                        .read_only()
                        .build(),
                    glib::ParamSpecDouble::builder("rotation")
                        .explicit_notify()
                        .build(),
                    glib::ParamSpecBoolean::builder("mirrored")
                        .explicit_notify()
                        .build(),
                    glib::ParamSpecDouble::builder("zoom")
                        .explicit_notify()
                        .build(),
                    glib::ParamSpecBoolean::builder("best-fit")
                        .explicit_notify()
                        .build(),
                    glib::ParamSpecBoolean::builder("is-max-zoom")
                        .read_only()
                        .build(),
                    glib::ParamSpecOverride::for_interface::<gtk::Scrollable>("hadjustment"),
                    glib::ParamSpecOverride::for_interface::<gtk::Scrollable>("vadjustment"),
                    glib::ParamSpecOverride::for_interface::<gtk::Scrollable>("hscroll-policy"),
                    glib::ParamSpecOverride::for_interface::<gtk::Scrollable>("vscroll-policy"),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = self.instance();
            match pspec.name() {
                "file" => obj.file().to_value(),
                "rotation" => obj.rotation().to_value(),
                "mirrored" => obj.mirrored().to_value(),
                "zoom" => obj.zoom().to_value(),
                "best-fit" => obj.is_best_fit().to_value(),
                "is-max-zoom" => obj.is_max_zoom().to_value(),
                // don't use getter functions here sicne they can return a fake adjustment
                "hadjustment" => self.hadjustment.borrow().to_value(),
                "vadjustment" => self.vadjustment.borrow().to_value(),
                "hscroll-policy" | "vscroll-policy" => gtk::ScrollablePolicy::Minimum.to_value(),
                name => unimplemented!("property {}", name),
            }
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.instance();
            match pspec.name() {
                "rotation" => obj.set_rotation(value.get().unwrap()),
                "mirrored" => obj.set_mirrored(value.get().unwrap()),
                "zoom" => obj.set_zoom(value.get().unwrap()),
                "best-fit" => obj.set_best_fit(value.get().unwrap()),
                "hadjustment" => obj.set_hadjustment(value.get().unwrap()),
                "vadjustment" => obj.set_vadjustment(value.get().unwrap()),
                "hscroll-policy" | "vscroll-policy" => (),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.instance();
            obj.set_hexpand(true);
            obj.set_vexpand(true);
            obj.set_overflow(gtk::Overflow::Hidden);

            self.zoom.set(1.);
            self.zoom_target.set(1.);
            self.best_fit.set(true);
            self.scale_factor.set(obj.scale_factor());

            self.connect_controllers();
            self.connect_gestures();

            obj.connect_scale_factor_notify(|obj| {
                let scale_before = obj.imp().scale_factor.get();
                let scale_now = obj.scale_factor();

                log::debug!("Scale factor change from {scale_before} to {scale_now}");

                obj.zoom_animation().pause();

                if obj.is_best_fit() {
                    obj.queue_resize();
                    obj.queue_draw();
                } else {
                    let new_zoom =
                        obj.imp().zoom_target.get() * scale_before as f64 / scale_now as f64;
                    obj.imp().zoom_target.set(new_zoom);
                    obj.set_zoom(new_zoom);
                }

                obj.imp().scale_factor.set(scale_now);
            });
        }

        fn dispose(&self) {
            let obj = self.instance();

            // remove target fron zoom animation because it's property of this object
            obj.rotation_animation()
                .set_target(&adw::CallbackAnimationTarget::new(|_| {}));
            obj.zoom_animation()
                .set_target(&adw::CallbackAnimationTarget::new(|_| {}));

            while let Some(child) = obj.first_child() {
                child.unparent();
            }
        }
    }

    impl LpImage {
        fn connect_controllers(&self) {
            let obj = self.instance();

            // Needed vor having the current cursor position available
            let motion_controller = gtk::EventControllerMotion::new();
            motion_controller.connect_enter(glib::clone!(@weak obj => move |_, x, y| {
                obj.imp().pointer_position.set(Some((x, y)));
            }));
            motion_controller.connect_motion(glib::clone!(@weak obj => move |_, x, y| {
                obj.imp().pointer_position.set(Some((x, y)));
            }));
            motion_controller.connect_leave(glib::clone!(@weak obj => move |_| {
                obj.imp().pointer_position.set(None);
            }));
            obj.add_controller(&motion_controller);

            // Zoom via scroll wheels etc
            let scroll_controller =
                gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);

            scroll_controller.connect_scroll(glib::clone!(@weak obj => @default-return gtk::Inhibit(false), move |event, _, y| {
                // use Ctrl key as modifier for vertical scrolling
                if event.current_event_state() == gdk::ModifierType::CONTROL_MASK
                    || event.current_event_state() == gdk::ModifierType::SHIFT_MASK
                {
                    // propagate event to scrolled window
                    return gtk::Inhibit(false);
                }

                // touchpads do zoom via gestures only
                if event.current_event_device().map(|x| x.source())
                    == Some(gdk::InputSource::Touchpad)
                {
                    // propagate event to scrolled window
                    return gtk::Inhibit(false);
                }

                let (zoom_factor, animated) = match event.unit() {
                    gdk::ScrollUnit::Wheel => (y.abs() * ZOOM_FACTOR_WHEEL, true),
                    // TODO: completely untested
                    gdk::ScrollUnit::Surface => (y.abs() * ZOOM_FACTOR_WHEEL_HI_RES + 1.0, false),
                    unknown_unit => {
                        log::warn!("Ignoring unknown scroll unit: {unknown_unit}");
                        (1., false)
                    }
                };

                let zoom = if y > 0. {
                    obj.imp().zoom_target.get() / zoom_factor
                } else {
                    obj.imp().zoom_target.get() * zoom_factor
                };

                if zoom > obj.zoom_level_best_fit() {
                    obj.set_best_fit(false);
                    if animated {
                        obj.zoom_to(zoom);
                    } else {
                        obj.set_zoom(zoom);
                    }
                } else {
                    obj.set_best_fit(true);
                    if animated {
                        obj.zoom_to(obj.zoom_level_best_fit());
                    } else {
                        obj.set_zoom(obj.zoom_level_best_fit());
                    }
                }

                // do not propagate event to scrolled window
                gtk::Inhibit(true)
            }));

            obj.add_controller(&scroll_controller);
        }

        fn connect_gestures(&self) {
            let obj = self.instance();

            // Drag for moving image around
            let drag_gesture = gtk::GestureDrag::new();
            obj.add_controller(&drag_gesture);

            drag_gesture.connect_drag_begin(glib::clone!(@weak obj => move |gesture, _, _| {
                if obj.is_hscrollable() || obj.is_vscrollable() {
                    obj.set_cursor(gdk::Cursor::from_name("grabbing", None).as_ref());
                    obj.imp().last_drag_value.set(Some((0., 0.)));
                } else {
                    // let drag and drop handle the events when not scrollable
                    gesture.set_state(gtk::EventSequenceState::Denied);
                }
            }));

            drag_gesture.connect_drag_update(glib::clone!(@weak obj => move |_, x1, y1| {
                if let Some((x0, y0)) = obj.imp().last_drag_value.get() {
                    let hadjustment = obj.hadjustment();
                    let vadjustment = obj.vadjustment();

                    hadjustment.set_value(hadjustment.value() - x1 + x0);
                    vadjustment.set_value(vadjustment.value() - y1 + y0);
                }

                obj.imp().last_drag_value.set(Some((x1, y1)));
            }));

            drag_gesture.connect_drag_end(glib::clone!(@weak obj => move |_, _, _| {
                obj.set_cursor(None);
                obj.imp().last_drag_value.set(None);
            }));

            // Rotate
            let rotation_gesture = gtk::GestureRotate::new();
            obj.add_controller(&rotation_gesture);

            rotation_gesture.connect_angle_changed(glib::clone!(@weak obj => move |_, angle, _| {
                // offset for rotate gesture to take effect
                if angle.abs().to_degrees() > 20. {
                    obj.set_rotation(obj.imp().rotation_target.get() + angle.to_degrees());
                }
            }));

            rotation_gesture.connect_end(glib::clone!(@weak obj => move |_, _| {
                log::debug!("Rotate gesture ended");

                let angle = (obj.rotation() / 90.).round() * 90. - obj.imp().rotation_target.get();
                obj.rotate_by(angle);
            }));

            // Zoom
            let zoom_gesture = gtk::GestureZoom::new();
            obj.add_controller(&zoom_gesture);

            zoom_gesture.connect_begin(glib::clone!(@weak obj => move |gesture, _| {
                obj.imp()
                    .zoom_gesture_center
                    .set(gesture.bounding_box_center());
            }));

            zoom_gesture.connect_scale_changed(glib::clone!(@weak obj => move |gesture, scale| {
                let hadjustment = obj.hadjustment();
                let vadjustment = obj.vadjustment();
                let zoom = obj.imp().zoom_target.get() * scale;

                // move image with fingers on touchscreens
                if gesture.device().map(|x| x.source()) == Some(gdk::InputSource::Touchscreen) {
                    if let p1 @ Some((x1, y1)) = gesture.bounding_box_center() {
                        if let Some((x0, y0)) = obj.imp().zoom_gesture_center.get() {
                            hadjustment.set_value(hadjustment.value() + x1 - x0);
                            vadjustment.set_value(vadjustment.value() + y1 - y0);
                        } else {
                            log::warn!("Zoom bounding box center: No previous value");
                        }

                        obj.imp().zoom_gesture_center.set(p1);
                    }
                }

                obj.set_zoom_aiming(zoom, obj.imp().zoom_gesture_center.get());
            }));

            zoom_gesture.connect_end(glib::clone!(@weak obj => move |_, _| {
                log::debug!("Zoom gesture ended");

                let rotation_target = (obj.rotation() / 90.).round() * 90.;
                if obj.zoom() < obj.zoom_level_best_fit_for_rotation(rotation_target) {
                    obj.zoom_to(obj.zoom_level_best_fit_for_rotation(rotation_target));
                } else {
                    // rubberband if over highest zoom level and sets `zoom_target`
                    obj.zoom_to(obj.zoom());
                };
            }));

            zoom_gesture.group_with(&rotation_gesture);
        }
    }

    impl WidgetImpl for LpImage {
        // called when the widget size might have changed
        fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
            dbg!("SIZE ALLOCAT");
            let widget = self.instance();

            // ensure there is an actual size change
            if self.widget_dimensions.get() != (width, height) {
                self.widget_dimensions.set((width, height));
                // calculate new zoom value for best fit
                if widget.is_best_fit() {
                    let best_fit_level = widget.zoom_level_best_fit();
                    self.zoom.set(best_fit_level);
                    self.zoom_target.set(best_fit_level);
                    self.instance().zoom_animation().pause();
                }
            }

            widget.configure_adjustments();
        }

        // called when the widget content should be re-rendered
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            if let Some(texture) = self.texture.borrow().as_ref() {
                let widget = self.instance();
                let widget_width = widget.width() as f64;
                let widget_height = widget.height() as f64;

                let zoom = widget.zoom() / widget.scale_factor() as f64;

                // make sure the scrollbars are correct
                widget.configure_adjustments();

                snapshot.save();

                // apply the scrolling position to the image
                if let Some(adj) = self.hadjustment.borrow().as_ref() {
                    let x = -(adj.value() - (adj.upper() - widget_width) / 2.);
                    snapshot.translate(&graphene::Point::new(x as f32, 0.));
                }
                if let Some(adj) = self.vadjustment.borrow().as_ref() {
                    let y = -(adj.value() - (adj.upper() - widget_height) / 2.);
                    snapshot.translate(&graphene::Point::new(0., y as f32));
                }

                // Center image in coordinates.
                // Needed for rotating around the center of the image, and
                // mirroring the image does not put it to a completely different position.
                snapshot.translate(&graphene::Point::new(
                    (widget_width / 2.) as f32,
                    (widget_height / 2.) as f32,
                ));

                // vertical centering in widget when no scrolling
                let x = f64::max((widget_width - widget.image_displayed_width()) / 2.0, 0.);
                snapshot.translate(&graphene::Point::new(x as f32, 0.));

                let y = f64::max((widget_height - widget.image_displayed_height()) / 2.0, 0.);
                snapshot.translate(&graphene::Point::new(0., y as f32));

                // apply the transformations from properties
                snapshot.rotate(widget.rotation() as f32);
                if widget.mirrored() {
                    snapshot.scale(-1., 1.);
                }
                snapshot.scale(zoom as f32, zoom as f32);

                // scale to actual pixel size
                // this is needed since usually the texture would just fill the widget
                snapshot.scale(
                    (texture.width() as f64 / widget_width) as f32,
                    (texture.height() as f64 / widget_height) as f32,
                );

                // move back to original position
                snapshot.translate(&graphene::Point::new(
                    (-widget_width / 2.) as f32,
                    (-widget_height / 2.) as f32,
                ));

                texture.snapshot(snapshot, widget_width, widget_height);
                snapshot.restore();
            }
        }
    }

    impl ScrollableImpl for LpImage {}
}

glib::wrapper! {
    pub struct LpImage(ObjectSubclass<imp::LpImage>)
        @extends gtk::Widget,
        @implements gtk::Scrollable;
}

impl LpImage {
    pub fn set_texture_with_file(&self, texture: gdk::Texture, source_file: &gio::File) {
        let imp = self.imp();

        imp.texture.replace(Some(texture));
        imp.file.replace(Some(source_file.clone()));

        let metadata = ImageMetadata::load(source_file);
        let orientation = metadata.orientation();

        imp.image_metadata.replace(metadata);

        imp.rotation_target.set(-orientation.rotation);
        imp.rotation.set(-orientation.rotation);
        imp.mirrored.set(orientation.mirrored);

        self.configure_adjustments();

        self.queue_draw();
        self.queue_allocate();
        self.queue_resize();
    }

    /// Zoom level that makes the image fit in widget
    ///
    /// During image rotation the image does not actually fit into widget.
    /// Instead the level is interpolated between zoom levels
    fn zoom_level_best_fit(&self) -> f64 {
        self.zoom_level_best_fit_for_rotation(self.rotation())
    }

    /// Same, but not for current rotation target
    ///
    /// Used for calculating the required zoom level after rotation
    fn zoom_level_best_fit_for_rotation(&self, rotation: f64) -> f64 {
        if let Some(texture) = self.texture() {
            let rotated = rotation.to_radians().sin().abs();
            let texture_aspect_ratio = texture.width() as f64 / texture.height() as f64;
            let widget_aspect_ratio = self.width() as f64 / self.height() as f64;
            let widget_phys_width = self.width() as f64 * self.scale_factor() as f64;
            let widget_phys_height = self.height() as f64 * self.scale_factor() as f64;

            let default_zoom = if texture_aspect_ratio > widget_aspect_ratio {
                (widget_phys_width / texture.width() as f64).min(1.)
            } else {
                (widget_phys_height / texture.height() as f64).min(1.)
            };

            let rotated_zoom = if 1. / texture_aspect_ratio > widget_aspect_ratio {
                (widget_phys_width / texture.height() as f64).min(1.)
            } else {
                (widget_phys_height / texture.width() as f64).min(1.)
            };

            rotated * rotated_zoom + (1. - rotated) * default_zoom
        } else {
            1.
        }
    }

    pub fn file(&self) -> Option<gio::File> {
        let imp = self.imp();

        imp.file.borrow().clone()
    }

    /// Texture that contains the original image
    pub fn texture(&self) -> Option<gdk::Texture> {
        let imp = self.imp();
        imp.texture.borrow().clone()
    }

    fn mirrored(&self) -> bool {
        self.imp().mirrored.get()
    }

    fn set_mirrored(&self, mirrored: bool) {
        if mirrored == self.mirrored() {
            return;
        }

        self.imp().mirrored.set(mirrored);
        self.notify("mirrored");
        self.queue_draw();
    }

    pub fn rotation(&self) -> f64 {
        self.imp().rotation.get()
    }

    pub fn set_rotation(&self, rotation: f64) {
        if rotation == self.rotation() {
            return;
        }

        self.imp().rotation.set(rotation);
        self.notify("rotation");
        self.queue_draw();
    }

    pub fn rotate_by(&self, angle: f64) {
        log::debug!("Rotate by {} degrees", angle);
        let target = &self.imp().rotation_target;
        target.set(target.get() + angle);

        let animation = self.rotation_animation();

        animation.set_value_from(self.rotation());
        animation.set_value_to(target.get());
        animation.play();

        if self.is_best_fit() {
            let animation = self.zoom_animation();

            animation.set_value_from(self.zoom());
            animation.set_value_to(self.zoom_level_best_fit_for_rotation(target.get()));
            animation.play();
        }
    }

    fn rotation_animation(&self) -> &adw::TimedAnimation {
        self.imp().rotation_animation.get_or_init(|| {
            adw::TimedAnimation::builder()
                .duration(ROTATION_ANIMATION_DURATION)
                .widget(self)
                .target(&adw::PropertyAnimationTarget::new(self, "rotation"))
                .build()
        })
    }

    pub fn is_best_fit(&self) -> bool {
        self.imp().best_fit.get()
    }

    pub fn set_best_fit(&self, best_fit: bool) {
        if best_fit == self.is_best_fit() {
            return;
        }

        self.imp().best_fit.set(best_fit);
        self.notify("best-fit");
    }

    /// Current zoom level
    pub fn zoom(&self) -> f64 {
        self.imp().zoom.get()
    }

    /// Set zoom level aiming for cursor position or center if not available
    ///
    /// Aiming means that the scrollbars are adjust such that the same point
    /// of the image remains under the cursor after changing the zoom level.
    fn set_zoom(&self, zoom: f64) {
        self.set_zoom_aiming(zoom, self.imp().pointer_position.get())
    }

    pub fn is_max_zoom(&self) -> bool {
        self.imp().max_zoom.get()
    }

    fn set_max_zoom(&self, value: bool) {
        if self.is_max_zoom() == value {
            return;
        }

        self.imp().max_zoom.set(value);
        self.notify("is-max-zoom");
    }

    /// Set zoom level aiming for given position or center if not available
    fn set_zoom_aiming(&self, zoom: f64, aiming: Option<(f64, f64)>) {
        if zoom == self.zoom() || zoom <= 0. {
            return;
        }

        let zoom_ratio = self.imp().zoom.get() / zoom;

        self.imp().zoom.set(zoom);

        self.configure_adjustments();

        let center_x = self.widget_width() / 2.;
        let center_y = self.widget_height() / 2.;

        let (x, y) = aiming.unwrap_or((center_x, center_y));

        if self.imp().zoom_hscrollbar_transition.get() {
            if zoom_ratio < 1. {
                self.hadjustment()
                    .set_value(self.max_hadjustment_value() / 2.);
            } else {
                // move towards center
                self.hadjustment()
                    .set_value(self.hadjustment_corrected_for_zoom(zoom_ratio, center_x));
            }
        } else {
            self.hadjustment()
                .set_value(self.hadjustment_corrected_for_zoom(zoom_ratio, x));
        }

        if self.imp().zoom_vscrollbar_transition.get() {
            if zoom_ratio < 1. {
                self.vadjustment()
                    .set_value(self.max_vadjustment_value() / 2.);
            } else {
                // move towards center
                self.vadjustment()
                    .set_value(self.vadjustment_corrected_for_zoom(zoom_ratio, center_y));
            }
        } else {
            self.vadjustment()
                .set_value(self.vadjustment_corrected_for_zoom(zoom_ratio, y));
        }

        self.notify("zoom");
        self.queue_draw();
    }

    /// Animation that makes larger zoom steps (from buttons etc) look smooth
    fn zoom_animation(&self) -> &adw::TimedAnimation {
        self.imp().zoom_animation.get_or_init(|| {
            let animation = adw::TimedAnimation::builder()
                .duration(ZOOM_ANIMATION_DURATION)
                .widget(self)
                .target(&adw::PropertyAnimationTarget::new(self, "zoom"))
                .build();

            animation.connect_done(glib::clone!(@weak self as obj => move |_| {
                obj.imp().zoom_hscrollbar_transition.set(false);
                obj.imp().zoom_vscrollbar_transition.set(false);
            }));

            animation
        })
    }

    /// Required scrollbar change to keep aiming
    ///
    /// When zooming by a ratio of `zoom_delta` and wanting to keep position `x`
    /// in the image at the same place in the widget, the returned value is
    /// the correct value for hadjustment to achive that.
    pub fn hadjustment_corrected_for_zoom(&self, zoom_delta: f64, x: f64) -> f64 {
        let adj = self.hadjustment();
        // Width of bars to the left and right of the image
        let border = if self.widget_width() > self.image_displayed_width() {
            (self.widget_width() - self.image_displayed_width()) / 2.
        } else {
            0.
        };

        f64::max((x + adj.value() - border) / zoom_delta - x, 0.)
    }

    /// Same but for vertical adjustment
    pub fn vadjustment_corrected_for_zoom(&self, zoom_delta: f64, y: f64) -> f64 {
        let adj = self.vadjustment();
        // Width of bars to the top and bottom of the image
        let border = if self.widget_height() > self.image_displayed_height() {
            (self.widget_height() - self.image_displayed_height()) / 2.
        } else {
            0.
        };

        f64::max((y + adj.value() - border) / zoom_delta - y, 0.)
    }

    /// Zoom in a step with animation
    ///
    /// Used by buttons
    pub fn zoom_in(&self) {
        let zoom = self.imp().zoom_target.get() * ZOOM_FACTOR_BUTTON;

        self.zoom_to(zoom);
    }

    /// Zoom out a step with animation
    ///
    /// Used by buttons
    pub fn zoom_out(&self) {
        let zoom = self.imp().zoom_target.get() / ZOOM_FACTOR_BUTTON;

        self.zoom_to(zoom);
    }

    /// Zoom to best fit
    ///
    /// Used by shortcut
    pub fn zoom_best_fit(&self) {
        self.zoom_to(self.zoom_level_best_fit());
    }

    /// Zoom to specific level with animation
    pub fn zoom_to(&self, mut zoom: f64) {
        if zoom >= MAX_ZOOM_LEVEL {
            zoom = MAX_ZOOM_LEVEL;
            self.set_max_zoom(true);
        } else {
            self.set_max_zoom(false);
        }

        // If image is only 1/4 of a zoom step away from best-fit, also
        // activate best-fit. This avoids bugs with floating point precision
        // and removes akward minimal zoom steps.
        let extended_best_fit_threshold =
            self.zoom_level_best_fit() * (1. + (ZOOM_FACTOR_BUTTON - 1.) / 4.);

        if zoom <= extended_best_fit_threshold {
            zoom = self.zoom_level_best_fit();
            self.set_best_fit(true);
        } else {
            self.set_best_fit(false);
        }

        log::debug!("Zoom to {zoom:.3}");

        self.imp().zoom_target.set(zoom);

        // abort if already at correct zoom level
        if zoom == self.zoom() {
            log::debug!("Already at correct zoom level");
            return;
        }

        if let Some(texture) = self.texture() {
            let current_hborder = self.widget_width() - self.image_displayed_width();
            let target_hborder = self.widget_width() - texture.width() as f64 * zoom;

            self.imp()
                .zoom_hscrollbar_transition
                .set(current_hborder.signum() != target_hborder.signum() && current_hborder != 0.);

            let current_vborder = self.widget_height() - self.image_displayed_height();
            let target_vborder = self.widget_height() - texture.height() as f64 * zoom;

            self.imp()
                .zoom_hscrollbar_transition
                .set(current_vborder.signum() != target_vborder.signum() && current_vborder != 0.);

            let animation = self.zoom_animation();

            animation.set_value_from(self.zoom());
            animation.set_value_to(zoom);
            animation.play();
        } else {
            log::error!("No texture to zoom");
        }
    }

    /// Image width with current zoom factor and rotation
    ///
    /// During rotation it is an interpolated size that does not
    /// represent the actual size. The size returned might well be
    /// larger than what can actually be displayed within the widget.
    pub fn image_displayed_width(&self) -> f64 {
        if let Some(texture) = self.texture() {
            let rotated = self.rotation().to_radians().sin().abs();

            ((1. - rotated) * texture.width() as f64 + rotated * texture.height() as f64)
                * self.zoom()
                / self.scale_factor() as f64
        } else {
            0.
        }
    }

    pub fn image_displayed_height(&self) -> f64 {
        if let Some(texture) = self.texture() {
            let rotated = self.rotation().to_radians().sin().abs();

            ((1. - rotated) * texture.height() as f64 + rotated * texture.width() as f64)
                * self.zoom()
                / self.scale_factor() as f64
        } else {
            0.
        }
    }

    fn hadjustment(&self) -> gtk::Adjustment {
        if let Some(adj) = self.imp().hadjustment.borrow().as_ref() {
            adj.clone()
        } else {
            log::debug!("Hadjustment not set yet: Using fake object");
            gtk::Adjustment::default()
        }
    }

    fn set_hadjustment(&self, adjustment: Option<gtk::Adjustment>) {
        if let Some(adj) = &adjustment {
            adj.connect_value_changed(glib::clone!(@weak self as obj => move |_| {
                obj.queue_draw();
            }));
            // TODO: needed?
            self.queue_allocate();
        }

        self.imp().hadjustment.replace(adjustment);
        self.configure_adjustments();
    }

    fn vadjustment(&self) -> gtk::Adjustment {
        if let Some(adj) = self.imp().vadjustment.borrow().as_ref() {
            adj.clone()
        } else {
            log::debug!("Vadjustment not set yet: Using fake object");
            gtk::Adjustment::default()
        }
    }

    fn set_vadjustment(&self, adjustment: Option<gtk::Adjustment>) {
        if let Some(adj) = &adjustment {
            adj.connect_value_changed(glib::clone!(@weak self as obj => move |_| {
                obj.queue_draw();
            }));
            self.queue_allocate();
        }

        self.imp().vadjustment.replace(adjustment);
        self.configure_adjustments();
    }

    /// Configure scrollbars for current situation
    fn configure_adjustments(&self) {
        let hadjustment = self.hadjustment();
        let content_width = self.image_displayed_width();
        let widget_width = self.widget_width();

        hadjustment.configure(
            hadjustment.value().clamp(0., self.max_hadjustment_value()),
            0.,
            content_width,
            // arrow button step (probably irrelevant)
            widget_width * 0.1,
            // page up/down step
            widget_width * 0.9,
            widget_width.min(content_width),
        );

        let vadjustment = self.vadjustment();
        let content_height = self.image_displayed_height();
        let widget_height = self.widget_height();

        vadjustment.configure(
            vadjustment.value().clamp(0., self.max_vadjustment_value()),
            0.,
            content_height,
            // arrow button step (probably irrelevant)
            widget_height * 0.1,
            // page up/down step
            widget_height * 0.9,
            widget_height.min(content_height),
        );
    }

    pub fn max_hadjustment_value(&self) -> f64 {
        f64::max(self.image_displayed_width() - self.widget_width(), 0.)
    }

    pub fn max_vadjustment_value(&self) -> f64 {
        f64::max(self.image_displayed_height() - self.widget_height(), 0.)
    }

    pub fn is_hscrollable(&self) -> bool {
        self.max_hadjustment_value() != 0.
    }

    pub fn is_vscrollable(&self) -> bool {
        self.max_vadjustment_value() != 0.
    }

    pub fn widget_height(&self) -> f64 {
        self.height() as f64
    }

    pub fn widget_width(&self) -> f64 {
        self.width() as f64
    }

    /// Drag and drop content provider
    pub fn content_provider(&self) -> Option<gdk::ContentProvider> {
        let file = self.file()?;
        let list = gdk::FileList::from_array(&[file]);
        Some(gdk::ContentProvider::for_value(&list.to_value()))
    }
}
