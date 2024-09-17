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

impl ScrollableImpl for imp::LpImage {}

impl imp::LpImage {
    /// Adjustment setter needed for Scrollable implementation
    ///
    /// The adjustment get's set by the `GtkScrolledWindow`
    pub fn set_hadjustment(&self, hadjustment: Option<gtk::Adjustment>) {
        let obj = self.obj();

        let adjustment = hadjustment.unwrap_or_default();

        adjustment.connect_value_changed(glib::clone!(
            #[weak]
            obj,
            move |_| {
                obj.imp().request_tiles();
                obj.queue_draw();
            }
        ));

        self.hadjustment.replace(adjustment);
        self.configure_adjustments();
    }

    pub fn set_vadjustment(&self, vadjustment: Option<gtk::Adjustment>) {
        let obj = self.obj();

        let adjustment = vadjustment.unwrap_or_default();

        adjustment.connect_value_changed(glib::clone!(
            #[weak]
            obj,
            move |_| {
                obj.imp().request_tiles();
                obj.queue_draw();
            }
        ));

        self.vadjustment.replace(adjustment);
        self.configure_adjustments();
    }

    pub fn set_ignore_scroll_policy(&self, scroll_policy: gtk::ScrollablePolicy) {
        log::error!("Ignored setting new scroll policy {scroll_policy:?}");
    }

    pub fn scroll_policy(&self) -> gtk::ScrollablePolicy {
        gtk::ScrollablePolicy::Minimum
    }

    /// Configure scrollbars for current situation
    pub(super) fn configure_adjustments(&self) {
        let obj = self.obj();

        let hadjustment = obj.hadj();
        // round to application pixels to avoid tiny rounding errors from zoom
        let content_width = self.round_f64(self.image_displayed_width());
        let widget_width = self.widget_width();

        hadjustment.configure(
            // value
            hadjustment.value().clamp(0., self.max_hadjustment_value()),
            // lower
            0.,
            // upper
            content_width,
            // arrow button and shortcut step
            widget_width * 0.1,
            // page up/down step
            widget_width * 0.9,
            // page size
            f64::min(widget_width, content_width),
        );

        let vadjustment = obj.vadj();
        // round to application pixels to avoid tiny rounding errors from zoom
        let content_height = self.round_f64(self.image_displayed_height());
        let widget_height = self.widget_height();

        vadjustment.configure(
            vadjustment.value().clamp(0., self.max_vadjustment_value()),
            // lower
            0.,
            // upper
            content_height,
            // arrow button and shortcut step
            widget_height * 0.1,
            // page up/down step
            widget_height * 0.9,
            // page_size
            f64::min(widget_height, content_height),
        );
    }

    pub fn set_hadj_value(&self, value: f64) {
        let hadjustment = self.obj().hadj();
        let value = value.clamp(0., hadjustment.upper() - hadjustment.page_size());
        hadjustment.set_value(value);
    }

    pub fn set_vadj_value(&self, value: f64) {
        let vadjustment = self.obj().vadj();
        let value = value.clamp(0., vadjustment.upper() - vadjustment.page_size());
        vadjustment.set_value(value);
    }

    pub fn hadj_value(&self) -> f64 {
        self.obj().hadj().value()
    }

    pub fn vadj_value(&self) -> f64 {
        self.obj().vadj().value()
    }

    pub fn max_hadjustment_value(&self) -> f64 {
        f64::max(self.image_displayed_width() - self.widget_width(), 0.)
    }

    pub fn max_vadjustment_value(&self) -> f64 {
        f64::max(self.image_displayed_height() - self.widget_height(), 0.)
    }
}

impl LpImage {
    pub fn is_hscrollable(&self) -> bool {
        self.imp().max_hadjustment_value() != 0.
    }

    pub fn is_vscrollable(&self) -> bool {
        self.imp().max_vadjustment_value() != 0.
    }

    pub fn hadj(&self) -> gtk::Adjustment {
        self.imp().hadjustment.borrow().clone()
    }

    pub fn vadj(&self) -> gtk::Adjustment {
        self.imp().vadjustment.borrow().clone()
    }
}
