// Copyright (c) 2023 Sophie Herold
// Copyright (c) 2023 FineFindus
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

//! A widget that shows an overlay when dragging an image over the window
//!
//! This implementation is inspired by [Amberol](https://gitlab.gnome.org/World/amberol)

use adw::prelude::*;

use crate::deps::*;

mod imp {
    use adw::subclass::prelude::*;
    use glib::{ParamSpec, Properties, Value};
    use once_cell::sync::OnceCell;

    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::LpDragOverlay)]
    pub struct LpDragOverlay {
        /// Usual content
        #[property(set = Self::set_child)]
        pub child: Option<gtk::Widget>,
        /// Widget overplayed when dragging over child
        #[property(set = Self::set_overlayed)]
        pub overlayed: Option<gtk::Widget>,
        pub overlay: gtk::Overlay,
        pub revealer: gtk::Revealer,
        #[property(set = Self::set_drop_target, get, explicit_notify, construct_only)]
        pub drop_target: OnceCell<gtk::DropTarget>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpDragOverlay {
        const NAME: &'static str = "LpDragOverlay";
        type Type = super::LpDragOverlay;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("lpdragoverlay");
        }
    }

    impl ObjectImpl for LpDragOverlay {
        fn properties() -> &'static [ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &Value, pspec: &ParamSpec) {
            self.derived_set_property(id, value, pspec)
        }

        fn property(&self, id: usize, pspec: &ParamSpec) -> Value {
            self.derived_property(id, pspec)
        }

        fn constructed(&self) {
            self.overlay.set_parent(&*self.obj());
            self.overlay.add_overlay(&self.revealer);

            self.revealer.set_can_target(false);
            self.revealer
                .set_transition_type(gtk::RevealerTransitionType::Crossfade);
            self.revealer.set_reveal_child(false);
        }

        fn dispose(&self) {
            self.overlay.unparent();
        }
    }
    impl WidgetImpl for LpDragOverlay {}
    impl BinImpl for LpDragOverlay {}

    impl LpDragOverlay {
        pub fn set_child(&self, child: Option<gtk::Widget>) {
            self.overlay.set_child(child.as_ref());
        }

        pub fn set_overlayed(&self, overlayed: Option<gtk::Widget>) {
            self.revealer.set_child(overlayed.as_ref());
        }

        pub fn set_drop_target(&self, drop_target: gtk::DropTarget) {
            drop_target.connect_current_drop_notify(
                glib::clone!(@weak self.revealer as revealer => move |target| {
                    let reveal = target.current_drop().is_some();
                    revealer.set_reveal_child(reveal);
                }),
            );

            self.drop_target.set(drop_target).unwrap();

            self.obj().notify("drop-target");
        }
    }
}

glib::wrapper! {
    pub struct LpDragOverlay(ObjectSubclass<imp::LpDragOverlay>)
        @extends gtk::Widget, adw::Bin;
}
