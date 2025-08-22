// Copyright (c) 2023-2024 Sophie Herold
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

use std::cell::OnceCell;
use std::marker::PhantomData;

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::Properties;
use gtk::CompositeTemplate;

use crate::deps::*;

mod imp {

    use super::*;

    #[derive(Debug, Default, CompositeTemplate, Properties)]
    #[properties(wrapper_type = super::LpDragOverlay)]
    #[template(file = "drag_overlay.ui")]
    pub struct LpDragOverlay {
        /// Widget overplayed when dragging over child
        #[property(set = Self::set_content)]
        pub content: PhantomData<Option<gtk::Widget>>,
        #[template_child]
        pub overlay: TemplateChild<gtk::Overlay>,
        #[template_child]
        pub revealer: TemplateChild<gtk::Revealer>,
        #[property(set = Self::set_drop_target, get, explicit_notify, construct_only)]
        pub drop_target: OnceCell<gtk::DropTarget>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpDragOverlay {
        const NAME: &'static str = "LpDragOverlay";
        type Type = super::LpDragOverlay;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            klass.set_css_name("lpdragoverlay");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for LpDragOverlay {
        fn constructed(&self) {
            self.parent_constructed();

            self.revealer.set_can_target(false);
            self.revealer
                .set_transition_type(gtk::RevealerTransitionType::Crossfade);
            //self.revealer.set_reveal_child(false);
        }
    }
    impl WidgetImpl for LpDragOverlay {}
    impl BinImpl for LpDragOverlay {}

    impl LpDragOverlay {
        pub fn set_drop_target(&self, drop_target: gtk::DropTarget) {
            drop_target.connect_current_drop_notify(glib::clone!(
                #[weak(rename_to = revealer)]
                self.revealer,
                move |target| {
                    let reveal = target.current_drop().is_some();
                    revealer.set_reveal_child(reveal);
                }
            ));

            self.drop_target.set(drop_target).unwrap();

            self.obj().notify("drop-target");
        }

        pub fn set_content(&self, child: Option<gtk::Widget>) {
            self.overlay.set_child(child.as_ref());
        }
    }
}

glib::wrapper! {
    pub struct LpDragOverlay(ObjectSubclass<imp::LpDragOverlay>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::Buildable, gtk::Accessible, gtk::ConstraintTarget;
}
