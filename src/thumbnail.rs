use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib};

mod imp {
    use super::*;

    use once_cell::sync::OnceCell;

    const SIZE: i32 = 128;

    #[derive(Debug, Default)]
    pub struct Thumbnail {
        pub image: OnceCell<gdk::Paintable>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Thumbnail {
        const NAME: &'static str = "Thumbnail";
        type Type = super::Thumbnail;
        type ParentType = glib::Object;
        type Interfaces = (gdk::Paintable,);
    }

    impl ObjectImpl for Thumbnail {}
    impl PaintableImpl for Thumbnail {
        fn intrinsic_height(&self, _paintable: &Self::Type) -> i32 {
            let image = self.image.get().unwrap();
            let width = image.intrinsic_width();
            let height = image.intrinsic_height();
            let aspect_ratio = width as f64 / (height as f64).max(std::f64::EPSILON);

            if width >= height {
                (SIZE as f64 / aspect_ratio) as i32
            } else {
                SIZE
            }
        }

        fn intrinsic_width(&self, _paintable: &Self::Type) -> i32 {
            let image = self.image.get().unwrap();
            let width = image.intrinsic_width();
            let height = image.intrinsic_height();
            let aspect_ratio = width as f64 / (height as f64).max(std::f64::EPSILON);

            if width >= height {
                SIZE
            } else {
                (SIZE as f64 * aspect_ratio) as i32
            }
        }

        fn intrinsic_aspect_ratio(&self, _paintable: &Self::Type) -> f64 {
            self.image.get().unwrap().intrinsic_aspect_ratio()
        }

        fn snapshot(
            &self,
            paintable: &Self::Type,
            snapshot: &gdk::Snapshot,
            _width: f64,
            _height: f64,
        ) {
            let width = paintable.intrinsic_width() as f64;
            let height = paintable.intrinsic_height() as f64;

            self.image.get().unwrap().snapshot(snapshot, width, height);
        }
    }
}

glib::wrapper! {
    /// Thumbnail Paintable
    ///
    /// Creates a thumbnail from another paintable. The thumbnail will be drawn
    /// at a maximum size of 128 × 128 taking into account the original
    /// paintable aspect ratio.
    pub struct Thumbnail(ObjectSubclass<imp::Thumbnail>) @implements gdk::Paintable;
}

impl Thumbnail {
    pub fn new(paintable: &impl glib::IsA<gdk::Paintable>) -> Self {
        let object = glib::Object::new::<Self>(&[]).unwrap();
        let imp = object.imp();

        imp.image.set(paintable.clone().upcast()).unwrap();

        object
    }
}
