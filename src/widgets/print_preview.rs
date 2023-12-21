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

use adw::subclass::prelude::*;
use glib::Properties;
use gtk::prelude::*;
use once_cell::sync::OnceCell;

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
    }

    impl WidgetImpl for LpPrintPreviewPage {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            snapshot.save();

            let scale = self.print().user_scale_original() * self.display_scale();
            let margin_left = self.print().effective_margin_left() * self.display_scale();
            let margin_top = self.print().effective_margin_top() * self.display_scale();

            if let Some(texture) = self.print().image().print_data(scale) {
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

    impl LpPrintPreviewPage {
        fn preview(&self) -> super::LpPrintPreview {
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
        #[property(get, set)]
        print: OnceCell<LpPrint>,
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

            self.page.queue_draw();
        }
    }

    impl LpPrintPreview {
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
        @extends gtk::Widget;
}

impl Default for LpPrintPreviewPage {
    fn default() -> Self {
        glib::Object::new()
    }
}

glib::wrapper! {
    pub struct LpPrintPreview(ObjectSubclass<imp::LpPrintPreview>)
        @extends gtk::Widget;
}
