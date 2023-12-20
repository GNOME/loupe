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

//! A window title
//!
//! This window title does not claim its complete width as natural width. This
//! avoids the window to grow to a large file name title instead of just
//! fittingly for the image size.

use adw::prelude::*;
use adw::subclass::prelude::*;

use crate::deps::*;

mod imp {
    use super::*;

    #[derive(Debug, Default, glib::Properties)]
    #[properties(wrapper_type = super::LpWindowTitle)]
    pub struct LpWindowTitle {
        #[property(type = String, set = Self::set_title)]
        _title: (),
        window_title: adw::WindowTitle,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpWindowTitle {
        const NAME: &'static str = "LpWindowTitle";
        type Type = super::LpWindowTitle;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for LpWindowTitle {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();

            obj.set_layout_manager(None::<gtk::LayoutManager>);
            obj.set_child(Some(&self.window_title));
            obj.set_hexpand(true);
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
    impl WidgetImpl for LpWindowTitle {
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let mut measure = self.window_title.measure(orientation, for_size);

            if orientation == gtk::Orientation::Horizontal {
                // Set natural size to minimal size
                measure.1 = measure.0;
            }

            measure
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.window_title.allocate(width, height, baseline, None);
        }
    }
    impl BinImpl for LpWindowTitle {}

    impl LpWindowTitle {
        fn set_title(&self, title: String) {
            self.window_title.set_title(&title);
        }
    }
}

glib::wrapper! {
    pub struct LpWindowTitle(ObjectSubclass<imp::LpWindowTitle>)
        @extends gtk::Widget, adw::Bin;
}
