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
//
// Copyright (c) 2024 Christopher Davis

//! Contains the main structure inside the window
//!
//! This is it's own widget since it's reused inside different layouts for
//! different positions of the image properties.

use std::cell::{Cell, RefCell};

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::Properties;
use gtk::CompositeTemplate;

use crate::deps::*;

mod imp {
    use super::*;

    #[derive(Default, Debug, Properties, CompositeTemplate)]
    #[properties(wrapper_type = super::LpWindowContent)]
    #[template(file = "window_content.ui")]
    pub struct LpWindowContent {
        #[template_child]
        pub(super) toolbar_view: TemplateChild<adw::ToolbarView>,

        #[property(get, set, nullable)]
        headerbar: RefCell<Option<gtk::Widget>>,
        #[property(get, set, nullable)]
        content: RefCell<Option<gtk::Widget>>,

        // Set via binding to window
        #[property(get, set)]
        fullscreened: Cell<bool>,
        // Set via binding to properties toggle button
        #[property(get, set)]
        show_properties: Cell<bool>,
        // Set via binding to window
        #[property(get, set)]
        is_showing_image: Cell<bool>,
    }

    impl LpWindowContent {
        fn update_header_opacity(&self) {
            let obj = self.obj();

            if let Some(headerbar) = obj.headerbar() {
                if obj.is_headerbar_flat() && obj.is_showing_image() {
                    // Bring headerbar opacity in sync with controls
                    headerbar.set_opacity(self.headerbar_opacity());
                } else {
                    headerbar.set_opacity(1.);
                };
            }
        }

        fn update_headerbar_style(&self) {
            let obj = self.obj();

            self.toolbar_view
                .set_extend_content_to_top_edge(obj.is_content_extended_to_top());

            let style = if !obj.is_showing_image() || obj.is_content_extended_to_top() {
                // Flat headerbar for empty state and fullscreen without properties enabled
                adw::ToolbarStyle::Flat
            } else {
                // Use the border variant of raised to avoid shadows over images
                adw::ToolbarStyle::RaisedBorder
            };

            let toolbar_view = self.toolbar_view.get();
            if style != toolbar_view.top_bar_style() {
                toolbar_view.set_top_bar_style(style);
                self.update_header_opacity();
            }
        }

        fn headerbar_opacity(&self) -> f64 {
            self.headerbar
                .borrow()
                .as_ref()
                .map(|x| x.opacity())
                .unwrap_or(1.)
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpWindowContent {
        const NAME: &'static str = "LpWindowContent";
        type Type = super::LpWindowContent;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &gio::glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for LpWindowContent {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.connect_is_showing_image_notify(|obj| {
                obj.imp().update_headerbar_style();
            });

            obj.connect_fullscreened_notify(|obj| {
                obj.imp().update_headerbar_style();
            });

            obj.connect_show_properties_notify(|obj| {
                obj.imp().update_headerbar_style();
            });
        }
    }

    impl WidgetImpl for LpWindowContent {}
    impl BinImpl for LpWindowContent {}
}

glib::wrapper! {
    pub struct LpWindowContent(ObjectSubclass<imp::LpWindowContent>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::Buildable, gtk::ConstraintTarget;
}

impl LpWindowContent {
    pub fn is_headerbar_flat(&self) -> bool {
        matches!(
            self.imp().toolbar_view.top_bar_style(),
            adw::ToolbarStyle::Flat
        )
    }

    pub fn is_content_extended_to_top(&self) -> bool {
        self.fullscreened() && self.show_properties()
    }
}
