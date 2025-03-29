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

use tiling::FrameBuffer;

use super::*;

impl WidgetImpl for imp::LpImage {
    // called when the widget size might have changed
    fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
        let obj = self.obj();

        let (scale_changed, scale_change) = if self.scaling() != self.scaling.get() {
            let scale_change = self.scaling() / self.scaling.get();
            self.scaling.set(self.scaling());
            (true, scale_change)
        } else {
            (false, 1.)
        };

        if obj.is_best_fit() {
            // ensure there is an actual size change
            if self.widget_dimensions.get() != (width, height) || scale_changed {
                self.configure_best_fit();
            }
        } else if scale_changed {
            // Show same area of the image when scale changes
            let new_zoom = self.zoom_target.get() * scale_change;

            self.zoom_animation().pause();
            self.zoom.set(new_zoom);
            self.zoom_target.set(new_zoom);
        }

        self.widget_dimensions.set((width, height));
        self.configure_adjustments();

        // Avoid updates for first size_allocate with zoom not set yet
        if obj.is_loaded() {
            // Get potentially missing tiles for enlarged viewing area

            self.request_tiles();
        }
    }

    // called when the widget content should be re-rendered
    fn snapshot(&self, snapshot: &gtk::Snapshot) {
        let obj = self.obj();
        let widget_width = self.widget_width();
        let widget_height = self.widget_height();
        let display_width = self.image_displayed_width();
        let display_height = self.image_displayed_height();

        let applicable_zoom = self.applicable_zoom();

        let nth_snapshot = self.nth_snapshot.get() + 1;
        if nth_snapshot < 3 {
            self.nth_snapshot.set(nth_snapshot);
            log::trace!("Creating snapshot #{nth_snapshot}");
        }

        let scaling_filter = if obj.metadata().is_svg() {
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
            scaling: self.scaling(),
            background_color: Some(self.background_color()),
        };

        // Operations on snapshots are coordinate transformations
        // It might help to read the following code from bottom to top
        snapshot.save();

        // Add background
        snapshot.append_color(
            &self.background_color(),
            &graphene::Rect::new(0., 0., widget_width as f32, widget_height as f32),
        );

        // Apply the scrolling position to the image
        let hadj: gtk::Adjustment = obj.hadj();
        let x = -(hadj.value() - (hadj.upper() - display_width) / 2.);
        snapshot.translate(&graphene::Point::new(self.round_f64(x) as f32, 0.));

        let vadj = obj.vadj();
        let y = -(vadj.value() - (vadj.upper() - display_height) / 2.);
        snapshot.translate(&graphene::Point::new(0., self.round_f64(y) as f32));

        // Centering in widget when no scrolling (black bars around image)
        let x = obj.image_rendering_x();
        let y = obj.image_rendering_y();
        snapshot.translate(&graphene::Point::new(x as f32, y as f32));

        // Add texture(s)

        // Don't use current frame buffer if it empty during an image reload
        let frame_buffer = self.active_frame_buffer();

        if self.operations.borrow().is_some() {
            let tmp_snapshot = gtk::Snapshot::new();
            frame_buffer.add_to_snapshot(&tmp_snapshot, applicable_zoom, &render_options);
            if let Some(node) = tmp_snapshot.to_node() {
                if let Err(err) = self.apply_operations(node, snapshot) {
                    log::error!("Failed to apply operations to node: {err}");
                }
            } else {
                log::error!("Render node is empty");
            }
        } else {
            // Apply rotation and mirroring
            self.snapshot_rotate_mirror(
                snapshot,
                obj.rotation() as f32,
                obj.mirrored(),
                applicable_zoom,
            );

            frame_buffer.add_to_snapshot(snapshot, applicable_zoom, &render_options);
        }

        snapshot.restore();

        if nth_snapshot < 3 {
            log::trace!("Snapshot #{nth_snapshot} created");
        }
    }

    fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
        let obj = self.obj();
        let (image_width_i32, image_height_i32) = obj.image_size();
        let image_width = image_width_i32 as f64;
        let image_height = image_height_i32 as f64;

        if image_width_i32 > 0 && image_height_i32 > 0 {
            if let Some((monitor_width, monitor_height)) = self.monitor_size() {
                let hidpi_scale = self.scaling();
                log::trace!("Physical monitor dimensions: {monitor_width} x {monitor_height}");

                // areas
                let monitor_area = monitor_width * monitor_height;
                let logical_monitor_area = monitor_area * hidpi_scale.powi(2);
                let image_area = image_width * image_height;

                let occupy_area_factor = if logical_monitor_area <= SMALL_SCREEN_AREA {
                    log::trace!("Small monitor detected: Using {SMALL_OCCUPY_SCREEN} screen area");
                    SMALL_OCCUPY_SCREEN
                } else {
                    log::trace!("Sufficiently large monitor detected: Using {DEFAULT_OCCUPY_SCREEN} screen area");
                    DEFAULT_OCCUPY_SCREEN
                };

                // factor for width and height that will achieve the desired area
                // occupation derived from:
                // monitor_area * occupy_area_factor ==
                //   (image_width * size_scale) * (image_height * size_scale)
                let size_scale = f64::sqrt(monitor_area / image_area * occupy_area_factor);
                // ensure that we never increase image size
                let target_scale = f64::min(1.0, size_scale);
                let mut nat_width = image_width * target_scale;
                let mut nat_height = image_height * target_scale;

                // Scale down if targeted occupation does not fit horizontally
                // Add some margin to not touch corners
                let max_width = monitor_width - 20.;
                if nat_width > max_width {
                    nat_width = max_width;
                    nat_height = image_height * nat_width / image_width;
                }

                // Same for vertical size
                // Additionally substract some space for HeaderBar and Shell bar
                let max_height = monitor_height - (50. + 35. + 20.) * hidpi_scale;
                if nat_height > max_height {
                    nat_height = max_height;
                    nat_width = image_width * nat_height / image_height;
                }

                let size = match orientation {
                    gtk::Orientation::Horizontal => (nat_width / hidpi_scale).round(),
                    gtk::Orientation::Vertical => (nat_height / hidpi_scale).round(),
                    _ => unreachable!(),
                };

                return (0, size as i32, -1, -1);
            }
        }

        // fallback if monitor size or image size is not known:
        // use original image size and hope for the best
        let size = match orientation {
            gtk::Orientation::Horizontal => image_width_i32,
            gtk::Orientation::Vertical => image_height_i32,
            _ => unreachable!(),
        };

        log::warn!("Not enough information available to calculate fitting window size");

        (0, size, -1, -1)
    }
}

