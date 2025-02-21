// Copyright (c) 2023-2025 Sophie Herold
// Copyright (c) 2024 Fina Wilke
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

use super::*;

impl imp::LpImage {
    /// To be called by constructor
    pub(super) fn connect_input_handling(&self) {
        self.connect_controllers();
        self.connect_gestures();
    }

    fn connect_controllers(&self) {
        let obj = self.obj();

        // Needed for having the current cursor position available
        let motion_controller = gtk::EventControllerMotion::new();
        motion_controller.connect_enter(glib::clone!(
            #[weak]
            obj,
            move |_, x, y| {
                obj.imp().pointer_position.set(Some((x, y)));
            }
        ));
        motion_controller.connect_motion(glib::clone!(
            #[weak]
            obj,
            move |_, x, y| {
                obj.imp().pointer_position.set(Some((x, y)));
            }
        ));
        motion_controller.connect_leave(glib::clone!(
            #[weak]
            obj,
            move |_| {
                obj.imp().pointer_position.set(None);
            }
        ));
        obj.add_controller(motion_controller);

        // Zoom via scroll wheels etc
        let scroll_controller =
            gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);

        scroll_controller.connect_scroll_end(glib::clone!(
            #[weak]
            obj,
            move |event| {
                // Avoid kinetc scrolling in scrolled window after zooming
                if event
                    .current_event_state()
                    .contains(gdk::ModifierType::CONTROL_MASK)
                {
                    obj.imp().cancel_deceleration();
                }
            }
        ));

        scroll_controller.connect_scroll(glib::clone!(
            #[weak]
            obj,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |event, _, y| {
                let imp = obj.imp();
                let state = event.current_event_state();
                if event.current_event_device().map(|x| x.source())
                    == Some(gdk::InputSource::Touchpad)
                {
                    // Touchpads do zoom via gestures, expect when Ctrl key is pressed
                    if !state.contains(gdk::ModifierType::CONTROL_MASK) {
                        // propagate event to scrolled window
                        return glib::Propagation::Proceed;
                    }
                } else {
                    // use Ctrl key as modifier for vertical scrolling
                    if state.contains(gdk::ModifierType::CONTROL_MASK)
                        || state.contains(gdk::ModifierType::SHIFT_MASK)
                    {
                        // propagate event to scrolled window
                        return glib::Propagation::Proceed;
                    }
                }

                // Use exponential scaling since zoom is always multiplicative with the existing
                // value This is the right thing since `exp(n/2)^2 == exp(n)`
                // (two small steps are the same as one larger step)
                let (zoom_factor, animated) = match event.unit() {
                    gdk::ScrollUnit::Wheel => (
                        f64::exp(-y * f64::ln(ZOOM_FACTOR_SCROLL_WHEEL)),
                        y.abs() >= 1.,
                    ),
                    gdk::ScrollUnit::Surface => {
                        (f64::exp(-y * f64::ln(ZOOM_FACTOR_SCROLL_SURFACE)), false)
                    }
                    unknown_unit => {
                        log::warn!("Ignoring unknown scroll unit: {unknown_unit:?}");
                        (1., false)
                    }
                };

                let zoom = imp.zoom_target.get() * zoom_factor;

                if animated {
                    obj.zoom_to(zoom);
                } else {
                    imp.zoom_to_full(zoom, false, false, false);
                }

                // do not propagate event to scrolled window
                glib::Propagation::Stop
            }
        ));

