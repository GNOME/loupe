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

use gtk::{prelude::*, subclass::prelude::*};

use once_cell::sync::Lazy;

use std::cell::{Cell, RefCell};

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct LpImage {
        pub file: RefCell<Option<gio::File>>,
        pub image_width: Cell<i32>,
        pub image_height: Cell<i32>,
        pub texture: RefCell<Option<gdk::Texture>>,
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
                vec![glib::ParamSpecObject::new(
                    "file",
                    "File",
                    "The current file",
                    gio::File::static_type(),
                    glib::ParamFlags::READWRITE,
                )]
            });

            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = self.instance();
            match pspec.name() {
                "file" => obj.file().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.instance();
            match pspec.name() {
                "file" => obj.set_file(&value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self) {
            let obj = self.instance();
            obj.set_hexpand(true);
            obj.set_vexpand(true);
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
                let width = widget.width() as f64;
                let height = widget.height() as f64;
                let texture_ratio = texture.intrinsic_aspect_ratio();

                if texture_ratio == 0.0 {
                    texture.snapshot(snapshot, width, height);
                } else {
                    let widget_ratio = width / height;

                    let (snapshot_width, snapshot_height) = if texture_ratio > widget_ratio {
                        (width, width / texture_ratio)
                    } else {
                        (height * texture_ratio, height)
                    };

                    let x = (width - snapshot_width.ceil()) / 2.0;
                    let y = (height - snapshot_height.ceil()).floor() / 2.0;

                    snapshot.save();
                    snapshot.translate(&graphene::Point::new(x as f32, y as f32));
                    texture.snapshot(snapshot, snapshot_width, snapshot_height);
                    snapshot.restore();
                }
            }
        }

        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(&self, orienation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            if let Some(texture) = self.texture.borrow().as_ref() {
                let default_size = 16.0; // Fall back to default icon size; Just a guess since the API GtkPicture uses is private
                let measurement = if for_size < 0 { 0 } else { for_size };
                let (min_width, min_height): (f64, f64) = (0.0, 0.0);

                let (min, nat) = if orienation == gtk::Orientation::Horizontal {
                    let size = texture.compute_concrete_size(
                        0.0,
                        measurement.into(),
                        default_size,
                        default_size,
                    );
                    let nat_width = size.0;

                    (min_width.ceil(), nat_width.ceil())
                } else {
                    let size = texture.compute_concrete_size(
                        measurement.into(),
                        0.0,
                        default_size,
                        default_size,
                    );
                    let nat_height = size.1;

                    (min_height.ceil(), nat_height.ceil())
                };

                (min as i32, nat as i32, -1, -1)
            } else {
                (0, 0, -1, -1)
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

    pub fn set_file(&self, file: &gio::File) {
        let imp = self.imp();

        match gdk::Texture::from_file(file) {
            Ok(texture) => {
                imp.image_width.set(texture.width());
                imp.image_height.set(texture.height());
                imp.file.replace(Some(file.clone()));
                imp.texture.replace(Some(texture));
            }
            Err(e) => log::error!("Could not load a valid image from this file: {}", e),
        };

        self.queue_draw();
        self.queue_resize();
    }

    pub fn image_width(&self) -> i32 {
        let imp = self.imp();
        imp.image_width.get()
    }

    pub fn image_height(&self) -> i32 {
        let imp = self.imp();
        imp.image_height.get()
    }

    pub fn set_texture_with_file(&self, texture: gdk::Texture, source_file: &gio::File) {
        let imp = self.imp();
        imp.texture.replace(Some(texture));
        imp.file.replace(Some(source_file.clone()));

        self.queue_draw();
        self.queue_resize();
    }

    pub fn texture(&self) -> Option<gdk::Texture> {
        let imp = self.imp();
        imp.texture.borrow().clone()
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
}
