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

//! A widget that shows images and handles control inputs
//!
//! This widget handles showing the right section of the image
//! with a spicfic zoom level and rotation. It also handles control
//! input that change those properties.
//!
//! While this widget logically coposes the image, decoding images
//! and composing the textures happens in [`decoder`].
//!
//! [`decoder`]: crate::decoder

use crate::deps::*;

use crate::decoder::{self, tiling, Decoder, DecoderUpdate};
use crate::i18n::i18n;
use crate::image_metadata::LpImageMetadata;
use crate::util;

use adw::{prelude::*, subclass::prelude::*};
use arc_swap::ArcSwap;
use futures::prelude::*;
use gtk_macros::spawn;
use once_cell::sync::Lazy;
use once_cell::unsync::OnceCell;

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Milliseconds
const ZOOM_ANIMATION_DURATION: u32 = 200;
/// Milliseconds
const ROTATION_ANIMATION_DURATION: u32 = 200;

/// Relative to current zoom level
const ZOOM_FACTOR_BUTTON: f64 = 1.5;
/// Zoom 30% per scroll wheel detent
const ZOOM_FACTOR_SCROLL_WHEEL: f64 = 1.3;
/// Zoom 0.5% per pixel
///
/// This is for scrolling devices that might not exist
const ZOOM_FACTOR_SCROLL_SURFACE: f64 = 1.005;

/// Relative to best-fit level
const ZOOM_FACTOR_DOUBLE_TAP: f64 = 2.5;

/// Relative to best-fit and `MAX_ZOOM_LEVEL`
const ZOOM_FACTOR_MAX_RUBBERBAND: f64 = 2.;
/// Smaller values make the band feel stiffer
const RUBBERBANDING_EXPONENT: f64 = 0.4;

/// Max zoom level 2000%
const MAX_ZOOM_LEVEL: f64 = 20.0;