        obj.add_controller(scroll_controller);
    }

    fn connect_gestures(&self) {
        let obj = self.obj();

        // Double click for fullscreen (mouse/touchpad) or zoom (touch screen)
        let left_click_gesture = gtk::GestureClick::builder().button(1).build();
        obj.add_controller(left_click_gesture.clone());
        left_click_gesture.connect_pressed(glib::clone!(
            #[weak]
            obj,
            move |gesture, n_press, x, y| {
                // only handle double clicks
                if n_press != 2 {
                    log::trace!("Gesture {n_press} click");
                    gesture.set_state(gtk::EventSequenceState::Denied);
                    return;
                }

                log::trace!("Gesture double click");

                gesture.set_state(gtk::EventSequenceState::Claimed);
                if gesture.device().map(|x| x.source()) == Some(gdk::InputSource::Touchscreen) {
                    // zoom
                    obj.imp().pointer_position.set(Some((x, y)));
                    if obj.is_best_fit() {
                        // zoom in
                        obj.zoom_to(ZOOM_FACTOR_DOUBLE_TAP * obj.imp().zoom_level_best_fit());
                    } else {
                        // zoom back out
                        obj.zoom_best_fit();
                    }
                } else {
                    // fullscreen
                    obj.activate_action("win.toggle-fullscreen", None).unwrap();
                }
            }
        ));

        // Drag for moving image around
        let drag_gesture = gtk::GestureDrag::builder().button(0).build();
        obj.add_controller(drag_gesture.clone());

        drag_gesture.connect_drag_begin(glib::clone!(
            #[weak]
            obj,
            move |gesture, _, _| {
                log::trace!("Drag gesture begin");
                let imp = obj.imp();

                // Allow only left and middle button
                if ![1, 2].contains(&gesture.current_button())
                // Drag gesture for touchscreens is handled by ScrolledWindow
                || gesture.device().map(|x| x.source()) == Some(gdk::InputSource::Touchscreen)
                {
                    gesture.set_state(gtk::EventSequenceState::Denied);
                    return;
                }

                if obj.is_hscrollable() || obj.is_vscrollable() {
                    imp.cancel_deceleration();
                    obj.set_cursor(gdk::Cursor::from_name("grabbing", None).as_ref());
                    imp.last_drag_value.set(Some((0., 0.)));
                } else {
                    // let drag and drop handle the events when not scrollable
                    gesture.set_state(gtk::EventSequenceState::Denied);
                }
            }
        ));

        drag_gesture.connect_drag_update(glib::clone!(
            #[weak]
            obj,
            move |_, x1, y1| {
                let imp = obj.imp();
                if let Some((x0, y0)) = obj.imp().last_drag_value.get() {
                    imp.set_hadj_value(imp.hadj_value() - x1 + x0);
                    imp.set_vadj_value(imp.vadj_value() - y1 + y0);
                }

                obj.imp().last_drag_value.set(Some((x1, y1)));
            }
        ));

        drag_gesture.connect_drag_end(glib::clone!(
            #[weak]
            obj,
            move |_, _, _| {
                log::trace!("Drag gesture end");
                obj.set_cursor(None);
                obj.imp().last_drag_value.set(None);
            }
        ));

        // Rotate
        let rotation_gesture = gtk::GestureRotate::new();
        obj.add_controller(rotation_gesture.clone());

        rotation_gesture.connect_begin(glib::clone!(
            #[weak]
            obj,
            move |_, _| {
                log::trace!("Rotate gesture begin");
                obj.imp().cancel_deceleration();
            }
        ));

        rotation_gesture.connect_angle_changed(glib::clone!(
            #[weak]
            obj,
            move |gesture, _, _| {
                let angle = -gesture.angle_delta().to_degrees();

                // Only reset rotation if scale gesture is locked in
                if let Some(Gesture::Scale) = obj.imp().locked_gestured.get() {
                    obj.imp().rotation.set(obj.imp().rotation_target.get());
                    return;
                }

                // Correct angle by the the angle at the moment of passing the threshold.
                // This stops the rotation from suddenly jumping when passing the threshold.
                let correction =
                    if let Some(Gesture::Rotate(correction)) = obj.imp().locked_gestured.get() {
                        correction
                    } else if angle.abs() > ROTATE_GESTURE_LOCK_THRESHOLD {
                        let correction = angle.signum() * ROTATE_GESTURE_LOCK_THRESHOLD;
                        obj.imp()
                            .locked_gestured
                            .set(Some(Gesture::Rotate(correction)));
                        correction
                    } else {
                        return;
                    };

                obj.set_rotation(obj.imp().rotation_target.get() + angle - correction);
            }
        ));

        rotation_gesture.connect_end(glib::clone!(
            #[weak]
            obj,
            move |_, _| {
                log::debug!("Rotate gesture end");

                let angle = (obj.rotation() / 90.).round() * 90. - obj.imp().rotation_target.get();
                obj.rotate_by(angle);
                obj.imp().locked_gestured.set(None);
            }
        ));

        // Zoom
        let zoom_gesture = gtk::GestureZoom::new();
        obj.add_controller(zoom_gesture.clone());

        zoom_gesture.connect_begin(glib::clone!(
            #[weak]
            obj,
            move |gesture, _| {
                log::trace!("Zoom gesture begin");

                let imp = obj.imp();
                imp.cancel_deceleration();
                imp.zoom_gesture_center.set(gesture.bounding_box_center());
            }
        ));

        zoom_gesture.connect_scale_changed(glib::clone!(
            #[weak]
            obj,
            move |gesture, scale| {
                let imp = obj.imp();

                let zoom = imp.zoom_target.get() * scale;

                // Move image with fingers on touchscreens
                if gesture.device().map(|x| x.source()) == Some(gdk::InputSource::Touchscreen) {
                    if let p1 @ Some((x1, y1)) = gesture.bounding_box_center() {
                        if let Some((x0, y0)) = imp.zoom_gesture_center.get() {
                            imp.set_hadj_value(imp.hadj_value() + x0 - x1);
                            imp.set_vadj_value(imp.vadj_value() + y0 - y1);
                        } else {
                            log::warn!("Zoom bounding box center: No previous value");
                        }

                        imp.zoom_gesture_center.set(p1);
                    }
                }

                let zoom_out_threshold = 1. / ZOOM_GESTURE_LOCK_THRESHOLD;
                let zoom_in_threshold = ZOOM_GESTURE_LOCK_THRESHOLD;

                if let Some(Gesture::Rotate(_)) = imp.locked_gestured.get() {
                    // Do not zoom when rotate is locked in
                    return;
                } else if !(zoom_out_threshold..zoom_in_threshold).contains(&scale) {
                    // Lock in scale when leaving the scale threshold
                    imp.locked_gestured.set(Some(Gesture::Scale));
                }

                imp.set_zoom_aiming(zoom, imp.zoom_gesture_center.get());
            }
        ));

        zoom_gesture.connect_end(glib::clone!(
            #[weak]
            obj,
            move |_, _| {
                let imp = obj.imp();
                log::debug!("Zoom gesture end");

                let rotation_target = (obj.rotation() / 90.).round() * 90.;
                if obj.zoom() < imp.zoom_level_best_fit_for_rotation(rotation_target)
                    && obj.fit_mode() != FitMode::ExactVolatile
                {
                    obj.zoom_to(imp.zoom_level_best_fit_for_rotation(rotation_target));
                } else {
                    // rubberband if over highest zoom level and sets `zoom_target`
                    obj.zoom_to(obj.zoom());
                };

                imp.locked_gestured.set(None);
            }
        ));

        zoom_gesture.group_with(&rotation_gesture);
    }

    /// Cancel kinetic scrolling movements, needed for some gestures
    ///
    /// If deceleration is not canceled gestures become buggy.
    fn cancel_deceleration(&self) {
        if let Some(scrolled_window) = self
            .obj()
            .parent()
            .and_then(|x| x.downcast::<gtk::ScrolledWindow>().ok())
        {
            scrolled_window.set_kinetic_scrolling(false);
            scrolled_window.set_kinetic_scrolling(true);
        } else {
            log::error!("Could not find GtkScrolledWindow parent to cancel deceleration");
        }
    }
}
