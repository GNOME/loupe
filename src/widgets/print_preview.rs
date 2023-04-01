//! Print preview widget that shows a fake page with the image on it

use crate::deps::*;
use crate::widgets::LpPrint;

use adw::subclass::prelude::*;
use glib::Properties;
use gtk::prelude::*;
use once_cell::sync::OnceCell;

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::LpPrintPreview)]
    pub struct LpPrintPreview {
        #[property(get, set)]
        print: OnceCell<LpPrint>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpPrintPreview {
        const NAME: &'static str = "LpPrintPreview";
        type Type = super::LpPrintPreview;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for LpPrintPreview {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().set_halign(gtk::Align::Center);
            self.obj().set_valign(gtk::Align::Start);
            self.obj().set_overflow(gtk::Overflow::Hidden);
        }

        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }
        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec)
        }
        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }
    }

    impl WidgetImpl for LpPrintPreview {
        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            let (width, height) = self.display_paper_dimensions();

            let size = match orientation {
                gtk::Orientation::Horizontal => width,
                _ => height,
            };

            (size, size, -1, -1)
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let obj = self.obj();

            snapshot.save();

            let scale = obj.print().user_scale() * self.display_scale();
            let margin_left = obj.print().user_margin_left() * self.display_scale();
            let margin_top = obj.print().user_margin_top() * self.display_scale();

            if let Some(texture) = self.obj().print().image().print_data(scale) {
                let area = graphene::Rect::new(
                    margin_left as f32,
                    margin_top as f32,
                    texture.width() as f32,
                    texture.height() as f32,
                );

                snapshot.append_texture(&texture, &area);

                // Border around image
                snapshot.append_inset_shadow(
                    &gsk::RoundedRect::from_rect(area, 0.),
                    &gdk::RGBA::new(0., 0., 0., 0.3),
                    0.,
                    0.,
                    1.,
                    0.,
                );
            }
            snapshot.restore();
        }
    }

    impl LpPrintPreview {
        /// Returns scale factor from print pixel to preview size
        fn display_scale(&self) -> f64 {
            let print = self.obj().print();

            let height = print.page_setup().paper_height(gtk::Unit::Inch) * print.dpi();
            let width = print.page_setup().paper_width(gtk::Unit::Inch) * print.dpi();

            300. / f64::max(width, height)
        }

        /// Returns total preview size in pixels
        fn display_paper_dimensions(&self) -> (i32, i32) {
            let print = self.obj().print();

            let height = print.page_setup().paper_height(gtk::Unit::Inch) * print.dpi();
            let width = print.page_setup().paper_width(gtk::Unit::Inch) * print.dpi();

            let scale = self.display_scale();

            ((width * scale) as i32, (height * scale) as i32)
        }
    }
}
glib::wrapper! {
    pub struct LpPrintPreview(ObjectSubclass<imp::LpPrintPreview>)
        @extends gtk::Widget;
}

#[gtk::template_callbacks]
impl LpPrintPreview {}
