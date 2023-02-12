use gio::prelude::*;

use super::{GPSLocation, ImageMetadata, Orientation};

use glib::subclass::prelude::*;
use gtk::glib;
use once_cell::sync::Lazy;

use std::cell::RefCell;
use std::path::Path;

glib::wrapper! {
    pub struct LpImageMetadata(ObjectSubclass<imp::LpImageMetadata>);
}

impl LpImageMetadata {
    pub fn load(path: &Path) -> Self {
        let obj = glib::Object::new::<Self>();

        let metadata = ImageMetadata::load(path);
        obj.imp().metadata.replace(metadata);

        obj
    }

    pub fn orientation(&self) -> Orientation {
        self.imp().metadata.borrow().orientation()
    }

    pub fn gps_location(&self) -> Option<GPSLocation> {
        self.imp().metadata.borrow().gps_location()
    }
}

impl From<ImageMetadata> for LpImageMetadata {
    fn from(metadata: ImageMetadata) -> Self {
        let obj = glib::Object::new::<Self>();

        obj.imp().metadata.replace(metadata);

        obj
    }
}

impl Default for LpImageMetadata {
    fn default() -> Self {
        glib::Object::new()
    }
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct LpImageMetadata {
        pub(super) metadata: RefCell<ImageMetadata>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpImageMetadata {
        const NAME: &'static str = "LpImageMetadata";
        type Type = super::LpImageMetadata;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for LpImageMetadata {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                let mut vec = vec![glib::ParamSpecBoolean::builder("has-information").build()];

                vec.append(
                    &mut [
                        "originally-created",
                        "location",
                        "f-number",
                        "exposure-time",
                        "iso",
                        "focal-length",
                        "maker-model",
                    ]
                    .iter()
                    .map(|name| glib::ParamSpecString::builder(name).read_only().build())
                    .collect(),
                );

                vec
            });

            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let metadata = self.metadata.borrow();
            match pspec.name() {
                "has-information" => (!metadata.is_none()).to_value(),
                "originally-created" => metadata.originally_created().to_value(),
                "location" => metadata.gps_location().map(|x| x.display()).to_value(),
                "f-number" => metadata.f_number().to_value(),
                "exposure-time" => metadata.exposure_time().to_value(),
                "iso" => metadata.iso().to_value(),
                "focal-length" => metadata.focal_length().to_value(),
                "maker-model" => metadata.maker_model().to_value(),
                name => unimplemented!("property {name}"),
            }
        }
    }
}
