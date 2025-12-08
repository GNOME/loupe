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
        self.connect_gestures();
    }

    fn connect_gestures(&self) {
        let obj = self.obj();

        // Double click for fullscreen (mouse/touchpad) or zoom (touch screen)
        let left_click_gesture = gtk::GestureClick::builder().button(1).build();
        obj.add_controller(left_click_gesture.clone());
        left_click_gesture.connect_pressed(glib::clone!(
            #[weak]
            obj,
            move |gesture, n_press, _, _| {
                // only handle double clicks
                if n_press != 2 {
                    log::trace!("Gesture {n_press} click");
                    return;
                }

                log::trace!("Gesture double click");
                gesture.set_state(gtk::EventSequenceState::Claimed);
                obj.activate_action("win.toggle-fullscreen", None).unwrap();
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
