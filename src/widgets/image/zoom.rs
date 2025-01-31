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
    /// Set zoom level aiming for cursor position or center if not available
    ///
    /// Aiming means that the scrollbars are adjust such that the same point
    /// of the image remains under the cursor after changing the zoom level.
    pub(super) fn set_zoom(&self, zoom: f64) {
        self.set_zoom_aiming(zoom, self.pointer_position.get())
    }

    /// Zoom level that makes the image fit in widget
    ///
    /// During image rotation the image does not actually fit into widget.
    /// Instead the level is interpolated between zoom levels
    pub(super) fn zoom_level_best_fit(&self) -> f64 {
        self.zoom_level_best_fit_for_rotation(self.obj().rotation())
    }

    /// Same, but not for current rotation target
    ///
    /// Used for calculating the required zoom level after rotation
    pub(super) fn zoom_level_best_fit_for_rotation(&self, rotation: f64) -> f64 {
        let rotated = rotation.to_radians().sin().abs();
        let (image_width, image_height) = (
            self.untransformed_dimensions().0 as f64 / self.scaling(),
            self.untransformed_dimensions().1 as f64 / self.scaling(),
        );
        let texture_aspect_ratio = image_width / image_height;
        let widget_aspect_ratio = self.widget_width() / self.widget_height();

        let max_zoom_factor = match self.fit_mode.get() {
            // Do not allow to zoom larger than original size
            FitMode::BestFit => 1.,
            // Allow arbitrary zoom
            FitMode::LargeFit | FitMode::ExactVolatile => f64::MAX,
        };

        let default_zoom = if texture_aspect_ratio > widget_aspect_ratio {
            (self.widget_width() / image_width).min(max_zoom_factor)
        } else {
            (self.widget_height() / image_height).min(max_zoom_factor)
        };

        let rotated_zoom = if 1. / texture_aspect_ratio > widget_aspect_ratio {
            (self.widget_width() / image_height).min(max_zoom_factor)
        } else {
            (self.widget_height() / image_width).min(max_zoom_factor)
        };

        rotated * rotated_zoom + (1. - rotated) * default_zoom
    }

    /// Sets respective output values if best-fit is active
    pub(super) fn configure_best_fit(&self) {
        // calculate new zoom value for best fit
        if self.obj().is_best_fit() {
            let best_fit_level = self.zoom_level_best_fit();
            self.zoom.set(best_fit_level);
            self.set_zoom_target(best_fit_level);
            self.zoom_animation().pause();
            self.configure_adjustments();
        }
    }

    pub(super) fn applicable_zoom(&self) -> f64 {
        self.applicable_zoom_for(self.obj().zoom())
    }

    pub(super) fn applicable_zoom_for(&self, zoom: f64) -> f64 {
        decoder::tiling::zoom_normalize(zoom) / self.scaling()
    }

    /// Maximal zoom allowed for this image
    pub(super) fn max_zoom(&self) -> f64 {
        let obj = self.obj();

        if obj.metadata().is_svg() {
            let (width, height) = self.untransformed_dimensions();
            // Avoid division by 0
            let long_side = f64::max(1., i32::max(width, height) as f64);
            // Limit to maz size supported by rsvg
            f64::min(MAX_ZOOM_LEVEL, decoder::RSVG_MAX_SIZE as f64 / long_side)
        } else {
            MAX_ZOOM_LEVEL
        }
    }

    /// Required adjustment to put image coordinate under the cursor at this
    /// zoom level
    fn adj_for_position(
        &self,
        (cur_x, cur_y): (f64, f64),
        (img_x, img_y): (f64, f64),
        zoom: f64,
    ) -> (f64, f64) {
        let zoom = self.applicable_zoom_for(zoom);

        // Transform image coordiantes to view coordinates
        let (img_x, img_y) = (img_x * zoom, img_y * zoom);

        let h_adj = f64::max(0., img_x - cur_x);
        let v_adj = f64::max(0., img_y - cur_y);

        (h_adj, v_adj)
    }

    /// Set zoom level aiming for given position or center if not available
    pub(super) fn set_zoom_aiming(&self, mut zoom: f64, cur: Option<(f64, f64)>) {
        let obj = self.obj();

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

        if zoom < self.zoom_level_best_fit() && obj.fit_mode() == FitMode::BestFit {
            let minimum = self.zoom_level_best_fit();
            let max_deviation = minimum / ZOOM_FACTOR_MAX_RUBBERBAND;
            let deviation = zoom / minimum;
            zoom = f64::max(
                minimum * deviation.powf(RUBBERBANDING_EXPONENT),
                max_deviation,
            );
        }

        if zoom == obj.zoom() {
            return;
        }

        // Point in image that should stay under the cursor
        let img_pos = if let Some(img_pos) = self.zoom_cursor_target.get() {
            // Point is stored for animation
            img_pos
        } else {
            // Use center of viewport
            obj.widget_to_img_coord((self.widget_width() / 2., self.widget_height() / 2.))
        };

        let cur_pos = if let Some(cur) = cur {
            if self.zoom_cursor_target.get().is_some() {
                cur
            } else {
                // Use center of widget for cursor since `img_pos` is locked in on center
                (self.widget_width() / 2., self.widget_height() / 2.)
            }
        } else {
            // Use center of widget since no cursor position available
            (self.widget_width() / 2., self.widget_height() / 2.)
        };

        let (h_adj, v_adj) = self.adj_for_position(cur_pos, img_pos, zoom);

        self.zoom.set(zoom);
        self.configure_adjustments();

        self.set_hadj_value(h_adj);
        self.set_vadj_value(v_adj);

        obj.notify_zoom();
        obj.queue_draw();
    }

    pub(super) fn set_zoom_target(&self, zoom_target: f64) {
        log::trace!("Setting zoom target {zoom_target}");

        self.zoom_target.set(zoom_target);
        self.obj().notify_zoom_target();

        if self.obj().zoom() == self.zoom_target.get() {
            self.request_tiles();
        }
    }

    pub(super) fn zoom_to_full(
        &self,
        mut zoom: f64,
        animated: bool,
        snap_best_fit: bool,
        force_cursor_center: bool,
    ) {
        let obj = self.obj().to_owned();

        let max_zoom = self.max_zoom();
        if zoom >= max_zoom {
            zoom = max_zoom;
            obj.set_is_max_zoom(true);
        } else {
            obj.set_is_max_zoom(false);
        }

        let extended_best_fit_threshold = if snap_best_fit {
            // If image is only 1/4 of a zoom step away from best-fit, also
            // activate best-fit. This avoids bugs with floating point precision
            // and removes awkward minimal zoom steps.
            self.zoom_level_best_fit() * (1. + (ZOOM_FACTOR_BUTTON - 1.) / 4.)
        } else {
            self.zoom_level_best_fit()
        };

        // Reset fit mode if entering in the usual zoom levels again
        if obj.fit_mode() == FitMode::ExactVolatile
            && zoom >= f64::min(1.0, self.zoom_level_best_fit())
        {
            obj.set_fit_mode(FitMode::BestFit)
        }

        if zoom <= extended_best_fit_threshold && obj.fit_mode() != FitMode::ExactVolatile {
            zoom = self.zoom_level_best_fit();
            obj.set_best_fit(true);
        } else {
            obj.set_best_fit(false);
        }

        log::trace!("Zoom to {zoom:.3}");

        self.set_zoom_target(zoom);

        // abort if already at correct zoom level
        if zoom == obj.zoom() {
            log::trace!("Already at correct zoom level");
            return;
        }

        if animated {
            let animation = self.zoom_animation();

            if force_cursor_center {
                self.zoom_cursor_target.set(None);
            } else {
                // Set new point in image that should remain under the cursor while zooming if
                // there isn't one already
                if self.zoom_cursor_target.get().is_none() {
                    let img_pos: Option<(f64, f64)> = self
                        .pointer_position
                        .get()
                        .map(|x| obj.widget_to_img_coord(x));
                    self.zoom_cursor_target.set(img_pos);
                }
            }

            animation.set_value_from(obj.zoom());
            animation.set_value_to(zoom);
            animation.play();
        } else {
            self.set_zoom(zoom);
            self.set_zoom_target(zoom);
        }
    }

    /// Animation that makes larger zoom steps (from buttons etc) look smooth
    pub(super) fn zoom_animation(&self) -> &adw::TimedAnimation {
        self.zoom_animation.get_or_init(|| {
            let obj = self.obj().to_owned();

            let animation = adw::TimedAnimation::builder()
                .duration(ZOOM_ANIMATION_DURATION)
                .widget(&obj)
                // The actual target will be set individually later
                .target(&adw::PropertyAnimationTarget::new(&obj, "zoom"))
                .build();

            animation.connect_done(glib::clone!(
                #[weak]
                obj,
                move |_| {
                    let imp = obj.imp();
                    imp.zoom_cursor_target.set(None);
                    imp.set_zoom_target(obj.imp().zoom_target.get());
                }
            ));

            animation
        })
    }

    pub fn set_fit_mode(&self, fit_mode: FitMode) {
        self.fit_mode.replace(fit_mode);
        if fit_mode != FitMode::ExactVolatile {
            self.configure_best_fit();
        }
    }
}

