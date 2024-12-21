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

use gtk::prelude::*;

use crate::widgets::LpWindow;

pub trait ParentWindow: WidgetExt {
    fn window(&self) -> LpWindow {
        let result = self.root().and_downcast();

        if result.is_none() {
            log::error!("Couldn't find LpWindow for {self:?}");
        }

        result.unwrap()
    }
}

impl<T: WidgetExt> ParentWindow for T {}