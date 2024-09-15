use std::cell::{Cell, OnceCell};

use adw::prelude::*;
use adw::subclass::prelude::*;
use adw::{glib, gtk};

use crate::widgets::edit::LpEditCropSelection;
use crate::widgets::LpImage;

#[derive(Debug, Clone, Copy, Default, glib::Enum)]
#[enum_type(name = "LpAspectRatio")]
pub enum LpAspectRatio {
    #[default]
    Free,
    Original,
    /// 1.0
    Square,
    /// 1.25
    R5to4,
    /// 1.33
    R4to3,
    /// 1.5
    R3to2,
    /// 1.6
    R16to10,
    /// 1.77
    R16to9,
}

#[derive(Debug, Clone, Copy, Default, glib::Enum)]
#[enum_type(name = "LpOrientation")]
pub enum LpOrientation {
    #[default]
    Landscape,
    Portrait,
}

mod imp {
    use super::*;
    use crate::widgets::LpImage;
    #[derive(Debug, Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::LpEditCrop)]
    #[template(file = "crop.ui")]
    pub struct LpEditCrop {
        #[template_child]
        image: TemplateChild<LpImage>,
        #[template_child]
        pub(super) selection: TemplateChild<LpEditCropSelection>,

        #[property(get, set, builder(LpAspectRatio::default()))]
        aspect_ratio: Cell<LpAspectRatio>,
        #[property(get, set, builder(LpOrientation::default()))]
        orientation: Cell<LpOrientation>,

        #[property(get, construct_only)]
        original_image: OnceCell<LpImage>,

        #[property(get, set)]
        child: OnceCell<gtk::Widget>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpEditCrop {
        const NAME: &'static str = "LpEditCrop";
        type Type = super::LpEditCrop;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);

            klass.install_property_action("edit-crop.aspect-ratio", "aspect_ratio");
            klass.install_property_action("edit-crop.orientation", "orientation");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for LpEditCrop {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = &*self.obj();

            obj.child().set_parent(obj);

            obj.set_layout_manager(None::<gtk::LayoutManager>);
            self.image.duplicate_from(&obj.original_image());
        }

        fn dispose(&self) {
            self.obj().child().unparent();
        }
    }

    impl WidgetImpl for LpEditCrop {
        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            let obj = self.obj();

            let image: &TemplateChild<LpImage> = &self.image;

            obj.child().allocate(width, height, baseline, None);

            // TODO: Round wrong
            self.selection.set_size(
                image.image_rendering_x() as i32,
                image.image_rendering_y() as i32,
                image.image_displayed_width().round() as i32,
                image.image_displayed_height().round() as i32,
            );
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            self.obj().child().measure(orientation, for_size)
        }
    }
    impl BinImpl for LpEditCrop {}
}

glib::wrapper! {
    pub struct LpEditCrop(ObjectSubclass<imp::LpEditCrop>)
    @extends gtk::Widget, adw::Bin;
}

impl LpEditCrop {
    pub fn new(original_image: LpImage) -> Self {
        glib::Object::builder()
            .property("original_image", original_image)
            .build()
    }

    pub fn selection(&self) -> LpEditCropSelection {
        self.imp().selection.clone()
    }
}
