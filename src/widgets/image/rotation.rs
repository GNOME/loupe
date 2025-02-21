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
            let animation = adw::TimedAnimation::builder()
                .duration(ROTATION_ANIMATION_DURATION)
                .widget(&obj)
                .target(&adw::PropertyAnimationTarget::new(&obj, "rotation"))
                .build();

            animation.connect_done(glib::clone!(
                #[weak]
                obj,
                move |_| {
                    let imp = obj.imp();
                    if let Some(new_file) = imp.queued_reload.replace(None) {
                        glib::spawn_future_local(async move {
                            log::debug!("Animation finished: Executing delayed reload");
                            obj.load(&new_file).await;
                        });
                    }
                }
            ));

            animation
        })
    }
}

impl LpImage {
    /// Set rotation and mirroring to the state would have after loading
    pub fn reset_rotation(&self) {
        log::debug!("Resetting rotation");
        let orientation = self.metadata().orientation();
        self.imp()
            .rotation_target
            .set(orientation.rotate().degrees() as f64);
        self.set_mirrored(orientation.mirror());
        self.set_rotation(orientation.rotate().degrees() as f64);
    }

    pub fn rotate_by(&self, angle: f64) {
        let imp = self.imp();

        log::debug!("Rotate by {} degrees", angle);
        let target = &self.imp().rotation_target;
        target.set(target.get() + angle);

        let rotation_animation = imp.rotation_animation();

        rotation_animation.set_value_from(self.rotation());
        rotation_animation.set_value_to(target.get());
        rotation_animation.play();

        if self.is_best_fit() {
            let zoom_animation = imp.zoom_animation();

            zoom_animation.set_value_from(self.zoom());
            zoom_animation.set_value_to(imp.zoom_level_best_fit_for_rotation(target.get()));
            zoom_animation.play();
        }

        if let Ok(r) = gufo_common::orientation::Rotation::try_from(angle) {
            if r != gufo_common::orientation::Rotation::_0 {
                log::debug!("Editing image to rotate by {r:?}");
                let operation = glycin::Operation::Rotate(r);
                let editing_queue = &self.imp().editing_queue;
                editing_queue.push(operation);
                editing_queue.write_to_image(self);
            }
        }
    }
}
