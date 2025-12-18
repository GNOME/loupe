// Copyright (c) 2024-2025 Sophie Herold
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

use crate::widgets::{LpEditWindow, LpWindow};

pub trait ParentWindow: WidgetExt {
    #[track_caller]
    fn window(&self) -> LpWindow {
        let result = self.root().and_downcast();

        if result.is_none() {
            tracing::error!(
                "Couldn't find LpWindow for {self:?}. mapped={}. caller={}",
                self.is_mapped(),
                std::panic::Location::caller(),
            );
        }

        result.unwrap()
    }

    #[track_caller]
    fn try_window(&self) -> Option<LpWindow> {
        let result = self.root().and_downcast();

        if result.is_none() {
            tracing::error!(
                "Couldn't find LpWindow for {self:?}. mapped={}. caller={}",
                self.is_mapped(),
                std::panic::Location::caller(),
            );
        }

        result
    }

    #[track_caller]
    fn window_show_toast(&self, text: &str, priority: adw::ToastPriority) {
        self.window_inspect(|w| w.show_toast(text, priority));
    }

    #[track_caller]
    fn window_add_toast(&self, toast: adw::Toast) {
        self.window_inspect(|w| w.add_toast(toast));
    }

    #[track_caller]
    fn window_inspect(&self, f: impl FnOnce(&LpWindow)) {
        self.try_window().inspect(f);
    }

    #[track_caller]
    fn window_show_error(&self, stub: &str, details: &str, error_type: super::ErrorType) {
        self.window_inspect(|w| w.show_error(stub, details, error_type));
    }

    fn edit_window(&self) -> LpEditWindow {
        let parent = self.parent().unwrap();

        parent
            .clone()
            .downcast()
            .unwrap_or_else(|_| parent.edit_window())
    }
}

impl<T: WidgetExt> ParentWindow for T {}
