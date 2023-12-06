// Copyright (c) 2021 Christopher Davis
// Copyright (c) 2022-2023 Sophie Herold
// Copyright (c) 2023 Julian Hofer
// Copyright (c) 2023 Lubosz Sarnecki
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

mod drag;
mod guess_background_color;
mod input_handling;
mod loading;
mod metadata;
mod pan;
mod printing;
mod rendering;
mod rotation;
mod scrollable;
mod zoom;

use std::cell::{Cell, OnceCell, RefCell};
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use arc_swap::ArcSwap;
use futures::prelude::*;
use glib::subclass::Signal;
use glib::{Properties, SignalGroup};
use once_cell::sync::Lazy;

use crate::decoder::tiling::FrameBufferExt;
use crate::decoder::{self, tiling, Decoder, DecoderUpdate};
use crate::deps::*;
use crate::metadata::Metadata;
use crate::util::Gesture;

/// Default background color around images and behind transparent images
/// `#242424`
static BACKGROUND_COLOR_DEFAULT: Lazy<gdk::RGBA> =
    Lazy::new(|| gdk::RGBA::new(36. / 255., 36. / 255., 36. / 255., 1.));
/// Background color if the default does not give enough contrast for
/// transparent images `#e8e7e6`
static BACKGROUND_COLOR_ALTERNATE: Lazy<gdk::RGBA> =
    Lazy::new(|| gdk::RGBA::new(232. / 255., 231. / 255., 230. / 255., 1.));

static BACKGROUND_COLOR_DEFAULT_LIGHT_MODE: Lazy<gdk::RGBA> =
    Lazy::new(|| gdk::RGBA::new(250. / 255., 250. / 255., 250. / 255., 1.));
static BACKGROUND_COLOR_ALTERNATE_LIGHT_MODE: Lazy<gdk::RGBA> =
    Lazy::new(|| gdk::RGBA::new(103. / 255., 101. / 255., 110. / 255., 1.));

/// Consider 3.5:1 contrast and worse to be bad contrast for a pixel
static BACKGROUND_GUESS_LOW_CONTRAST_RATIO: f32 = 3.5;
/// Consider transparent images with more than 90% pixels bad contrast as bad
/// contrast
///
/// Bad contrast image will use the `BACKGROUND_COLOR_ALTERNATE`.
static BACKGROUND_GUESS_LOW_CONTRAST_TRHESHOLD: f64 = 0.90;

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
const RUBBERBANDING_EXPONENT: f64 = 0.3;

/// When this scale factor is reached, rotate is deactivated
const ZOOM_GESTURE_LOCK_THRESHOLD: f64 = 1.2;
/// When this rotate angle is reached, zoom is deactivated
const ROTATE_GESTURE_LOCK_THRESHOLD: f64 = 15.;

/// Max zoom level 2000%
const MAX_ZOOM_LEVEL: f64 = 20.0;

