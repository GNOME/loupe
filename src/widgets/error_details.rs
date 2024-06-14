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

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::CompositeTemplate;

use crate::deps::*;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(file = "error_details.ui")]
    pub struct LpErrorDetails {
        #[template_child]
        pub(super) message: TemplateChild<gtk::TextView>,
        #[template_child]
        pub(super) copy: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) report: TemplateChild<gtk::Button>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpErrorDetails {
        const NAME: &'static str = "LpErrorDetails";
        type Type = super::LpErrorDetails;
        type ParentType = adw::Dialog;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for LpErrorDetails {
        fn constructed(&self) {
            self.message.buffer().set_text("Some test text");
        }
    }
    impl WidgetImpl for LpErrorDetails {}
    impl AdwDialogImpl for LpErrorDetails {}
}

glib::wrapper! {
    pub struct LpErrorDetails(ObjectSubclass<imp::LpErrorDetails>)
        @extends adw::Dialog, gtk::Widget;
}

impl LpErrorDetails {
    pub fn new(root: &impl IsA<gtk::Widget>, text: &str) -> Self {
        let obj = glib::Object::new::<Self>();
        let imp = obj.imp();
        imp.message.buffer().set_text(text);

        imp.copy
            .connect_clicked(glib::clone!(@weak obj => move |_| {
                let buffer = obj.imp().message.buffer();
                let (start, end) = buffer.bounds();
                obj.display()
                    .clipboard()
                    .set_text(buffer.text(&start, &end, true).as_str())
            }));

        imp.report
            .connect_clicked(glib::clone!(@weak obj => move |_| {
                gtk::UriLauncher::new("https://gitlab.gnome.org/sophie-h/glycin/-/issues").launch(
                    obj.root().and_downcast_ref::<gtk::Window>(),
                    gio::Cancellable::NONE,
                    |_| {},
                );
            }));

        obj.present(Some(root));
        obj
    }
}