impl imp::LpImage {
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

        let (untransformed_width, untransformed_height) = self.untransformed_dimensions();
        let width = self.image_width(zoom) as f32;
        let height = self.image_height(zoom) as f32;

        // Put image origin at (0, 0) again with rotation
        snapshot.translate(&graphene::Point::new(
            self.round_f32(width / 2.),
            self.round_f32(height / 2.),
        ));

        // Apply the transformations from properties
        snapshot.rotate(-rotation);
        if mirrored {
            snapshot.scale(-1., 1.);
        }

        // Center image in coordinates.
        // Needed for rotating around the center of the image, and
        // mirroring the image does not put it to a completely different position.
        snapshot.translate(&graphene::Point::new(
            -self.round_f32(untransformed_width as f32 * zoom as f32 / 2.),
            -self.round_f32(untransformed_height as f32 * zoom as f32 / 2.),
        ));
    }

    /// Returns the area of the image that is visible in physical pixels
    pub(super) fn viewport(&self) -> graphene::Rect {
        let obj = self.obj();

        let scaling = self.scaling() as f32;
        let x = obj.hadj().value() as f32 * scaling;
        let y = obj.vadj().value() as f32 * scaling;
        let width = self.widget_width() as f32 * scaling;
        let height = self.widget_height() as f32 * scaling;

        graphene::Rect::new(x, y, width, height)
    }

    pub(super) fn active_frame_buffer(&self) -> Arc<FrameBuffer> {
        let frame_buffer = self.frame_buffer.load_full();

        if frame_buffer.is_empty() {
            // Fallback to previous buffer to show previous image until new one is available
            self.previous_frame_buffer.load_full()
        } else {
            frame_buffer
        }
    }

    pub fn widget_width(&self) -> f64 {
        self.obj().width() as f64
    }

    pub fn widget_height(&self) -> f64 {
        self.obj().height() as f64
    }

    /// Returns scaling aware rounded application pixel
    ///
    /// One physical pixel is 0.5 application pixels
    pub fn round_f64(&self, number: f64) -> f64 {
        // Do not round during animation to avoid wiggling around
        if self.zoom_animation().state() == adw::AnimationState::Playing {
            return number;
        }

        let scale = self.scaling();
        (number * scale).round() / scale
    }

    pub fn round_f32(&self, number: f32) -> f32 {
        if self.zoom_animation().state() == adw::AnimationState::Playing {
            return number;
        }

        let scale = self.scaling() as f32;
        (number * scale).round() / scale
    }

    pub fn scaling(&self) -> f64 {
        let obj = self.obj();

        obj.native()
            .and_then(|x| x.surface())
            .map(|x| x.scale())
            .unwrap_or_else(|| obj.scale_factor() as f64)
    }

    /// Monitor size in physical pixels
    pub fn monitor_size(&self) -> Option<(f64, f64)> {
        let obj = self.obj();

        if let Some(display) = gdk::Display::default() {
            if let Some(surface) = obj.native().and_then(|x| x.surface()) {
                if let Some(monitor) = display.monitor_at_surface(&surface) {
                    let hidpi_scale = self.scaling();
                    let monitor_geometry = monitor.geometry();

                    return Some((
                        monitor_geometry.width() as f64 * hidpi_scale,
                        monitor_geometry.height() as f64 * hidpi_scale,
                    ));
                }
            }
        }

        None
    }
}

