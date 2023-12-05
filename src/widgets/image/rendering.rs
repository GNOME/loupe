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

impl WidgetImpl for imp::LpImage {
    // called when the widget size might have changed
    fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
        let obj = self.obj();

        let (scale_changed, scale_change) = if obj.scale_factor() != self.scale_factor.get() {
            let scale_change = obj.scale_factor() as f64 / self.scale_factor.get() as f64;
            self.scale_factor.set(obj.scale_factor());
            (true, scale_change)
        } else {
            (false, 1.)
        };

        if obj.is_best_fit() {
            // ensure there is an actual size change
            if self.widget_dimensions.get() != (width, height) || scale_changed {
                obj.configure_best_fit();
            }
        } else if scale_changed {
            // Show same area of the image when scale changes
            let new_zoom = self.zoom_target.get() * scale_change;

            obj.zoom_animation().pause();
            self.zoom.set(new_zoom);
            self.zoom_target.set(new_zoom);
        }

        self.widget_dimensions.set((width, height));
        obj.configure_adjustments();

        // Avoid updates for first size_allocate with zoom not set yet
        if obj.is_loaded() {
            // Get potentially missing tiles for enlarged viewing area

            obj.request_tiles();
        }
    }

    // called when the widget content should be re-rendered
    fn snapshot(&self, snapshot: &gtk::Snapshot) {
        let obj = self.obj();
        let widget_width = obj.width() as f64;
        let widget_height = obj.height() as f64;
        let display_width = obj.image_displayed_width();
        let display_height = obj.image_displayed_height();

        let applicable_zoom = obj.applicable_zoom();

        let scaling_filter = if obj.metadata().format().map_or(false, |x| x.is_svg()) {
            // Looks better in SVG animations and avoids rendering issues
            gsk::ScalingFilter::Linear
        } else if applicable_zoom < 1. {
            // Uses mipmaps to avoid moiré patterns
            gsk::ScalingFilter::Trilinear
        } else {
            // Show pixels when zooming in because making images blurry looks worse
            gsk::ScalingFilter::Nearest
        };

        let render_options = tiling::RenderOptions {
            scaling_filter,
            scale_factor: obj.scale_factor(),
            background_color: Some(obj.background_color()),
        };

        // Operations on snapshots are coordinate transformations
        // It might help to read the following code from bottom to top
        snapshot.save();

        // Add background
        snapshot.append_color(
            &obj.background_color(),
            &graphene::Rect::new(0., 0., widget_width as f32, widget_height as f32),
        );

        // Apply the scrolling position to the image
        let hadj: gtk::Adjustment = obj.hadjustment();
        let x = -(hadj.value() - (hadj.upper() - display_width) / 2.);
        snapshot.translate(&graphene::Point::new(obj.round_f64(x) as f32, 0.));

        let vadj = obj.vadjustment();
        let y = -(vadj.value() - (vadj.upper() - display_height) / 2.);
        snapshot.translate(&graphene::Point::new(0., obj.round_f64(y) as f32));

        // Centering in widget when no scrolling (black bars around image)
        let x = obj.round_f64(f64::max((widget_width - display_width) / 2.0, 0.));
        let y = obj.round_f64(f64::max((widget_height - display_height) / 2.0, 0.));
        // Round to pixel values to not have a half pixel offset to physical pixels
        // The offset would leading to a blurry output
        snapshot.translate(&graphene::Point::new(
            obj.round_f64(x) as f32,
            obj.round_f64(y) as f32,
        ));

        // Apply rotation and mirroring
        obj.snapshot_rotate_mirror(
            snapshot,
            obj.rotation() as f32,
            obj.mirrored(),
            applicable_zoom,
        );

        // Add texture(s)
        self.frame_buffer
            .load()
            .add_to_snapshot(snapshot, applicable_zoom, &render_options);

        snapshot.restore();
    }

    fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
        let (image_width, image_height) = self.obj().image_size();

        if image_width > 0 && image_height > 0 {
            if let Some(display) = gdk::Display::default() {
                if let Some(native) = self.obj().native() {
                    if let Some(monitor) = display.monitor_at_surface(&native.surface()) {
                        let hidpi_scale = self.obj().scale_factor() as f64;

                        let monitor_geometry = monitor.geometry();
                        // TODO: Per documentation those dimensions should not be physical
                        // pixels. But on Wayland they are physical
                        // pixels and on X11 not. Taking the version
                        // that works on Wayland for now. <https://gitlab.gnome.org/GNOME/gtk/-/issues/5391>
                        let monitor_width = monitor_geometry.width() as f64 - 40.;
                        let monitor_height = monitor_geometry.height() as f64 - 60.;

                        // areas
                        let monitor_area = monitor_width * monitor_height;
                        let image_area = image_width as f64 * image_height as f64;

                        let occupy_area_factor = if monitor_area < 1024. * 768. {
                            // for small monitors occupy 80% of the area
                            0.8
                        } else {
                            // for large monitors occupy 30% of the area
                            0.3
                        };

                        // factor for width and height that will achieve the desired area
                        // occupation derived from:
                        // monitor_area * occupy_area_factor ==
                        //   (image_width * size_scale) * (image_height * size_scale)
                        let size_scale = f64::sqrt(monitor_area / image_area * occupy_area_factor);
                        // ensure that we never increase image size
                        let target_scale = f64::min(1.0, size_scale);
                        let mut nat_width = image_width as f64 * target_scale;
                        let mut nat_height = image_height as f64 * target_scale;

                        // scale down if targeted occupation does not fit in one direction
                        if nat_width > monitor_width {
                            nat_width = monitor_width;
                            nat_height = nat_height * monitor_width / nat_width;
                        }

                        // same for other direction
                        if nat_height > monitor_height {
                            nat_height = monitor_height;
                            nat_width = nat_width * monitor_height / nat_height;
                        }

                        let size = match orientation {
                            gtk::Orientation::Horizontal => (nat_width / hidpi_scale).round(),
                            gtk::Orientation::Vertical => (nat_height / hidpi_scale).round(),
                            _ => unreachable!(),
                        };

                        return (0, size as i32, -1, -1);
                    }
                }
            }
        }

        // fallback if monitor size or image size is not known:
        // use original image size and hope for the best
        let size = match orientation {
            gtk::Orientation::Horizontal => image_width,
            gtk::Orientation::Vertical => image_height,
            _ => unreachable!(),
        };

        log::warn!("Not enough information available to calculate fitting window size");

        (0, size, -1, -1)
    }
}

