// Copyright (c) 2024 Sophie Herold
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

use std::cell::OnceCell;

use adw::prelude::*;
use adw::subclass::prelude::*;

use super::edit::LpEditCrop;
use super::LpImage;
use crate::deps::*;
mod imp {

    use super::*;
    use crate::widgets::LpImage;

    #[derive(Debug, Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::LpEditWindow)]
    #[template(file = "edit_window.ui")]
    pub struct LpEditWindow {
        #[template_child]
        toolbar_view: TemplateChild<adw::ToolbarView>,

        #[property(get, construct_only)]
        original_image: OnceCell<LpImage>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpEditWindow {
        const NAME: &'static str = "LpEditWindow";
        type Type = super::LpEditWindow;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for LpEditWindow {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            self.toolbar_view
                .set_content(Some(&LpEditCrop::new(obj.original_image())));
        }
    }

    impl WidgetImpl for LpEditWindow {}
    impl BinImpl for LpEditWindow {}
}

glib::wrapper! {
    pub struct LpEditWindow(ObjectSubclass<imp::LpEditWindow>)
    @extends gtk::Widget, adw::Bin;
}

impl LpEditWindow {
    pub fn new(image: LpImage) -> Self {
        glib::Object::builder()
            .property("original_image", image)
            .build()
    }
}