impl LpImage {
    pub fn image_rendering_x(&self) -> f64 {
        let imp = self.imp();
        let widget_width = imp.widget_width();
        let display_width = imp.image_displayed_width();

        // Centering in widget when no scrolling (black bars around image)
        //
        // Round to pixel values to not have a half pixel offset to physical pixels
        // The offset would leading to a blurry output
        imp.round_f64(f64::max((widget_width - display_width) / 2.0, 0.))
    }

    pub fn image_rendering_y(&self) -> f64 {
        let imp = self.imp();
        let widget_height = imp.widget_height();
        let display_height = imp.image_displayed_height();
        imp.round_f64(f64::max((widget_height - display_height) / 2.0, 0.))
    }

    pub fn image_rendering_width(&self) -> f64 {
        let imp = self.imp();
        imp.round_f64(imp.image_displayed_width())
    }

    pub fn image_rendering_height(&self) -> f64 {
        let imp = self.imp();
        imp.round_f64(imp.image_displayed_height())
    }

    /// Convert widget coordinates to image coordinates
    pub fn widget_to_img_coord(&self, (cur_x, cur_y): (f64, f64)) -> (f64, f64) {
        let imp = self.imp();
        let zoom = imp.applicable_zoom();
        let x = (cur_x - self.image_rendering_x() + imp.hadj_value()) / zoom;
        let y = (cur_y - self.image_rendering_y() + imp.vadj_value()) / zoom;

        (x, y)
    }

    /// Convert image coordinates to drawn image coordinates
    pub fn img_to_draw_coord(&self, (cur_x, cur_y): (f64, f64)) -> (f64, f64) {
        let imp = self.imp();
        let zoom = imp.applicable_zoom();

        let x = cur_x * zoom;
        let y = cur_y * zoom;

        (x, y)
    }
}