/// Thumbnail size in application pixels
///
/// The thumbnail is currently used for drag and drop.
const THUMBNAIL_SIZE: f32 = 128.;

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::LpImage)]
    pub struct LpImage {
        pub(super) file: RefCell<Option<gio::File>>,
        #[property(get)]
        pub(super) is_deleted: Cell<bool>,
        #[property(get)]
        pub(super) is_unsupported: Cell<bool>,
        /// Set to true when image is ready for displaying
        #[property(get)]
        pub(super) is_loaded: Cell<bool>,
        /// Set if an error has occurred, shown on error_page
        #[property(get)]
        pub(super) error: RefCell<Option<String>>,
        //pub(super) format: RefCell<Option<decoder::ImageFormat>>,
        pub(super) background_color: RefCell<Option<gdk::RGBA>>,

        /// Track changes to this image
        pub(super) file_monitor: RefCell<Option<gio::FileMonitor>>,
        pub(super) frame_buffer: Arc<ArcSwap<tiling::FrameBuffer>>,
        pub(super) decoder: RefCell<Option<Arc<Decoder>>>,

        /// Rotation final value (can differ from `rotation` during animation)
        pub(super) rotation_target: Cell<f64>,
        /// Rotated presentation of original image in degrees clockwise
        #[property(get, set = Self::set_rotation, explicit_notify)]
        pub(super) rotation: Cell<f64>,
        // Animates the `rotation` property
        pub(super) rotation_animation: OnceCell<adw::TimedAnimation>,

        /// Mirrored presentation of original image
        #[property(get, set = Self::set_mirrored, explicit_notify)]
        pub(super) mirrored: Cell<bool>,

        /// Displayed zoom level
        #[property(get, set = Self::set_zoom, explicit_notify)]
        pub(super) zoom: Cell<f64>,
        pub(super) zoom_animation: OnceCell<adw::TimedAnimation>,
        /// Targeted zoom level, might differ from `zoom` when animation is
        /// running
        pub(super) zoom_target: Cell<f64>,
        /// Current animation is transitioning from having horizontal scrollbars
        /// to not having them or vice versa.
        pub(super) zoom_hscrollbar_transition: Cell<bool>,
        /// Same but for vertical
        pub(super) zoom_vscrollbar_transition: Cell<bool>,

        /// Always fit image into window, causes `zoom` to change automatically
        #[property(get, set)]
        pub(super) best_fit: Cell<bool>,
        /// Max zoom level is reached, stored to only send signals on change
        #[property(get, set)]
        pub(super) is_max_zoom: Cell<bool>,

        /// Horizontal scrolling
        #[property(override_interface = gtk::Scrollable, get , set = Self::set_hadjustment)]
        pub(super) hadjustment: RefCell<gtk::Adjustment>,
        /// Vertical scrolling
        #[property(override_interface = gtk::Scrollable, get , set = Self::set_vadjustment)]
        pub(super) vadjustment: RefCell<gtk::Adjustment>,

        #[property(override_interface = gtk::Scrollable, get = Self::scroll_policy, set = Self::set_ignore_scroll_policy)]
        pub(super) _hscroll_policy: PhantomData<gtk::ScrollablePolicy>,
        #[property(override_interface = gtk::Scrollable, get = Self::scroll_policy, set = Self::set_ignore_scroll_policy)]
        pub(super) _vscroll_policy: PhantomData<gtk::ScrollablePolicy>,

        /// Currently EXIF data
        pub(super) metadata: RefCell<Metadata>,
        /// Image dimension details for SVGs
        pub(super) dimension_details: RefCell<decoder::ImageDimensionDetails>,

        #[property(get)]
        _image_size_available: bool,

        /// Current pointer position
        pub(super) pointer_position: Cell<Option<(f64, f64)>>,

        /// Position of fingers during zoom gesture
        ///
        /// Required for calculating delta when moving window on touchscreen.
        /// On touchpad this is only the initial value used as the zoom target.
        pub(super) zoom_gesture_center: Cell<Option<(f64, f64)>>,
        /// Required for calculating delta while moving window around
        pub(super) last_drag_value: Cell<Option<(f64, f64)>>,

        /// Ticks callback for animated image formats
        pub(super) tick_callback: RefCell<Option<gtk::TickCallbackId>>,
        /// Frame block time for currently shown frame
        pub(super) last_animated_frame: Cell<i64>,

        /// Gesture, zoom or rotate, used for the duration of the gesture
        pub(super) locked_gestured: Cell<Option<Gesture>>,

        pub(super) widget_dimensions: Cell<(i32, i32)>,
        pub(super) scaling: Cell<f64>,
        pub(super) surface_signals: OnceCell<SignalGroup>,
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
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec)
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> =
                Lazy::new(|| vec![Signal::builder("metadata-changed").build()]);
            SIGNALS.as_ref()
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
            self.scaling.set(obj.scaling());

            self.connect_input_handling();

            let surface_signals = self
                .surface_signals
                .get_or_init(SignalGroup::new::<gdk::Surface>);

            surface_signals.connect_notify_local(
                Some("scale"),
                glib::clone!(@weak obj => move |_, _| {
                    log::debug!("Scale changed signal");
                    obj.queue_resize();
                }),
            );

            obj.connect_realize(glib::clone!(@weak surface_signals => move |obj| {
                surface_signals.set_target(obj.native().map(|x| x.surface()).as_ref());
            }));

            obj.connect_unrealize(glib::clone!(@weak surface_signals => move |_| {
                surface_signals.set_target(gdk::Surface::NONE);
            }));

            adw::StyleManager::default().connect_dark_notify(glib::clone!(@weak obj => move |_| {
                glib::spawn_future_local(async move {
                    let color = obj.background_color_guess().await;
                    obj.set_background_color(color);
                    if obj.is_mapped() {
                        obj.queue_draw();
                    }
                });
            }));
        }

        fn dispose(&self) {
            let obj = self.obj();

            log::debug!("Disposing LpImage");

            // remove target from zoom animation because it's property of this object
            obj.rotation_animation()
                .set_target(&adw::CallbackAnimationTarget::new(|_| {}));
            obj.zoom_animation()
                .set_target(&adw::CallbackAnimationTarget::new(|_| {}));
        }
    }
}

glib::wrapper! {
    pub struct LpImage(ObjectSubclass<imp::LpImage>)
        @extends gtk::Widget,
        @implements gtk::Scrollable;
}
