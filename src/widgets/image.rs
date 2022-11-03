// image.rs
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

use crate::deps::*;

use adw::{prelude::*, subclass::prelude::*};

use once_cell::sync::Lazy;

use std::cell::{Cell, RefCell};

use crate::image_metadata::ImageMetadata;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct LpImage {
        pub file: RefCell<Option<gio::File>>,
        pub texture: RefCell<Option<gdk::Texture>>,
        /// Rotation final value (can differ from `rotation` during animation)
        pub rotation_target: Cell<f64>,
        /// Rotated presentation of original image in degrees clockwise
        pub rotation: Cell<f64>,
        // Animates the `rotation` property
        pub rotation_animation: RefCell<Option<adw::TimedAnimation>>,
        /// Mirrored presentation of original image
        pub mirrored: Cell<bool>,
        /// Currently EXIF data
        pub image_metadata: RefCell<ImageMetadata>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpImage {
        const NAME: &'static str = "LpImage";
        type ParentType = gtk::Widget;
        type Type = super::LpImage;
    }

    impl ObjectImpl for LpImage {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecObject::builder::<gio::File>("file")
                        .read_only()
                        .build(),
                    glib::ParamSpecDouble::builder("rotation").build(),
                    glib::ParamSpecBoolean::builder("mirrored").build(),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = self.instance();
            match pspec.name() {
                "file" => obj.file().to_value(),
                "rotation" => obj.rotation().to_value(),
                "mirrored" => obj.mirrored().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.instance();
            match pspec.name() {
                "rotation" => obj.set_rotation(value.get().unwrap()),
                "mirrored" => obj.set_mirrored(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self) {
            let obj = self.instance();
            obj.set_hexpand(true);
            obj.set_vexpand(true);

            let rotation_animation = adw::TimedAnimation::builder()
                .duration(200)
                .widget(&*obj)
                .target(&adw::PropertyAnimationTarget::new(&*obj, "rotation"))
                .build();
            self.rotation_animation.replace(Some(rotation_animation));
        }

        fn dispose(&self) {
            let obj = self.instance();
            while let Some(child) = obj.first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for LpImage {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            if let Some(texture) = self.texture.borrow().as_ref() {
                let widget = self.instance();
                let widget_width = widget.width() as f64;
                let widget_height = widget.height() as f64;
                let texture_ratio = texture.intrinsic_aspect_ratio();
                let rotated = widget.rotation().to_radians().sin().abs();

                if texture_ratio == 0.0 {
                    // TODO: We probably need rotation etc on this?
                    // Maybe just set ratio to 1. or widget_width/widget_height and run code below?
                    texture.snapshot(snapshot, widget_width, widget_height);
                } else {
                    let widget_ratio = widget_width / widget_height;

                    // image dimension for default orientation
                    let (snapshot_width_default, snapshot_height_default) =
                        if texture_ratio > widget_ratio {
                            (widget_width, widget_width / texture_ratio)
                        } else {
                            (widget_height * texture_ratio, widget_height)
                        };

                    // image dimensions for 90deg rotated
                    let (snapshot_width_rotated, snapshot_height_rotated) =
                        if (1.0 / texture_ratio) > widget_ratio {
                            (widget_width * texture_ratio, widget_width)
                        } else {
                            (widget_height, widget_height / texture_ratio)
                        };

                    // interpolate between size computation during rotation
                    let mut snapshot_height = rotated * snapshot_height_rotated
                        + (1. - rotated) * snapshot_height_default;
                    let mut snapshot_width =
                        rotated * snapshot_width_rotated + (1. - rotated) * snapshot_width_default;

                    // Don't make the image larger then default size
                    if snapshot_height > texture.height() as f64
                        || snapshot_width > texture.width() as f64
                    {
                        snapshot_height = texture.height() as f64;
                        snapshot_width = texture.width() as f64;
                    }

                    snapshot.save();

                    // center image in widget
                    let x = (widget_width - snapshot_width) / 2.0;
                    let y = (widget_height - snapshot_height) / 2.0;
                    snapshot.translate(&graphene::Point::new(x as f32, y as f32));

                    // center image in coordinates to rotate origin
                    snapshot.translate(&graphene::Point::new(
                        (snapshot_width / 2.) as f32,
                        (snapshot_height / 2.) as f32,
                    ));

                    // apply the transformations from properties
                    snapshot.rotate(widget.rotation() as f32);
                    if widget.mirrored() {
                        snapshot.scale(-1., 1.);
                    }

                    // move back to original position
                    snapshot.translate(&graphene::Point::new(
                        (-snapshot_width / 2.) as f32,
                        (-snapshot_height / 2.) as f32,
                    ));

                    texture.snapshot(snapshot, snapshot_width, snapshot_height);
                    snapshot.restore();
                }
            }
        }
    }
}

glib::wrapper! {
    pub struct LpImage(ObjectSubclass<imp::LpImage>) @extends gtk::Widget;
}

impl LpImage {
    pub fn file(&self) -> Option<gio::File> {
        let imp = self.imp();

        imp.file.borrow().clone()
    }

    pub fn set_texture_with_file(&self, texture: gdk::Texture, source_file: &gio::File) {
        let imp = self.imp();

        imp.texture.replace(Some(texture));
        imp.file.replace(Some(source_file.clone()));

        let metadata = ImageMetadata::load(source_file);
        let orientation = metadata.orientation();

        imp.image_metadata.replace(metadata);

        imp.rotation_target.set(-orientation.rotation);
        imp.rotation.set(-orientation.rotation);
        imp.mirrored.set(orientation.mirrored);

        self.queue_draw();
    }

    pub fn texture(&self) -> Option<gdk::Texture> {
        let imp = self.imp();
        imp.texture.borrow().clone()
    }

    pub fn rotation(&self) -> f64 {
        self.imp().rotation.get()
    }

    pub fn set_rotation(&self, rotation: f64) {
        self.imp().rotation.set(rotation);
        self.queue_draw();
    }

    pub fn mirrored(&self) -> bool {
        self.imp().mirrored.get()
    }

    pub fn set_mirrored(&self, mirrored: bool) {
        self.imp().mirrored.set(mirrored);
        self.queue_draw();
    }

    pub fn content_provider(&self) -> gdk::ContentProvider {
        let imp = self.imp();
        let mut contents = vec![];

        if let Some(file) = imp.file.borrow().as_ref() {
            let content = gdk::ContentProvider::for_value(&file.to_value());
            contents.push(content);
        }

        if let Some(texture) = imp.texture.borrow().as_ref() {
            let bytes = texture.save_to_png_bytes();
            let content = gdk::ContentProvider::for_bytes("image/png", &bytes);
            contents.push(content);
        }

        gdk::ContentProvider::new_union(contents.as_slice())
    }

    pub fn rotate(&self) {
        let target = &self.imp().rotation_target;
        target.set(target.get() + 90.);

        let borrow = self.imp().rotation_animation.borrow();
        let animation = borrow.as_ref().unwrap();

        animation.set_value_from(self.rotation());
        animation.set_value_to(target.get());
        animation.play();
    }
}