impl LpImage {
    /// Mirrors and rotates snapshot according to arguments
    ///
    /// After the operation the image is positioned such that it's origin
    /// is a `(0, 0)` again.
    pub(super) fn snapshot_rotate_mirror(
        &self,
        snapshot: &gtk::Snapshot,
        rotation: f32,
        mirrored: bool,
        zoom: f64,
    ) {
        if rotation == 0. && !mirrored {
            return;
        }

        let (original_width, original_height) = self.original_dimensions();
        let width = self.image_width(zoom) as f32;
        let height = self.image_height(zoom) as f32;

        // Put image origin at (0, 0) again with rotation
        snapshot.translate(&graphene::Point::new(
            self.round_f32(width / 2.),
            self.round_f32(height / 2.),
        ));

        // Apply the transformations from properties
        snapshot.rotate(rotation);
        if mirrored {
            snapshot.scale(-1., 1.);
        }

        // Center image in coordinates.
        // Needed for rotating around the center of the image, and
        // mirroring the image does not put it to a completely different position.
        snapshot.translate(&graphene::Point::new(
            -self.round_f32(original_width as f32 * zoom as f32 / 2.),
            -self.round_f32(original_height as f32 * zoom as f32 / 2.),
        ));
    }

    /// Returns the area of the image that is visible in physical pixels
    pub(super) fn viewport(&self) -> graphene::Rect {
        let scale_factor = self.scale_factor() as f32;
        let x = self.hadjustment().value() as f32 * scale_factor;
        let y = self.vadjustment().value() as f32 * scale_factor;
        let width = self.width() as f32 * scale_factor;
        let height = self.height() as f32 * scale_factor;

        graphene::Rect::new(x, y, width, height)
    }

    pub fn widget_height(&self) -> f64 {
        self.height() as f64
    }

    pub fn widget_width(&self) -> f64 {
        self.width() as f64
    }

    /// Returns scaling aware rounded application pixel
    ///
    /// One physical pixel is 0.5 application pixels
    pub fn round_f64(&self, number: f64) -> f64 {
        // Do not round during animation to avoid wiggling around
        if self.zoom_animation().state() == adw::AnimationState::Playing {
            return number;
        }

        let scale = self.scale_factor() as f64;
        (number * scale).round() / scale
    }

    pub fn round_f32(&self, number: f32) -> f32 {
        if self.zoom_animation().state() == adw::AnimationState::Playing {
            return number;
        }

        let scale = self.scale_factor() as f32;
        (number * scale).round() / scale
    }
}