/// Thumbnail size in application pixels
///
/// The thumbnail is currently used for drag and drop.
const THUMBNAIL_SIZE: f32 = 128.;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct LpImage {
        pub file: RefCell<Option<gio::File>>,
        pub path: RefCell<Option<PathBuf>>,
        pub is_deleted: Cell<bool>,
        /// Track changes to this image
        pub file_monitor: RefCell<Option<gio::FileMonitor>>,
        pub tiles: Arc<ArcSwap<tiling::TilingStore>>,
        pub decoder: RefCell<Option<Arc<Decoder>>>,

        /// Set to true when image is ready for displaying
        pub is_loaded: Cell<bool>,
        /// Set if an error has occurred, shown on error_page
        pub error: RefCell<Option<String>>,

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
        /// Same but for vertical
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
        pub image_metadata: RefCell<LpImageMetadata>,

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
                    glib::ParamSpecVariant::builder("path", glib::VariantTy::BYTE_STRING)
                        .read_only()
                        .build(),
                    glib::ParamSpecBoolean::builder("is-deleted")
                        .read_only()
                        .build(),
                    glib::ParamSpecBoolean::builder("is-loaded")
                        .read_only()
                        .build(),
                    glib::ParamSpecBoolean::builder("error").read_only().build(),
                    glib::ParamSpecObject::builder::<LpImageMetadata>("metadata")
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
                    glib::ParamSpecVariant::builder("image-size", glib::VariantTy::TUPLE)
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
            let obj = self.obj();
            match pspec.name() {
                "file" => obj.file().to_value(),
                "path" => obj.path().to_variant().to_value(),
                "is-deleted" => obj.is_deleted().to_value(),
                "is-loaded" => obj.is_loaded().to_value(),
                "error" => obj.error().to_value(),
                "metadata" => obj.metadata().to_value(),
                "rotation" => obj.rotation().to_value(),
                "mirrored" => obj.mirrored().to_value(),
                "zoom" => obj.zoom().to_value(),
                "best-fit" => obj.is_best_fit().to_value(),
                "is-max-zoom" => obj.is_max_zoom().to_value(),
                "image-size" => obj.image_size().to_variant().to_value(),
                // don't use getter functions here since they can return a fake adjustment
                "hadjustment" => self.hadjustment.borrow().to_value(),
                "vadjustment" => self.vadjustment.borrow().to_value(),
                "hscroll-policy" | "vscroll-policy" => gtk::ScrollablePolicy::Minimum.to_value(),
                name => unimplemented!("property {}", name),
            }
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.obj();
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

            let obj = self.obj();
            obj.set_hexpand(true);
            obj.set_vexpand(true);
            obj.set_overflow(gtk::Overflow::Hidden);

            self.zoom.set(1.);
            self.zoom_target.set(1.);
            self.best_fit.set(true);

            self.connect_controllers();
            self.connect_gestures();
        }

        fn dispose(&self) {
            let obj = self.obj();

            log::debug!("Disposing LpImage");

            // remove target from zoom animation because it's property of this object
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
            let obj = self.obj();

            // Needed for having the current cursor position available
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
            obj.add_controller(motion_controller);

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

		// Use exponential scaling since zoom is always multiplicative with the existing value
		// This is the right thing since `exp(n/2)^2 == exp(n)` (two small steps are the same as one larger step)
                let (zoom_factor, animated) = match event.unit() {
                    gdk::ScrollUnit::Wheel => (f64::exp( - y * f64::ln(ZOOM_FACTOR_SCROLL_WHEEL)), y.abs() >= 1.),
                    gdk::ScrollUnit::Surface => (f64::exp( - y * f64::ln(ZOOM_FACTOR_SCROLL_SURFACE)), false),
                    unknown_unit => {
                        log::warn!("Ignoring unknown scroll unit: {unknown_unit}");
                        (1., false)
                    }
                };

                let zoom =
                    obj.imp().zoom_target.get() * zoom_factor;

                if zoom > obj.zoom_level_best_fit() {
                    obj.set_best_fit(false);
                    if animated {
                        obj.zoom_to(zoom);
                    } else {
                        obj.set_zoom(zoom);
                        obj.imp().zoom_target.set(zoom);
                    }
                } else {
                    obj.set_best_fit(true);
                    if animated {
                        obj.zoom_to(obj.zoom_level_best_fit());
                    } else {
                        obj.set_zoom(obj.zoom_level_best_fit());
                        obj.imp().zoom_target.set(obj.zoom_level_best_fit());
                    }
                }

                // do not propagate event to scrolled window
                gtk::Inhibit(true)
            }));

            obj.add_controller(scroll_controller);
        }

        fn connect_gestures(&self) {
            let obj = self.obj();

            // Double click for fullscreen (mouse/touchpad) or zoom (touch screen)
            let left_click_gesture = gtk::GestureClick::builder().button(1).build();
            obj.add_controller(left_click_gesture.clone());
            left_click_gesture.connect_pressed(
                glib::clone!(@weak obj => move |gesture, n_press, x, y| {
                    // only handle double clicks
                    if n_press != 2 {
                        return;
                    }

                    if gesture.device().map(|x| x.source()) == Some(gdk::InputSource::Touchscreen) {
                        // zoom
                        obj.imp().pointer_position.set(Some((x, y)));
                        if obj.is_best_fit() {
                            // zoom in
                            obj.zoom_to(ZOOM_FACTOR_DOUBLE_TAP * obj.zoom_level_best_fit());
                        } else {
                            // zoom back out
                            obj.zoom_best_fit();
                        }
                    } else {
                        // fullscreen
                        obj.activate_action("win.toggle-fullscreen", None).unwrap();
                    }
                }),
            );

            // Drag for moving image around
            let drag_gesture = gtk::GestureDrag::new();
            obj.add_controller(drag_gesture.clone());

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
            obj.add_controller(rotation_gesture.clone());

            rotation_gesture.connect_angle_changed(
                glib::clone!(@weak obj => move |gesture, _, _| {
                    let angle = gesture.angle_delta();
                    // offset for rotate gesture to take effect
                    if angle.abs().to_degrees() > 20. {
                        obj.set_rotation(obj.imp().rotation_target.get() + angle.to_degrees());
                    }
                }),
            );

            rotation_gesture.connect_end(glib::clone!(@weak obj => move |_, _| {
                log::debug!("Rotate gesture ended");

                let angle = (obj.rotation() / 90.).round() * 90. - obj.imp().rotation_target.get();
                obj.rotate_by(angle);
            }));

            // Zoom
            let zoom_gesture = gtk::GestureZoom::new();
            obj.add_controller(zoom_gesture.clone());

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
                            hadjustment.set_value(hadjustment.value() + x0 - x1);
                            vadjustment.set_value(vadjustment.value() + y0 - y1);
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
            let widget = self.obj();

            // ensure there is an actual size change
            if self.widget_dimensions.get() != (width, height) {
                self.widget_dimensions.set((width, height));

                widget.configure_best_fit();
            }

            widget.configure_adjustments();
        }

        // called when the widget content should be re-rendered
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let widget_width = widget.width() as f64;
            let widget_height = widget.height() as f64;
            let display_width = widget.image_displayed_width();
            let display_height = widget.image_displayed_height();

            // make sure the scrollbars are correct
            widget.configure_adjustments();

            let applicable_zoom = widget.applicable_zoom();

            let scaling_filter = gsk::ScalingFilter::Linear;
            /*
                            let scaling_filter = if applicable_zoom < 1. {
                                gsk::ScalingFilter::Trilinear
                            } else {
                                gsk::ScalingFilter::Nearest
                            };
            */

            let render_options = tiling::RenderOptions { scaling_filter };

            // Operations on snapshots are coordinate transformations
            // It might help to read the following code from bottom to top
            snapshot.save();

            // Apply the scrolling position to the image
            if let Some(adj) = self.hadjustment.borrow().as_ref() {
                let x = -(adj.value() - (adj.upper() - display_width) / 2.);
                snapshot.translate(&graphene::Point::new(x as f32, 0.));
            }
            if let Some(adj) = self.vadjustment.borrow().as_ref() {
                let y = -(adj.value() - (adj.upper() - display_height) / 2.);
                snapshot.translate(&graphene::Point::new(0., y as f32));
            }

            // Centering in widget when no scrolling (black bars around image)
            let x = f64::max((widget_width - display_width) / 2.0, 0.).floor();
            let y = f64::max((widget_height - display_height) / 2.0, 0.).floor();
            // Round to pixel values to not have a half pixel offset to physical pixels
            // The offset would leading to a blurry output
            snapshot.translate(&graphene::Point::new(
                widget.round(x) as f32,
                widget.round(y) as f32,
            ));

            // Apply zoom
            snapshot.scale(applicable_zoom as f32, applicable_zoom as f32);

            // Apply rotation and mirroring
            widget.snapshot_rotate_mirror(snapshot, widget.rotation() as f32, widget.mirrored());

            // Add texture(s)
            self.tiles
                .load()
                .add_to_snapshot(snapshot, applicable_zoom, &render_options);

            snapshot.restore();
        }

        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            let (image_width, image_height) = self.obj().image_size();

            if image_width > 0 && image_height > 0 {
                if let Some(display) = gdk::Display::default() {
                    if let Some(native) = self.obj().native() {
                        let hidpi_scale = self.obj().scale_factor() as f64;

                        // get monitor information
                        // TODO: Get rid of unwrap
                        let monitor = display.monitor_at_surface(&native.surface()).unwrap();
                        let monitor_geometry = monitor.geometry();
                        // TODO: Per documentation those dimensions should not be physical pixels.
                        // But on Wayland they are physical pixels and on X11 not.
                        // Taking the version that works on Wayland for now.
                        // <https://gitlab.gnome.org/GNOME/gtk/-/issues/5391>
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

                        // factor for width and height that will achieve the desired area occupation
                        // derived from:
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

            // fallback if monitor size or image size is not known:
            // use original image size and hope for the best
            // TODO: We could use a small default monitor size estimate instead
            let size = match orientation {
                gtk::Orientation::Horizontal => image_width,
                gtk::Orientation::Vertical => image_height,
                _ => unreachable!(),
            };

            (0, size, -1, -1)
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
    pub async fn load(&self, path: &Path) {
        let path = path.to_path_buf();
        let file = gio::File::for_path(&path);
        self.set_file(&file);

        if !path.is_file() {
            self.set_error(anyhow::Error::msg(i18n("File does not exist")));
            return;
        }

        log::debug!("Loading file {path:?}");

        let tiles = &self.imp().tiles;
        // TODO: Fix two unwraps
        let (decoder, mut decoder_update) = util::spawn(
            "image-init-decoder",
            glib::clone!(@strong file, @strong tiles => move || {
                Decoder::new(file.clone(), tiles.clone())
            }),
        )
        .await
        .unwrap()
        .unwrap();

        let weak_obj = self.downgrade();
        spawn!(async move {
            while let Some(update) = decoder_update.next().await {
                if let Some(obj) = weak_obj.upgrade() {
                    obj.update(update);
                }
            }
            log::debug!("Stopped listening to decoder since sender is gone");
        });

        let decoder = Arc::new(decoder);
        self.imp().decoder.replace(Some(decoder));
    }

    /// Called when decoder sends update
    pub fn update(&self, update: DecoderUpdate) {
        let imp = self.imp();

        match update {
            DecoderUpdate::Metadata(metadata) => {
                log::debug!("Received metadata");
                imp.image_metadata.replace(LpImageMetadata::from(metadata));
                self.notify("metadata");

                self.reset_rotation();
            }
            DecoderUpdate::Dimensions => {
                log::debug!("Received dimensions: {:?}", self.original_dimensions());
                self.notify("image-size");
                self.configure_best_fit();
                self.request_tiles();
            }
            DecoderUpdate::Redraw => {
                if !self.is_loaded() {
                    self.imp().is_loaded.set(true);
                    self.notify("is-loaded");
                }

                self.queue_draw();
                imp.tiles.rcu(|tiles| {
                    let mut new_tiles = (**tiles).clone();
                    // TODO: Use an area larger than the viewport
                    new_tiles.cleanup(self.imp().zoom_target.get(), self.viewport());
                    new_tiles
                });
            }
            DecoderUpdate::Error(err) => {
                self.set_error(err);
            }
            DecoderUpdate::Format(_) => {
                // TODO: Store and use in image properties
            }
        }
    }

    pub fn is_loaded(&self) -> bool {
        self.imp().is_loaded.get()
    }

    pub fn is_deleted(&self) -> bool {
        self.imp().is_deleted.get()
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
        let rotated = rotation.to_radians().sin().abs();
        let (image_width, image_height) = self.original_dimensions();
        let texture_aspect_ratio = image_width as f64 / image_height as f64;
        let widget_aspect_ratio = self.width() as f64 / self.height() as f64;

        let default_zoom = if texture_aspect_ratio > widget_aspect_ratio {
            (self.width() as f64 / image_width as f64).min(1.)
        } else {
            (self.height() as f64 / image_height as f64).min(1.)
        };

        let rotated_zoom = if 1. / texture_aspect_ratio > widget_aspect_ratio {
            (self.width() as f64 / image_height as f64).min(1.)
        } else {
            (self.height() as f64 / image_width as f64).min(1.)
        };

        rotated * rotated_zoom + (1. - rotated) * default_zoom
    }

    /// Sets respective output values if best-fit is active
    fn configure_best_fit(&self) {
        // calculate new zoom value for best fit
        if self.is_best_fit() {
            let best_fit_level = self.zoom_level_best_fit();
            self.imp().zoom.set(best_fit_level);
            self.set_zoom_target(best_fit_level);
            self.zoom_animation().pause();
        }
    }

    pub fn file(&self) -> Option<gio::File> {
        self.imp().file.borrow().clone()
    }

    pub fn path(&self) -> Option<PathBuf> {
        self.imp().path.borrow().clone()
    }

    pub(super) fn set_file(&self, file: &gio::File) {
        let imp = self.imp();

        imp.file.replace(Some(file.clone()));
        imp.path.replace(file.path());
        self.notify("path");

        let monitor = file.monitor_file(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE);
        if let Ok(m) = &monitor {
            m.connect_changed(
                glib::clone!(@weak self as obj => move |_, file_a, file_b, event| {
                    obj.file_changed(event, file_a, file_b);
                }),
            );
        }

        imp.file_monitor.replace(monitor.ok());
    }

    /// File changed on drive
    fn file_changed(
        &self,
        event: gio::FileMonitorEvent,
        file_a: &gio::File,
        file_b: Option<&gio::File>,
    ) {
        match event {
            gio::FileMonitorEvent::Renamed => {
                if let Some(file) = file_b {
                    log::debug!("Moved to {:?}", file.path());
                    self.set_file(file);
                }
            }
            gio::FileMonitorEvent::ChangesDoneHint => {
                let obj = self.clone();
                let file = file_a.clone();
                // TODO: error handling is missing
                spawn! {async move { obj.load(&file.path().unwrap()).await; }};
            }
            gio::FileMonitorEvent::Deleted
            | gio::FileMonitorEvent::MovedOut
            | gio::FileMonitorEvent::Unmounted => {
                self.imp().is_deleted.set(true);
                self.notify("is-deleted");
            }
            _ => {}
        }
    }

    /// Returns a thumbnail of the displated image
    pub fn thumbnail(&self) -> Option<gdk::Paintable> {
        let (width, height) = self.original_dimensions();
        let long_side = i32::max(width, height);
        let orientation = self.metadata().orientation();

        let scale = f32::min(1., THUMBNAIL_SIZE / long_side as f32);
        let render_options = tiling::RenderOptions {
            scaling_filter: gsk::ScalingFilter::Trilinear,
        };

        let snapshot = gtk::Snapshot::new();

        snapshot.scale(scale, scale);
        self.snapshot_rotate_mirror(
            &snapshot,
            -orientation.rotation as f32,
            orientation.mirrored,
        );

        self.imp()
            .tiles
            .load()
            .add_to_snapshot(&snapshot, scale as f64, &render_options);

        snapshot.to_paintable(None)
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

    /// Set rotation and mirroring to the state would have after loading
    pub fn reset_rotation(&self) {
        let orientation = self.metadata().orientation();
        self.imp().rotation_target.set(-orientation.rotation);
        self.set_mirrored(orientation.mirrored);
        self.set_rotation(-orientation.rotation);
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

    fn applicable_zoom(&self) -> f64 {
        decoder::tiling::zoom_normalize(self.zoom())
    }

    /// Set zoom level aiming for given position or center if not available
    fn set_zoom_aiming(&self, mut zoom: f64, aiming: Option<(f64, f64)>) {
        // allow some deviation from max value for rubberbanding
        if zoom > MAX_ZOOM_LEVEL {
            let max_deviation = MAX_ZOOM_LEVEL * ZOOM_FACTOR_MAX_RUBBERBAND;
            let deviation = zoom / MAX_ZOOM_LEVEL;
            zoom = f64::min(
                MAX_ZOOM_LEVEL * deviation.powf(RUBBERBANDING_EXPONENT),
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

    fn set_zoom_target(&self, zoom_target: f64) {
        log::debug!("Setting zoom target {zoom_target}");

        self.imp().zoom_target.set(zoom_target);

        if self.zoom() == self.imp().zoom_target.get() {
            self.request_tiles();
        }
    }

    fn request_tiles(&self) {
        if let Some(decoder) = self.imp().decoder.borrow().as_ref() {
            if self.zoom_animation().state() != adw::AnimationState::Playing {
                decoder.request(crate::decoder::TileRequest {
                    viewport: self.viewport(),
                    zoom: self.imp().zoom_target.get(),
                });
            }
        }
    }

    fn viewport(&self) -> graphene::Rect {
        let x = self.hadjustment().value();
        let y = self.vadjustment().value();
        let width = self.widget_width();
        let height = self.widget_height();

        graphene::Rect::new((x) as f32, (y) as f32, (width) as f32, (height) as f32)
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
        // and removes awkward minimal zoom steps.
        let extended_best_fit_threshold =
            self.zoom_level_best_fit() * (1. + (ZOOM_FACTOR_BUTTON - 1.) / 4.);

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
    }

    /// Image size of original image with EXIF rotation applied
    pub fn image_size(&self) -> (i32, i32) {
        let orientation = self.imp().image_metadata.borrow().orientation();
        if orientation.rotation.abs() == 90. || orientation.rotation.abs() == 270. {
            let (x, y) = self.original_dimensions();
            (y, x)
        } else {
            self.original_dimensions()
        }
    }

    fn original_dimensions(&self) -> (i32, i32) {
        if let Some((width, height)) = self.imp().tiles.load().original_dimensions {
            (width as i32, height as i32)
        } else {
            (0, 0)
        }
    }

    /// Image width with current zoom factor and rotation
    ///
    /// During rotation it is an interpolated size that does not
    /// represent the actual size. The size returned might well be
    /// larger than what can actually be displayed within the widget.
    pub fn image_displayed_width(&self) -> f64 {
        let (width, height) = self.original_dimensions();

        let rotated = self.rotation().to_radians().sin().abs();

        ((1. - rotated) * width as f64 + rotated * height as f64) * self.applicable_zoom()
    }

    pub fn image_displayed_height(&self) -> f64 {
        let (width, height) = self.original_dimensions();

        let rotated = self.rotation().to_radians().sin().abs();

        ((1. - rotated) * height as f64 + rotated * width as f64) * self.applicable_zoom()
    }

    /// Stepwise scrolls inside an image when zoomed in
    pub fn pan(&self, direction: &gtk::PanDirection) {
        let sign = match direction {
            gtk::PanDirection::Left | gtk::PanDirection::Up => -1.,
            gtk::PanDirection::Right | gtk::PanDirection::Down => 1.,
            _ => {
                log::error!("Unknown pan direction {direction:?}");
                return;
            }
        };

        let (adjustment, max) = match direction {
            gtk::PanDirection::Left | gtk::PanDirection::Right => {
                (self.hadjustment(), self.max_hadjustment_value())
            }
            gtk::PanDirection::Up | gtk::PanDirection::Down => {
                (self.vadjustment(), self.max_vadjustment_value())
            }
            _ => {
                log::error!("Unknown pan direction {direction:?}");
                return;
            }
        };

        let value = (adjustment.value() + sign * adjustment.step_increment()).clamp(0., max);

        adjustment.set_value(value);
    }

    fn hadjustment(&self) -> gtk::Adjustment {
        if let Some(adj) = self.imp().hadjustment.borrow().as_ref() {
            adj.clone()
        } else {
            log::trace!("Hadjustment not set yet: Using fake object");
            gtk::Adjustment::default()
        }
    }

    fn set_hadjustment(&self, adjustment: Option<gtk::Adjustment>) {
        if let Some(adj) = &adjustment {
            adj.connect_value_changed(glib::clone!(@weak self as obj => move |_| {
                obj.request_tiles();
                obj.queue_draw();
            }));
        }

        self.imp().hadjustment.replace(adjustment);
        self.configure_adjustments();
    }

    fn vadjustment(&self) -> gtk::Adjustment {
        if let Some(adj) = self.imp().vadjustment.borrow().as_ref() {
            adj.clone()
        } else {
            log::trace!("Vadjustment not set yet: Using fake object");
            gtk::Adjustment::default()
        }
    }

    fn set_vadjustment(&self, adjustment: Option<gtk::Adjustment>) {
        if let Some(adj) = &adjustment {
            adj.connect_value_changed(glib::clone!(@weak self as obj => move |_| {
                obj.request_tiles();
                obj.queue_draw();
            }));
        }

        self.imp().vadjustment.replace(adjustment);
        self.configure_adjustments();
    }

    /// Configure scrollbars for current situation
    fn configure_adjustments(&self) {
        let hadjustment = self.hadjustment();
        // round to application pixels to avoid tiny rounding errors from zoom
        let content_width = self.round(self.image_displayed_width());
        let widget_width = self.widget_width();

        hadjustment.configure(
            // value
            hadjustment.value().clamp(0., self.max_hadjustment_value()),
            // lower
            0.,
            // upper
            content_width,
            // arrow button and shortcut step
            widget_width * 0.1,
            // page up/down step
            widget_width * 0.9,
            // page size
            f64::min(widget_width, content_width),
        );

        let vadjustment = self.vadjustment();
        // round to application pixels to avoid tiny rounding errors from zoom
        let content_height = self.round(self.image_displayed_height());
        let widget_height = self.widget_height();

        vadjustment.configure(
            vadjustment.value().clamp(0., self.max_vadjustment_value()),
            // lower
            0.,
            // upper
            content_height,
            // arrow button and shortcut step
            widget_height * 0.1,
            // page up/down step
            widget_height * 0.9,
            // page_size
            f64::min(widget_height, content_height),
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

    /// Mirrors and rotates snapshot according to arguments
    ///
    /// After the operation the image is positioned such that it's origin
    /// is a `(0, 0)` again.
    pub fn snapshot_rotate_mirror(&self, snapshot: &gtk::Snapshot, rotation: f32, mirrored: bool) {
        let applicable_zoom = self.applicable_zoom();
        let (original_width, original_height) = self.original_dimensions();
        let display_width = self.image_displayed_width();
        let display_height = self.image_displayed_height();

        // Put image origin at (0, 0) again with rotation
        snapshot.translate(&graphene::Point::new(
            -(original_width as f32 - display_width as f32 / applicable_zoom as f32) / 2.,
            -(original_height as f32 - display_height as f32 / applicable_zoom as f32) / 2.,
        ));

        // Undo centering in coordinates
        snapshot.translate(&graphene::Point::new(
            original_width as f32 / 2.,
            original_height as f32 / 2.,
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
            -original_width as f32 / 2.,
            -original_height as f32 / 2.,
        ));
    }

    pub fn metadata(&self) -> LpImageMetadata {
        self.imp().image_metadata.borrow().clone()
    }

    /// Drag and drop content provider
    pub fn content_provider(&self) -> Option<gdk::ContentProvider> {
        let file = self.file()?;
        let list = gdk::FileList::from_array(&[file]);
        Some(gdk::ContentProvider::for_value(&list.to_value()))
    }

    /// Returns decoding error if one occured
    pub fn error(&self) -> Option<String> {
        self.imp().error.borrow().clone()
    }

    fn set_error(&self, err: anyhow::Error) {
        log::debug!("Decoding error: {err:?}");
        self.imp().error.replace(Some(err.to_string()));
        self.notify("error");
    }

    /// Returns scaling aware rounded application pixel
    ///
    /// One physical pixel is 0.5 application pixels
    pub fn round(&self, number: f64) -> f64 {
        let scale = self.scale_factor() as f64;
        (number * scale).round() / scale
    }
}