impl LpImage {
    /// Zoom in a step with animation
    ///
    /// Used by keyboard shortcuts
    pub fn zoom_in_cursor(&self) {
        let zoom = self.imp().zoom_target.get() * ZOOM_FACTOR_BUTTON;

        self.zoom_to(zoom);
    }

    /// Zoom in a step with animation
    ///
    /// Used by buttons
    pub fn zoom_in_center(&self) {
        let zoom = self.imp().zoom_target.get() * ZOOM_FACTOR_BUTTON;

        self.imp().zoom_to_full(zoom, true, true, true);
    }

    /// Zoom out a step with animation
    ///
    /// Used by keyboard shortcuts
    pub fn zoom_out_cursor(&self) {
        let zoom = self.imp().zoom_target.get() / ZOOM_FACTOR_BUTTON;

        self.zoom_to(zoom);
    }

    /// Zoom out a step with animation
    ///
    /// Used by buttons
    pub fn zoom_out_center(&self) {
        let zoom = self.imp().zoom_target.get() / ZOOM_FACTOR_BUTTON;

        self.imp().zoom_to_full(zoom, true, true, true);
    }

    /// Zoom to best fit
    ///
    /// Used by shortcut
    pub fn zoom_best_fit(&self) {
        self.zoom_to(self.imp().zoom_level_best_fit());
    }

    /// Zoom to specific level with animation
    pub fn zoom_to(&self, zoom: f64) {
        self.imp().zoom_to_full(zoom, true, true, false);
    }

    /// Zoom to specific level with animation not snapping to best-fit
    ///
    /// Used for zooming to 100% or 200% etc
    pub fn zoom_to_exact(&self, zoom: f64) {
        self.imp().zoom_to_full(zoom, true, false, false);
    }

    pub fn zoom_to_no_best_fit(&self, zoom: f64) {
        self.set_fit_mode(FitMode::ExactVolatile);
        self.set_best_fit(false);
        self.imp().zoom_to_full(zoom, true, false, false);
    }

    pub fn zoom_to_center_no_best_fit(&self, zoom: f64) {
        self.set_fit_mode(FitMode::ExactVolatile);
        self.set_best_fit(false);
        self.imp().zoom_to_full(zoom, true, false, true);
    }

    pub fn is_best_fit(&self) -> bool {
        self.imp().best_fit.get()
    }
}
