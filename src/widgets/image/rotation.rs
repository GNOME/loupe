// Copyright (c) 2023-2024 Sophie Herold
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
    pub fn set_rotation(&self, rotation: f64) {
        let obj = self.obj();

        if rotation == obj.rotation() {
            return;
        }

        self.rotation.set(rotation);
        self.configure_adjustments();
        obj.notify_rotation();
        obj.queue_draw();
    }

    pub(super) fn set_mirrored(&self, mirrored: bool) {
        let obj = self.obj();

        if mirrored == obj.mirrored() {
            return;
        }

        self.mirrored.set(mirrored);
        obj.notify_mirrored();
        obj.queue_draw();
    }

    pub(super) fn rotation_animation(&self) -> &adw::TimedAnimation {
        let obj = self.obj().to_owned();
        self.rotation_animation.get_or_init(|| {
            adw::TimedAnimation::builder()
                .duration(ROTATION_ANIMATION_DURATION)
                .widget(&obj)
                .target(&adw::PropertyAnimationTarget::new(&obj, "rotation"))
                .build()
        })
    }
}

impl LpImage {
    /// Set rotation and mirroring to the state would have after loading
    pub fn reset_rotation(&self) {
        let orientation = self.metadata().orientation();
        self.imp().rotation_target.set(-orientation.rotation);
        self.set_mirrored(orientation.mirrored);
        self.set_rotation(-orientation.rotation);
    }

    pub fn rotate_by(&self, angle: f64) {
        let imp = self.imp();

        log::debug!("Rotate by {} degrees", angle);
        let target = &self.imp().rotation_target;
        target.set(target.get() + angle);

        let animation = imp.rotation_animation();

        animation.set_value_from(self.rotation());
        animation.set_value_to(target.get());
        animation.play();

        if self.is_best_fit() {
            let animation = imp.zoom_animation();

            animation.set_value_from(self.zoom());
            animation.set_value_to(imp.zoom_level_best_fit_for_rotation(target.get()));
            animation.play();
        }
    }
}
