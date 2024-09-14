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

use adw::prelude::*;
use adw::subclass::prelude::*;
use adw::{glib, gtk};

mod imp {
    use super::*;
    #[derive(Debug, Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::LpImageEdit)]
    #[template(file = "edit_window.ui")]
    pub struct LpImageEdit {
        #[property(get)]
        example: u64,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpImageEdit {
        const NAME: &'static str = "LpImageEdit";
        type Type = super::LpImageEdit;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for LpImageEdit {
        fn constructed(&self) {
            self.parent_constructed();
        }
    }

    impl WidgetImpl for LpImageEdit {}
    impl BinImpl for LpImageEdit {}
}

glib::wrapper! {
    pub struct LpImageEdit(ObjectSubclass<imp::LpImageEdit>)
    @extends gtk::Widget, adw::Bin;
}

impl LpImageEdit {
    pub fn new() -> Self {
        glib::Object::new()
    }
}
