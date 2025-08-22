// Copyright (c) 2022-2024 Sophie Herold
// Copyright (c) 2022 Christopher Davis
// Copyright (c) 2024 Lukáš Tyrychtr
// Copyright (c) 2024 Fina Wilke
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

use std::cell::{OnceCell, RefCell};

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::translate::IntoGlib;
use glib::Properties;
use gtk::CompositeTemplate;

use crate::deps::*;
use crate::util::gettext::*;
use crate::util::{self};
use crate::widgets::image::LpImage;

const FALLBACK: &str = "–";

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate, Properties)]
    #[properties(wrapper_type = super::LpPropertiesView)]
    #[template(file = "properties_view.ui")]
    pub struct LpPropertiesView {
        #[property(nullable, set = Self::set_image, get)]
        image: RefCell<Option<LpImage>>,
        image_signals: OnceCell<glib::SignalGroup>,

        #[template_child]
        folder: TemplateChild<adw::ActionRow>,
        #[template_child]
        folder_button: TemplateChild<gtk::Button>,
        #[template_child]
        uri: TemplateChild<adw::ActionRow>,

        #[template_child]
        image_size: TemplateChild<adw::ActionRow>,
        #[template_child]
        image_format: TemplateChild<adw::ActionRow>,
        #[template_child]
        file_size: TemplateChild<adw::ActionRow>,

        #[template_child]
        dates: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        file_created: TemplateChild<adw::ActionRow>,
        #[template_child]
        file_modified: TemplateChild<adw::ActionRow>,

        #[template_child]
        details: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        location: TemplateChild<adw::ActionRow>,
        #[template_child]
        originally_created: TemplateChild<adw::ActionRow>,
        #[template_child]
        user_comment: TemplateChild<adw::ActionRow>,
        #[template_child]
        aperture: TemplateChild<adw::ActionRow>,
        #[template_child]
        exposure: TemplateChild<adw::ActionRow>,
        #[template_child]
        iso: TemplateChild<adw::ActionRow>,
        #[template_child]
        focal_length: TemplateChild<adw::ActionRow>,
        #[template_child]
        maker_model: TemplateChild<adw::ActionRow>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpPropertiesView {
        const NAME: &'static str = "LpPropertiesView";
        type Type = super::LpPropertiesView;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);

            klass.install_action_async("properties.open-folder", None, |obj, _, _| async move {
                obj.imp().open_directory().await;
            });

            klass.install_action("properties.copy-location", None, move |obj, _, _| {
                obj.imp().copy_location();
            });

            klass.install_action("properties.open-location", None, move |obj, _, _| {
                obj.imp().open_location();
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for LpPropertiesView {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            let image_signals = self.image_signals();

            image_signals.connect_notify_local(
                Some("image-size-available"),
                glib::clone!(
                    #[weak]
                    obj,
                    move |_, _| obj.imp().update()
                ),
            );

            image_signals.connect_local(
                "metadata-changed",
                false,
                glib::clone!(
                    #[weak]
                    obj,
                    #[upgrade_or_default]
                    move |_| {
                        obj.imp().update();
                        None
                    }
                ),
            );

            self.folder_button
                .reset_relation(gtk::AccessibleRelation::LabelledBy);
            self.folder_button
                .reset_relation(gtk::AccessibleRelation::DescribedBy);

            self.update();
        }

        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec)
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }
    }

    impl WidgetImpl for LpPropertiesView {}
    impl BinImpl for LpPropertiesView {}

    impl LpPropertiesView {
        fn set_image(&self, image: Option<LpImage>) {
            self.image.replace(image.clone());
            self.image_signals().set_target(image.as_ref());
            self.update();
        }

        fn image_signals(&self) -> &glib::SignalGroup {
            self.image_signals
                .get_or_init(glib::SignalGroup::new::<LpImage>)
        }

        fn update(&self) {
            let obj = self.obj();

            let Some(image) = obj.image() else {
                return;
            };
            let Some(file) = image.file() else {
                return;
            };

            let metadata = image.metadata();

            // Folder
            let parent = metadata
                .host_path()
                .or_else(|| file.path())
                .as_ref()
                .and_then(|x| x.parent())
                .and_then(|p| util::get_file_display_name(&gio::File::for_path(p)));
            let has_folder = Self::update_row(&self.folder, parent);
            // The portal only supports opening folders of files that have a path
            obj.action_set_enabled("properties.open-folder", file.path().is_some());
            // Only show URI if now folder available
            let uri = if !has_folder {
                image.file().map(|x| x.uri())
            } else {
                None
            };
            Self::update_row(&self.uri, uri);

            // Image info
            Self::update_row(&self.image_size, self.image_size());
            Self::update_row(&self.image_format, self.format_name());
            Self::update_row(&self.file_size, metadata.file_size());

            // Dates
            let has_dates = [
                Self::update_row(&self.file_created, metadata.file_created()),
                Self::update_row(&self.file_modified, metadata.file_modified()),
            ]
            .into_iter()
            .any(|x| x);
            self.dates.set_visible(has_dates);

            // Details (EXIF)
            let has_details = [
                Self::update_row(&self.location, metadata.gps_location_place()),
                Self::update_row(&self.originally_created, metadata.originally_created()),
                Self::update_row(&self.user_comment, metadata.user_comment()),
                Self::update_row(&self.aperture, metadata.f_number()),
                Self::update_row(&self.exposure, metadata.exposure_time()),
                Self::update_row(&self.iso, metadata.iso()),
                Self::update_row(&self.focal_length, metadata.focal_length()),
                Self::update_row(&self.maker_model, metadata.maker_model()),
            ]
            .iter()
            .any(|x| *x);
            self.details.set_visible(has_details);
        }

        fn update_row(row: &adw::ActionRow, value: Option<impl AsRef<str>>) -> bool {
            if let Some(value) = value {
                row.set_subtitle(value.as_ref());
                row.set_visible(true);
                true
            } else {
                row.set_subtitle(FALLBACK);
                row.set_visible(false);
                false
            }
        }

        fn format_name(&self) -> Option<String> {
            let image = self.image.borrow();
            let metadata = image.as_ref()?.metadata();

            let mut format = metadata.format_name().unwrap_or(gettext("Unknown"));

            if metadata.alpha_channel().unwrap_or(false) {
                // Translators: Addition of image being transparent to format name
                format.push_str(&gettext(", transparent"));
            }

            if metadata.grayscale().unwrap_or(false) {
                // Translators: Addition of image being grayscale to format name
                format.push_str(&gettext(", grayscale"));
            }

            if let Some(bit_depth) = metadata.bit_depth() {
                // Translators: Addition of bit size to format name
                format.push_str(&gettext_f(r", {}\u{202F}bit", [bit_depth.to_string()]));
            }

            Some(format)
        }

        fn image_size(&self) -> Option<String> {
            let obj = self.obj();

            if let Some(image) = obj.image() {
                match image.metadata().dimensions_text() {
                    Some(string) => Some(string.to_string()),
                    _ => {
                        let (width, height) = image.image_size();
                        if width > 0 && height > 0 {
                            Some(
                                // Translators: Image "<width> x <height>"
                                gettext_f(
                                    r"{}\u{202F}\u{D7}\u{202F}{}",
                                    [width.to_string(), height.to_string()],
                                ),
                            )
                        } else {
                            None
                        }
                    }
                }
            } else {
                None
            }
        }

        fn file(&self) -> Option<gio::File> {
            self.image.borrow().as_ref().and_then(|x| x.file())
        }

        async fn open_directory(&self) {
            let obj = self.obj();

            let launcher = gtk::FileLauncher::new(self.file().as_ref());
            let win = obj.native().and_downcast::<gtk::Window>();
            if let Err(e) = launcher.open_containing_folder_future(win.as_ref()).await {
                log::error!("Could not open parent directory: {e}");
            };
        }

        /// Copy GPS location
        fn copy_location(&self) {
            let obj = self.obj();

            if let Some(location) = obj.image().and_then(|x| x.metadata().gps_location()) {
                let clipboard = obj.display().clipboard();
                clipboard.set_text(&location.iso_6709());
                let _ = obj.activate_action(
                    "win.show-toast",
                    Some(
                        &(
                            gettext("Image Coordinates Copied"),
                            adw::ToastPriority::Normal.into_glib(),
                        )
                            .to_variant(),
                    ),
                );
            }
        }

        /// Open GPS location in apps like Maps via `geo:` URI
        fn open_location(&self) {
            let obj = self.obj();

            if let Some(uri) = obj
                .image()
                .and_then(|x| x.metadata().gps_location())
                .map(|x| x.geo_uri())
            {
                gio::AppInfo::launch_default_for_uri_async(
                    &uri,
                    gio::AppLaunchContext::NONE,
                    gio::Cancellable::NONE,
                    |result| {
                        if let Err(err) = result {
                            log::error!("Failed to show geo URI: {err}")
                        }
                    },
                );
            }
        }
    }
}

glib::wrapper! {
    pub struct LpPropertiesView(ObjectSubclass<imp::LpPropertiesView>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable, gtk::Accessible;
}
