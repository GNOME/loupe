// Copyright (c) 2023-2025 Sophie Herold
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
//
// Copyright (c) 2023 Lubosz Sarnecki

use super::*;

/// Manage visibility of overlay controls
impl LpImageWindow {
    /// Set control opacity
    ///
    /// Also changes headerbar transparency if it flat and hides/shows cursor on
    /// fullscreen
    fn set_control_opacity(&self, opacity: f64, hiding: bool) {
        if let Some(window) = self.try_window() {
            self.image_view().controls_box_start().set_opacity(opacity);
            self.image_view().controls_box_end().set_opacity(opacity);

            if self.is_headerbar_flat() && window.is_fullscreen() {
                self.headerbar().set_opacity(opacity);
            } else {
                self.headerbar().set_opacity(1.);
            }

            if window.is_fullscreen() && hiding && opacity < 0.9 {
                self.set_cursor(gdk::Cursor::from_name("none", None).as_ref());
            } else {
                self.set_cursor(None);
            }
        }
    }

    pub fn controls_opacity(&self) -> f64 {
        self.image_view().controls_box_start().opacity()
    }

    /// Animation to show controls
    fn show_controls_animation(&self) -> &adw::TimedAnimation {
        self.imp().show_controls_animation.get_or_init(|| {
            let target = adw::CallbackAnimationTarget::new(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                move |opacity| obj.set_control_opacity(opacity, false)
            ));

            adw::TimedAnimation::builder()
                .duration(SHOW_CONTROLS_ANIMATION_DURATION)
                .widget(&self.image_view())
                .target(&target)
                .value_to(1.)
                .build()
        })
    }

    /// Animation to hide controls
    fn hide_controls_animation(&self) -> &adw::TimedAnimation {
        self.imp().hide_controls_animation.get_or_init(|| {
            let target = adw::CallbackAnimationTarget::new(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                move |opacity| obj.set_control_opacity(opacity, true)
            ));

            adw::TimedAnimation::builder()
                .duration(HIDE_CONTROLS_ANIMATION_DURATION)
                .widget(&self.image_view())
                .target(&target)
                .value_to(0.)
                .build()
        })
    }

    /// Schedule a fade animation to play after `HIDE_CONTROLS_IDLE_TIMEOUT` of
    /// inactivity
    pub fn schedule_hide_controls(&self) {
        self.unschedule_hide_controls();

        let new_timeout = glib::timeout_add_local_once(
            std::time::Duration::from_millis(HIDE_CONTROLS_IDLE_TIMEOUT),
            glib::clone!(
                #[weak(rename_to = win)]
                self,
                move || {
                    win.imp().hide_controls_timeout.take();
                    win.hide_controls();
                }
            ),
        );

        self.imp().hide_controls_timeout.replace(Some(new_timeout));
    }

    fn unschedule_hide_controls(&self) {
        if let Some(current_timeout) = self.imp().hide_controls_timeout.take() {
            current_timeout.remove();
        }
    }

    pub fn are_controls_visible(&self) -> bool {
        if self.hide_controls_animation().state() == adw::AnimationState::Playing {
            return false;
        }

        self.image_view().controls_box_start().opacity() == 1.
            || self.show_controls_animation().state() == adw::AnimationState::Playing
    }

    /// Start animation to show controls
    pub fn show_controls(&self) {
        if !self.are_controls_visible() {
            self.hide_controls_animation().pause();
            self.show_controls_animation()
                .set_value_from(self.controls_opacity());
            self.show_controls_animation().play();
        }
    }

    /// Start animation to hide controls
    pub fn hide_controls(&self) {
        if self.are_controls_visible() && !self.image_view().zoom_menu_button().is_active() {
            self.show_controls_animation().pause();
            self.hide_controls_animation()
                .set_value_from(self.controls_opacity());
            self.hide_controls_animation().play();
        }
    }

    pub fn on_click(&self) {
        self.show_controls();

        if self.can_hide_controls() {
            self.schedule_hide_controls();
        } else {
            self.unschedule_hide_controls();
        }
    }

    pub fn on_motion(&self, pointer_position: (f64, f64)) {
        let imp = self.imp();

        // Check if position really changed since swipe gesture sends change event with
        // same position. Also, don't connect this to "leave" because swipe also
        // sends fake "leave" events when the widget under the cursor changes.
        if imp.pointer_position.get() == pointer_position {
            return;
        }
        imp.pointer_position.set(pointer_position);

        self.show_controls();

        if self.can_hide_controls() {
            self.schedule_hide_controls();
        } else {
            self.unschedule_hide_controls();
        }
    }

    /// Returns `true` if controls can be hidden
    ///
    /// Only hide controls if cursor not over controls and there is an image
    /// shown
    fn can_hide_controls(&self) -> bool {
        let imp = self.imp();

        let controls_hovered = self
            .image_view()
            .controls_box_start_events()
            .contains_pointer()
            || self
                .image_view()
                .controls_box_end_events()
                .contains_pointer();

        let headerbar_hideable = self.is_content_extended_to_top();
        let headerbar_hovered = imp.headerbar_events.contains_pointer();

        let main_menu_open = imp.primary_menu.is_active();

        // Buttom controls are not hovered
        !controls_hovered
        // Either headerbar must not be hidden with other controls or not be hovered
        && (!headerbar_hideable || !headerbar_hovered)
        // Main menu is not open
        && !main_menu_open
    }
}
