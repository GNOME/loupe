// Copyright (c) 2023 Sophie Herold
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
    /// Set zoom level aiming for cursor position or center if not available
    ///
    /// Aiming means that the scrollbars are adjust such that the same point
    /// of the image remains under the cursor after changing the zoom level.
    pub(super) fn set_zoom(&self, zoom: f64) {
        self.obj()
            .set_zoom_aiming(zoom, self.pointer_position.get())
    }
}

impl LpImage {
    /// Zoom level that makes the image fit in widget
    ///
    /// During image rotation the image does not actually fit into widget.
    /// Instead the level is interpolated between zoom levels
    pub(super) fn zoom_level_best_fit(&self) -> f64 {
        self.zoom_level_best_fit_for_rotation(self.rotation())
    }

    /// Same, but not for current rotation target
    ///
    /// Used for calculating the required zoom level after rotation
    pub(super) fn zoom_level_best_fit_for_rotation(&self, rotation: f64) -> f64 {
        let rotated = rotation.to_radians().sin().abs();
        let (image_width, image_height) = (
            self.original_dimensions().0 as f64 / self.scale_factor() as f64,
            self.original_dimensions().1 as f64 / self.scale_factor() as f64,
        );
        let texture_aspect_ratio = image_width / image_height;
        let widget_aspect_ratio = self.width() as f64 / self.height() as f64;

        let default_zoom = if texture_aspect_ratio > widget_aspect_ratio {
            (self.width() as f64 / image_width).min(1.)
        } else {
            (self.height() as f64 / image_height).min(1.)
        };

        let rotated_zoom = if 1. / texture_aspect_ratio > widget_aspect_ratio {
            (self.width() as f64 / image_height).min(1.)
        } else {
            (self.height() as f64 / image_width).min(1.)
        };

        rotated * rotated_zoom + (1. - rotated) * default_zoom
    }

    /// Sets respective output values if best-fit is active
    pub(super) fn configure_best_fit(&self) {
        // calculate new zoom value for best fit
        if self.is_best_fit() {
            let best_fit_level = self.zoom_level_best_fit();
            self.imp().zoom.set(best_fit_level);
            self.set_zoom_target(best_fit_level);
            self.zoom_animation().pause();
        }
    }

    pub fn is_best_fit(&self) -> bool {
        self.imp().best_fit.get()
    }

    pub(super) fn applicable_zoom(&self) -> f64 {
        decoder::tiling::zoom_normalize(self.zoom()) / self.scale_factor() as f64
    }

    /// Maximal zoom allowed for this image
    pub(super) fn max_zoom(&self) -> f64 {
        if self.metadata().format().map_or(false, |x| x.is_svg()) {
            let (width, height) = self.original_dimensions();
            // Avoid division by 0
            let long_side = f64::max(1., i32::max(width, height) as f64);
            // Limit to maz size supported by rsvg
            f64::min(MAX_ZOOM_LEVEL, decoder::RSVG_MAX_SIZE as f64 / long_side)
        } else {
            MAX_ZOOM_LEVEL
        }
    }

    /// Set zoom level aiming for given position or center if not available
    pub(super) fn set_zoom_aiming(&self, mut zoom: f64, aiming: Option<(f64, f64)>) {
        let max_zoom = self.max_zoom();

        // allow some deviation from max value for rubberbanding
        if zoom > max_zoom {
            let max_deviation = max_zoom * ZOOM_FACTOR_MAX_RUBBERBAND;
            let deviation = zoom / max_zoom;
            zoom = f64::min(
                max_zoom * deviation.powf(RUBBERBANDING_EXPONENT),
                max_deviation,
            );
        }

        if zoom < self.zoom_level_best_fit() {
            let minimum = self.zoom_level_best_fit();
            let max_deviation = minimum / ZOOM_FACTOR_MAX_RUBBERBAND;
            let deviation = zoom / minimum;
            zoom = f64::max(
                minimum * deviation.powf(RUBBERBANDING_EXPONENT),
                max_deviation,
            );
        }

        if zoom == self.zoom() {
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
                self.set_hadj_value(self.max_hadjustment_value() / 2.);
            } else {
                // move towards center
                self.set_hadj_value(self.hadjustment_corrected_for_zoom(zoom_ratio, center_x));
            }
        } else {
            self.set_hadj_value(self.hadjustment_corrected_for_zoom(zoom_ratio, x));
        }

        if self.imp().zoom_vscrollbar_transition.get() {
            if zoom_ratio < 1. {
                self.set_vadj_value(self.max_vadjustment_value() / 2.);
            } else {
                // move towards center
                self.set_vadj_value(self.vadjustment_corrected_for_zoom(zoom_ratio, center_y));
            }
        } else {
            self.set_vadj_value(self.vadjustment_corrected_for_zoom(zoom_ratio, y));
        }

