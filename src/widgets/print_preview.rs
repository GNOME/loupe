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

//! Print preview widget that shows a fake page with the image on it

use std::cell::RefCell;

use adw::subclass::prelude::*;
use glib::Properties;
use gtk::prelude::*;

use super::image;
use crate::deps::*;
use crate::widgets::LpPrint;

mod imp_page {
    use super::*;

    #[derive(Debug, Default)]
    pub struct LpPrintPreviewPage {}

    #[glib::object_subclass]
    impl ObjectSubclass for LpPrintPreviewPage {
        const NAME: &'static str = "LpPrintPreviewPage";
        type Type = super::LpPrintPreviewPage;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("lpprintpreviewpage")
        }
    }

    impl ObjectImpl for LpPrintPreviewPage {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().set_overflow(gtk::Overflow::Hidden);
        }

        fn dispose(&self) {
            self.obj().first_child().unwrap().unparent();
        }
    }

    impl WidgetImpl for LpPrintPreviewPage {
        fn size_allocate(&self, _width: i32, _height: i32, _baseline: i32) {
            let margin_left = self.print().effective_margin_left() * self.display_scale();
            let margin_top = self.print().effective_margin_top() * self.display_scale();
            let width = self.print().user_width() * self.display_scale();
            let height = self.print().user_height() * self.display_scale();

            let image = self.print().preview_image();

            image.allocate(
                width as i32,
                height as i32,
                -1,
                Some(
                    gsk::Transform::new()
                        .translate(&graphene::Point::new(margin_left as f32, margin_top as f32)),
                ),
            );
        }
    }

    impl LpPrintPreviewPage {
        pub(super) fn preview(&self) -> super::LpPrintPreview {
            self.obj().parent().unwrap().downcast().unwrap()
        }

        fn print(&self) -> LpPrint {
            self.preview().print()
        }

        fn display_scale(&self) -> f64 {
            let print = self.print();
            let page_width = print.page_setup().paper_width(gtk::Unit::Inch) * print.dpi();
            self.obj().width() as f64 / page_width
        }
    }
}

mod imp {

    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::LpPrintPreview)]
    pub struct LpPrintPreview {
        #[property(type = LpPrint, get, set = Self::set_print)]
        print: RefCell<Option<LpPrint>>,
        page: LpPrintPreviewPage,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpPrintPreview {
        const NAME: &'static str = "LpPrintPreview";
        type Type = super::LpPrintPreview;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("lpprintpreview")
        }
    }

    impl ObjectImpl for LpPrintPreview {
        fn constructed(&self) {
            self.parent_constructed();

            self.page.insert_after(&*self.obj(), gtk::Widget::NONE);
        }

        fn dispose(&self) {
            self.page.unparent();
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
        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            let (page_width, page_height) = self.aspect_preserving_size(width, height);

            self.page.size_allocate(
                &gdk::Rectangle::new(
                    (width - page_width) / 2,
                    (height - page_height) / 2,
                    page_width,
                    page_height,
                ),
                baseline,
            );

            self.page.queue_resize();
        }
    }

    impl LpPrintPreview {
        fn set_print(&self, print: LpPrint) {
            let page = self.page.clone();
            print.connect_image_notify(move |_| {
                page.init();
            });

            self.print.replace(Some(print));
            self.obj().notify_print();
        }

        fn aspect_preserving_size(&self, for_width: i32, for_height: i32) -> (i32, i32) {
            let print = self.obj().print();

            let ratio = (print.page_setup().paper_width(gtk::Unit::Inch)
                / print.page_setup().paper_height(gtk::Unit::Inch)) as f32;

            if ratio > for_width as f32 / for_height as f32 {
                (for_width, (for_width as f32 / ratio) as i32)
            } else {
                ((for_height as f32 * ratio) as i32, for_height)
            }
        }
    }
}
glib::wrapper! {
    pub struct LpPrintPreviewPage(ObjectSubclass<imp_page::LpPrintPreviewPage>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for LpPrintPreviewPage {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl LpPrintPreviewPage {
    fn init(&self) {
        let imp = self.imp();
        let image = imp.preview().print().preview_image();

        image.set_sensitive(false);
        image.set_parent(self);
        image.set_fit_mode(image::FitMode::LargeFit);
    }
}

glib::wrapper! {
    pub struct LpPrintPreview(ObjectSubclass<imp::LpPrintPreview>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}
