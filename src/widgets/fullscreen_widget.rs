// Copyright (c) 2024 Christopher Davis
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

use std::cell::{Cell, RefCell};

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::Properties;
use gtk::CompositeTemplate;

use super::*;
use crate::deps::*;

mod imp {
    use super::*;

    #[derive(Default, Debug, Properties, CompositeTemplate)]
    #[properties(wrapper_type = super::LpFullscreenWidget)]
    #[template(file = "fullscreen_widget.ui")]
    pub struct LpFullscreenWidget {
        #[template_child]
        pub(super) toolbar_view: TemplateChild<adw::ToolbarView>,
        #[template_child]
        pub(super) header_container: TemplateChild<LpShyBin>,

        #[property(get, set, nullable)]
        headerbar: RefCell<Option<gtk::Widget>>,
        #[property(get, set, nullable)]
        content: RefCell<Option<gtk::Widget>>,

        #[property(get, set)]
        fullscreened: Cell<bool>,
        #[property(get, set)]
        extend_content: Cell<bool>,
        #[property(get = LpFullscreenWidget::is_content_extended_to_top, type = bool)]
        _is_content_extended_to_top: (),
        #[property(get, set)]
        is_empty: Cell<bool>,
        #[property(get, set)]
        headerbar_opacity: Cell<f64>,
    }

    impl LpFullscreenWidget {
        fn is_content_extended_to_top(&self) -> bool {
            let obj = self.obj();
            obj.fullscreened() && obj.extend_content()
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpFullscreenWidget {
        const NAME: &'static str = "LpFullscreenWidget";
        type Type = super::LpFullscreenWidget;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &gio::glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for LpFullscreenWidget {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();
            obj.connect_is_empty_notify(|obj| {
                obj.update_headerbar_style();
            });

            obj.connect_fullscreened_notify(|obj| {
                obj.update_headerbar_style();
            });

            obj.connect_extend_content_notify(|obj| {
                obj.update_headerbar_style();
            });
        }
    }

    impl WidgetImpl for LpFullscreenWidget {}
    impl BinImpl for LpFullscreenWidget {}
}

glib::wrapper! {
    pub struct LpFullscreenWidget(ObjectSubclass<imp::LpFullscreenWidget>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::Buildable, gtk::ConstraintTarget;
}

impl LpFullscreenWidget {
    pub fn is_headerbar_flat(&self) -> bool {
        matches!(
            self.imp().toolbar_view.top_bar_style(),
            adw::ToolbarStyle::Flat
        )
    }

    fn update_header_opacity(&self) {
        if let Some(headerbar) = self.headerbar() {
            if self.is_headerbar_flat() && !self.is_empty() {
                // Bring headerbar opacity in sync with controls
                headerbar.set_opacity(self.headerbar_opacity());
            } else {
                headerbar.set_opacity(1.);
            };
        }
    }

    fn update_headerbar_style(&self) {
        self.imp()
            .toolbar_view
            .set_extend_content_to_top_edge(self.is_content_extended_to_top());

        // Flat headerbar for empty state and fullscreen
        let style = if self.is_empty() || self.is_content_extended_to_top() {
            adw::ToolbarStyle::Flat
        } else {
            // Use the border variant of raised to avoid shadows over images
            adw::ToolbarStyle::RaisedBorder
        };

        let toolbar_view = self.imp().toolbar_view.get();
        if style != toolbar_view.top_bar_style() {
            toolbar_view.set_top_bar_style(style);
            self.update_header_opacity();
        }
    }
}
