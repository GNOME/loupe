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

use crate::deps::*;
use crate::widgets::LpImage;
use crate::window::LpWindow;

use adw::subclass::prelude::*;
use glib::Properties;
use gtk::prelude::*;
use once_cell::sync::OnceCell;

mod imp {
    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate, Properties)]
    #[properties(wrapper_type = super::LpZoomTo)]
    #[template(file = "../../data/gtk/zoom_to.ui")]
    pub struct LpZoomTo {
        //#[template_child]
        //zoom: TemplateChild<adw::SpinRow>,
        #[property(get, set, builder().construct_only())]
        parent_window: OnceCell<LpWindow>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpZoomTo {
        const NAME: &'static str = "LpZoomTo";
        type Type = super::LpZoomTo;
        type ParentType = adw::Window;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for LpZoomTo {
        fn constructed(&self) {
            let obj = self.obj();
            self.parent_constructed();

            obj.set_transient_for(Some(&obj.parent_window()));
            obj.set_modal(true);
            /*
            self.zoom.set_increments(10., 100.);

            self.apply
                .connect_clicked(glib::clone!(@weak obj => move |_| {
                    if let Some(image) = obj.image() {
                        image.zoom_to(obj.imp().zoom.value() / 100.);
                    }
                }));

            if let Some(image) = obj.parent_window().image_view().current_image() {
                self.zoom.set_range(
                    dbg!(image.zoom_level_best_fit() * 100.),
                    dbg!(image.max_zoom() * 100.),
                );
                self.zoom.set_value(image.zoom());
            }*/
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
    impl WidgetImpl for LpZoomTo {}
    impl WindowImpl for LpZoomTo {}
    impl AdwWindowImpl for LpZoomTo {}
}
glib::wrapper! {
    pub struct LpZoomTo(ObjectSubclass<imp::LpZoomTo>)
        @extends gtk::Widget, gtk::Window;
}

#[gtk::template_callbacks]
impl LpZoomTo {
    pub fn new(parent_window: LpWindow) -> Self {
        glib::Object::builder()
            .property("parent-window", parent_window)
            .build()
    }

    pub fn image(&self) -> Option<LpImage> {
        self.parent_window().image_view().current_image()
    }
}
