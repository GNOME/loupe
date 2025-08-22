// Copyright (c) 2023-2024 Sophie Herold
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

//! A bin that only has min width as natural width
//!
//! This bin does not claim its complete width as natural width. The HeaderBar
//! is wrapped in with this widget. This prevents the window from growing based
//! on a long file name in the title instead of just fitting to the image
//! size.

use adw::prelude::*;
use adw::subclass::prelude::*;

use crate::deps::*;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct LpShyBin {}

    #[glib::object_subclass]
    impl ObjectSubclass for LpShyBin {
        const NAME: &'static str = "LpShyBin";
        type Type = super::LpShyBin;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for LpShyBin {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();

            obj.set_layout_manager(None::<gtk::LayoutManager>);
            obj.set_hexpand(true);
        }
    }

    impl WidgetImpl for LpShyBin {
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let mut measure = self.obj().child().unwrap().measure(orientation, for_size);

            if orientation == gtk::Orientation::Horizontal {
                // Set natural size to minimal size
                measure.1 = measure.0;
            }

            measure
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.obj()
                .child()
                .unwrap()
                .allocate(width, height, baseline, None);
        }
    }

    impl BinImpl for LpShyBin {}
}

glib::wrapper! {
    pub struct LpShyBin(ObjectSubclass<imp::LpShyBin>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::Buildable, gtk::Accessible, gtk::ConstraintTarget;
}
