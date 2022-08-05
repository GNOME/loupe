// properties_view.rs
//
// Copyright $year Christopher Davis <christopherdavis@gnome.org>
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

use crate::deps::*;
use crate::util;

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::clone;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;

use anyhow::Context;
use ashpd::desktop::open_uri;
use ashpd::WindowIdentifier;
use futures::future::{AbortHandle, Abortable};
use once_cell::sync::Lazy;
use std::cell::RefCell;

use gtk_macros::spawn;

const FALLBACK: &str = "-";

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/org/gnome/Loupe/gtk/properties_view.ui")]
    pub struct LpPropertiesView {
        pub file: RefCell<Option<glib::WeakRef<gio::File>>>,
        pub file_info: RefCell<Option<gio::FileInfo>>,
        pub dimensions: RefCell<Option<String>>,

        pub info_handle: RefCell<Option<AbortHandle>>,
        pub dimensions_handle: RefCell<Option<AbortHandle>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpPropertiesView {
        const NAME: &'static str = "LpPropertiesView";
        type Type = super::LpPropertiesView;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::Type::bind_template_callbacks(klass);

            klass.install_action("properties.open-folder", None, move |properties, _, _| {
                properties.open_directory();
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for LpPropertiesView {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecObject::new(
                        "file",
                        "",
                        "",
                        gio::File::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecObject::new(
                        "file-info",
                        "",
                        "",
                        gio::FileInfo::static_type(),
                        glib::ParamFlags::READABLE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecString::new(
                        "dimensions",
                        "",
                        "",
                        Some(FALLBACK),
                        glib::ParamFlags::READABLE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let prop_name = pspec.name();

            match prop_name {
                "file" => obj.file().to_value(),
                "file-info" => obj.file_info().to_value(),
                "dimensions" => self.dimensions.borrow().to_value(),
                _ => unimplemented!("Failed to get property \"{prop_name}\""),
            }
        }

        fn set_property(
            &self,
            obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
            let prop_name = pspec.name();

            match prop_name {
                "file" => obj.set_file(&value.get().ok()),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);
            obj.action_set_enabled("properties.open-folder", false);
        }
    }

    impl WidgetImpl for LpPropertiesView {}
    impl BinImpl for LpPropertiesView {}
}

glib::wrapper! {
    pub struct LpPropertiesView(ObjectSubclass<imp::LpPropertiesView>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

#[gtk::template_callbacks]
impl LpPropertiesView {
    fn set_file(&self, file: &Option<gio::File>) {
        let imp = self.imp();

        self.action_set_enabled("properties.open-folder", file.is_some());

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

            if let Some(handle) = imp.dimensions_handle.take() {
                handle.abort();
            }

            self.build_file_info(file);
            let weak = file.downgrade();
            imp.file.replace(Some(weak));
            self.notify("file");
        }
    }

    fn file(&self) -> Option<gio::File> {
        let imp = self.imp();
        imp.file.borrow().as_ref().and_then(|w| w.upgrade())
    }

    fn file_info(&self) -> Option<gio::FileInfo> {
        let imp = self.imp();
        imp.file_info.borrow().clone()
    }

    fn build_file_info(&self, file: &gio::File) {
        let imp = self.imp();

        // We need to be able to cancel this future so that
        // changing files before the metadata loads does not cause
        // old metadata to be loaded. In order to make a cancellable
        // future, we use `AbortHandle` and `Abortable` from the `futures`
        // crate.
        //
        // `handle` is the `AbortHandle`, which we need to store in order
        // to abort the future. `reg` is the `AbortRegistration` that we
        // give to `Abortable::new()` to tie our future to the `AbortHandle`.
        //
        // Here we build the future:
        let (handle, reg) = AbortHandle::new_pair();
        let fut = Abortable::new(
            clone!(@weak self as view, @weak file => async move {
                if let Ok(info) = util::query_attributes_future(
                    &file,
                    vec![
                        &gio::FILE_ATTRIBUTE_STANDARD_CONTENT_TYPE,
                        &gio::FILE_ATTRIBUTE_STANDARD_SIZE,
                        &gio::FILE_ATTRIBUTE_TIME_CREATED,
                        &gio::FILE_ATTRIBUTE_TIME_MODIFIED,
                    ],
                )
                .await {
                    view.on_info_loaded(info, &file);
                }
            }),
            reg,
        );

        // ...store the handle:
        imp.info_handle.replace(Some(handle));

        // ...then spawn the future.
        spawn!(async {
            let _ = fut.await;
        });
    }

    // This is where we handle the results of the future.
    // This could technically be part of the future, but we keep
    // them separate for a bit of cleanliness.
    fn on_info_loaded(&self, info: gio::FileInfo, file: &gio::File) {
        let imp = self.imp();
        self.build_dimensions(&info, file);
        imp.file_info.replace(Some(info));
        self.notify("file-info");
    }

    fn build_dimensions(&self, info: &gio::FileInfo, file: &gio::File) {
        let imp = self.imp();

        // Here we do the same thing we do for the rest of the file info.
        let (handle, reg) = AbortHandle::new_pair();
        let fut = Abortable::new(
            clone!(@weak self as view, @weak file, @weak info => async move {
                let is_svg = info.content_type()
                    .map(|t| t.to_string())
                    .map(|t| t == "image/svg+xml")
                    .unwrap_or_default();

                let path = file.peek_path();
                if path.is_none() {
                    return;
                }

                let dimensions = if is_svg {
                    parse_svg_file_future(path.unwrap())
                        .await
                        .ok()
                        .and_then(|m| {
                            match (m.width(), m.height()) {
                                (Some(w), Some(h)) => Some((w, h)),
                                _ => None::<(f64, f64)>,
                            }
                        })
                        .map(|(w, h)| format!("{w:.1} \u{D7} {h:.1}"))
                } else {
                    gdk_pixbuf::Pixbuf::file_info_future(path.unwrap())
                        .await
                        .ok()
                        .flatten()
                        .map(|(_, w, h)| format!("{w} \u{D7} {h}"))
                };

                view.on_dimensions_loaded(dimensions.unwrap_or_else(|| FALLBACK.to_string()));
            }),
            reg,
        );

        imp.dimensions_handle.replace(Some(handle));

        spawn!(async {
            let _ = fut.await;
        });
    }

    fn on_dimensions_loaded(&self, dimensions: String) {
        let imp = self.imp();
        imp.dimensions.replace(Some(dimensions));
        self.notify("dimensions");
    }

    fn open_directory(&self) {
        if let Some(directory) = self
            .file()
            .and_then(|f| f.peek_path())
            .and_then(|p| std::fs::File::open(p).ok())
        {
            spawn!(clone!(@weak self as view => async move {
                let id = WindowIdentifier::from_native(&view.native().expect("No GtkNative for view")).await;
                if let Err(e) = open_uri::open_directory(&id, &directory).await {
                    log::error!("Could not open parent directory: {e}");
                };
            }));
        }
    }

    // In the LpPropertiesView UI file we define a few `gtk::Expression`s
    // that are closures. These closures take either the current `gio::File`
    // or the current file's associated `gio::FileInfo` and process them
    // accordingly.
    //
    // In this function we chain `Option`s with `and_then()` in order
    // to handle optional results with a fallback, without needing to
    // have multiple `match` or `if let` branches, and without needing
    // to unwrap.
    #[template_callback]
    fn folder_name(&self, file: Option<gio::File>) -> String {
        file.and_then(|f| f.parent()) // If the file exists, get the parent
            .and_then(|p| util::get_file_display_name(&p)) // if that exists, the display name
            .unwrap_or_else(|| FALLBACK.to_owned()) // and if we get nothing, use `FALLBACK`
    }

    #[template_callback]
    fn file_type(&self, info: Option<gio::FileInfo>) -> String {
        info.and_then(|i| i.content_type())
            .map(|t| t.to_string())
            .map(|t| {
                t.rsplit('/').collect::<Vec<&str>>()[0]
                    .to_owned()
                    .to_uppercase()
            })
            .unwrap_or_else(|| FALLBACK.to_owned())
    }

    #[template_callback]
    fn file_size(&self, info: Option<gio::FileInfo>) -> String {
        info.map(|info| {
            let size = info.size() as u64;
            glib::format_size(size).to_string()
        })
        .unwrap_or_else(|| FALLBACK.to_owned())
    }

    #[template_callback]
    fn created_date(&self, info: Option<gio::FileInfo>) -> String {
        info.and_then(|i| i.creation_date_time())
            .and_then(|t| t.format("%x, %X").ok())
            .map(|t| t.to_string())
            .unwrap_or_else(|| FALLBACK.to_owned())
    }

    #[template_callback]
    fn modified_date(&self, info: Option<gio::FileInfo>) -> String {
        info.and_then(|i| i.modification_date_time())
            .and_then(|t| t.format("%x, %X").ok())
            .map(|t| t.to_string())
            .unwrap_or_else(|| FALLBACK.to_owned())
    }
}

async fn parse_svg_file_future(path: std::path::PathBuf) -> anyhow::Result<svg_metadata::Metadata> {
    let (sender, receiver) = futures_channel::oneshot::channel();

    let _ = std::thread::Builder::new()
        .name("Parse SVG".to_string())
        .spawn(move || {
            let result = svg_metadata::Metadata::parse_file(path);
            let _ = sender.send(result);
        });

    receiver.await.unwrap().context("Could not load metadata")
}
