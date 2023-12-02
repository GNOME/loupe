// Copyright (c) 2022-2023 Sophie Herold
// Copyright (c) 2022 Christopher Davis
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

use crate::decoder::ImageDimensionDetails;
use crate::deps::*;
use crate::util;

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::Properties;
use gtk::CompositeTemplate;

use std::cell::{OnceCell, RefCell};

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
        image_size: TemplateChild<adw::ActionRow>,
        #[template_child]
        image_format: TemplateChild<adw::ActionRow>,
        #[template_child]
        file_size: TemplateChild<adw::ActionRow>,

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
                glib::clone!(@weak obj => move |_,_| obj.imp().update()),
            );
            image_signals.connect_notify_local(
                Some("metadata"),
                glib::clone!(@weak obj => move |_,_| obj.imp().update()),
            );

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

            // Folder
            Self::update_row(&self.folder, self.folder_name());
            obj.action_set_enabled("properties.open-folder", file.path().is_some());

            // Image info
            Self::update_row(&self.image_size, self.image_size());
            Self::update_row(&self.image_format, image.format().map(|x| x.to_string()));
            //Self::update_row(&self.file_size, self.image_size());

            // Sizes

            // Details
            let metadata = image.metadata();
            let meta = metadata.src();
            let has_details = [
                Self::update_row(&self.location, meta.gps_location().map(|x| x.display())),
                Self::update_row(&self.originally_created, meta.originally_created()),
                Self::update_row(&self.aperture, meta.f_number()),
                Self::update_row(&self.exposure, meta.exposure_time()),
                Self::update_row(&self.iso, meta.iso()),
                Self::update_row(&self.focal_length, meta.focal_length()),
                Self::update_row(&self.maker_model, meta.maker_model()),
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

        fn folder_name(&self) -> Option<String> {
            self.file()
                .and_then(|f| f.parent())
                .and_then(|p| util::get_file_display_name(&p))
        }

        fn image_size(&self) -> Option<String> {
            let obj = self.obj();

            if let Some(image) = obj.image() {
                match image.dimension_details() {
                    ImageDimensionDetails::Svg(string) => Some(string),
                    _ => {
                        let (width, height) = image.image_size();
                        if width > 0 && height > 0 {
                            return Some(format!("{width}\u{202F}\u{D7}\u{202F}{height}"));
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
        @implements gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl LpPropertiesView {
    /*    fn set_file(&self, file: Option<&gio::File>) {
        let imp = self.imp();

        self.action_set_enabled(
            "properties.open-folder",
            file.map_or(false, |x| x.is_native()),
        );

        if let Some(file) = file {
            if let Some(current_file) = self.file() {
                if current_file.equal(file) {
                    return;
                }
            }

            // Cancel the file info future. See `build_file_info()` for details.
            if let Some(handle) = imp.info_handle.take() {
                handle.abort();
            }

            self.build_file_info(file);
        }

        let weak = file.map(|x| x.downgrade());
        imp.file.replace(weak);
        self.notify("file");
    }
     */
}
