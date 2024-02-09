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

use super::*;

impl LpImage {
    /// Stepwise scrolls inside an image when zoomed in
    pub fn pan(&self, direction: &gtk::PanDirection) {
        let imp = self.imp();

        let sign = match direction {
            gtk::PanDirection::Left | gtk::PanDirection::Up => -1.,
            gtk::PanDirection::Right | gtk::PanDirection::Down => 1.,
            _ => {
                log::error!("Unknown pan direction {direction:?}");
                return;
            }
        };

        let (adjustment, max) = match direction {
            gtk::PanDirection::Left | gtk::PanDirection::Right => {
                (self.hadj(), imp.max_hadjustment_value())
            }
            gtk::PanDirection::Up | gtk::PanDirection::Down => {
                (self.vadj(), imp.max_vadjustment_value())
            }
            _ => {
                log::error!("Unknown pan direction {direction:?}");
                return;
            }
        };

        let value = (adjustment.value() + sign * adjustment.step_increment()).clamp(0., max);

        adjustment.set_value(value);
    }
}