        self.notify_zoom();
        self.queue_draw();
    }

    pub(super) fn set_zoom_target(&self, zoom_target: f64) {
        log::debug!("Setting zoom target {zoom_target}");

        self.imp().zoom_target.set(zoom_target);

        if self.zoom() == self.imp().zoom_target.get() {
            self.request_tiles();
        }
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
    pub fn zoom_to(&self, zoom: f64) {
        self.zoom_to_full(zoom, true, true);
    }

    /// Zoom to specific level with animation not snapping to best-fit
    ///
    /// Used for zooming to 100% or 200%
    pub fn zoom_to_exact(&self, zoom: f64) {
        self.zoom_to_full(zoom, true, false);
    }

    pub(super) fn zoom_to_full(&self, mut zoom: f64, animated: bool, snap_best_fit: bool) {
        let max_zoom = self.max_zoom();
        if zoom >= max_zoom {
            zoom = max_zoom;
            self.set_is_max_zoom(true);
        } else {
            self.set_is_max_zoom(false);
        }

        let extended_best_fit_threshold = if snap_best_fit {
            // If image is only 1/4 of a zoom step away from best-fit, also
            // activate best-fit. This avoids bugs with floating point precision
            // and removes awkward minimal zoom steps.
            self.zoom_level_best_fit() * (1. + (ZOOM_FACTOR_BUTTON - 1.) / 4.)
        } else {
            self.zoom_level_best_fit()
        };

        if zoom <= extended_best_fit_threshold {
            zoom = self.zoom_level_best_fit();
            self.set_best_fit(true);
        } else {
            self.set_best_fit(false);
        }

        log::debug!("Zoom to {zoom:.3}");

        self.set_zoom_target(zoom);

        // abort if already at correct zoom level
        if zoom == self.zoom() {
            log::debug!("Already at correct zoom level");
            return;
        }

        if animated {
            // wild code
            let current_hborder = self.widget_width() - self.image_displayed_width();
            let target_hborder = self.widget_width() - self.image_size().0 as f64 * zoom;

            self.imp()
                .zoom_hscrollbar_transition
                .set(current_hborder.signum() != target_hborder.signum() && current_hborder != 0.);

            let current_vborder = self.widget_height() - self.image_displayed_height();
            let target_vborder = self.widget_height() - self.image_size().1 as f64 * zoom;

            self.imp()
                .zoom_hscrollbar_transition
                .set(current_vborder.signum() != target_vborder.signum() && current_vborder != 0.);

            let animation = self.zoom_animation();

            animation.set_value_from(self.zoom());
            animation.set_value_to(zoom);
            animation.play();
        } else {
            self.set_zoom(zoom);
            self.set_zoom_target(zoom);
        }
    }

    /// Animation that makes larger zoom steps (from buttons etc) look smooth
    pub(super) fn zoom_animation(&self) -> &adw::TimedAnimation {
        self.imp().zoom_animation.get_or_init(|| {
            let animation = adw::TimedAnimation::builder()
                .duration(ZOOM_ANIMATION_DURATION)
                .widget(self)
                .target(&adw::PropertyAnimationTarget::new(self, "zoom"))
                .build();

            animation.connect_done(glib::clone!(@weak self as obj => move |_| {
                obj.imp().zoom_hscrollbar_transition.set(false);
                obj.imp().zoom_vscrollbar_transition.set(false);
                obj.set_zoom_target(obj.imp().zoom_target.get());
            }));

            animation
        })
    }

    /// Required scrollbar change to keep aiming
    ///
    /// When zooming by a ratio of `zoom_delta` and wanting to keep position `x`
    /// in the image at the same place in the widget, the returned value is
    /// the correct value for hadjustment to achieve that.
    pub fn hadjustment_corrected_for_zoom(&self, zoom_delta: f64, x: f64) -> f64 {
        // Width of bars to the left and right of the image
        let border = if self.widget_width() > self.image_displayed_width() {
            (self.widget_width() - self.image_displayed_width()) / 2.
        } else {
            0.
        };

        f64::max((x + self.hadj_value() - border) / zoom_delta - x, 0.)
    }

    /// Same but for vertical adjustment
    pub fn vadjustment_corrected_for_zoom(&self, zoom_delta: f64, y: f64) -> f64 {
        // Width of bars to the top and bottom of the image
        let border = if self.widget_height() > self.image_displayed_height() {
            (self.widget_height() - self.image_displayed_height()) / 2.
        } else {
            0.
        };

        f64::max((y + self.vadj_value() - border) / zoom_delta - y, 0.)
    }
}
