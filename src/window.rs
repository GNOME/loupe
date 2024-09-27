// Copyright (c) 2020-2024 Christopher Davis
// Copyright (c) 2022-2024 Sophie Herold
// Copyright (c) 2022 Elton A Rodrigues
// Copyright (c) 2022 Maximiliano Sandoval R
// Copyright (c) 2023 Matteo Nardi
// Copyright (c) 2023 FineFindus
// Copyright (c) 2023 qwel
// Copyright (c) 2023 Huan Thieu Nguyen
// Copyright (c) 2024 Fina Wilke
// Copyright (c) 2024 DaPigGuy
// Copyright (c) 2024 James Frost
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

mod actions;
mod controls;

use std::cell::{Cell, OnceCell, RefCell};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use actions::*;
use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::{clone, Properties};
use gtk::{CompositeTemplate, Widget};

use crate::application::LpApplication;
use crate::config;
use crate::deps::*;
use crate::util::gettext::*;
use crate::util::Direction;
use crate::widgets::{LpDragOverlay, LpImage, LpImageView, LpPropertiesView, LpWindowContent};

/// Show window after X milliseconds even if image dimensions are not known yet
const SHOW_WINDOW_AFTER: u64 = 2000;

/// Animation duration for showing overlay buttons in milliseconds
const SHOW_CONTROLS_ANIMATION_DURATION: u32 = 200;
/// Animation duration for hiding overlay buttons in milliseconds
const HIDE_CONTROLS_ANIMATION_DURATION: u32 = 1000;
/// Time of inactivity after which controls will be hidden in milliseconds
const HIDE_CONTROLS_IDLE_TIMEOUT: u64 = 3000;

mod imp {

    use gio::glib::VariantTy;

    use super::*;

    // To use composite templates, you need
    // to use derive macro. Derive macros generate
    // code to e.g. implement a trait on something.
    // In this case, code is generated for Debug output
    // and to handle binding the template children.
    //
    // For this derive macro, you need to have
    // `use gtk::CompositeTemplate` in your code.
    //
    // Because all of our member fields implement the
    // `Default` trait, we can use `#[derive(Default)]`.
    // If some member fields did not implement default,
    // we'd need to have a `new()` function in the
    // `impl ObjectSubclass for $TYPE` section.
    #[derive(Default, Debug, Properties, CompositeTemplate)]
    #[properties(wrapper_type = super::LpWindow)]
    #[template(file = "window.ui")]
    pub struct LpWindow {
        #[template_child]
        pub(super) window_content: TemplateChild<LpWindowContent>,
        #[template_child]
        pub(super) headerbar: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub(super) headerbar_events: TemplateChild<gtk::EventControllerMotion>,
        #[template_child]
        pub(super) properties_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub(super) primary_menu: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub(super) fullscreen_button: TemplateChild<gtk::Button>,

        #[template_child]
        pub(super) toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) status_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub(super) image_view: TemplateChild<LpImageView>,
        #[template_child]
        pub(super) properties_view: TemplateChild<LpPropertiesView>,

        #[template_child]
        pub(super) drag_overlay: TemplateChild<LpDragOverlay>,
        #[template_child]
        pub(super) drop_target: TemplateChild<gtk::DropTarget>,

        #[template_child]
        pub(super) forward_click_gesture: TemplateChild<gtk::GestureClick>,
        #[template_child]
        pub(super) backward_click_gesture: TemplateChild<gtk::GestureClick>,

        #[property(get = Self::is_showing_image)]
        pub(super) is_showing_image: PhantomData<bool>,
        /// Set via binding to HeaderBar
        #[property(get, set)]
        headerbar_opacity: Cell<f64>,

        /// Motion controller for complete window
        pub(super) motion_controller: gtk::EventControllerMotion,
        pub(super) pointer_position: Cell<(f64, f64)>,

        pub(super) show_controls_animation: OnceCell<adw::TimedAnimation>,
        pub(super) hide_controls_animation: OnceCell<adw::TimedAnimation>,
        pub(super) hide_controls_timeout: RefCell<Option<glib::SourceId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LpWindow {
        const NAME: &'static str = "LpWindow";
        type Type = super::LpWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            // bind_template() is a function generated by the
            // CompositeTemplate macro to bind all children at once.
            Self::bind_template(klass);

            klass.add_binding(gdk::Key::c, gdk::ModifierType::CONTROL_MASK, |window| {
                if window.has_metadata_selected() {
                    // Pass on to normal copy handler to copy selected metadata
                    glib::Propagation::Proceed
                } else {
                    window.copy_image();
                    glib::Propagation::Stop
                }
            });

            klass.add_binding_action(
                gdk::Key::question,
                gdk::ModifierType::CONTROL_MASK,
                "win.show-help-overlay",
            );

            // Set up actions

            ActionPartGlobal::init_actions_and_bindings(klass);
            Action::init_actions_and_bindings(klass);

            klass.install_action(
                "win.show-toast",
                Some(VariantTy::TUPLE),
                move |win, _, var| {
                    if let Some((ref toast, i)) = var.and_then(|v| v.get::<(String, i32)>()) {
                        win.show_toast(toast, adw::ToastPriority::__Unknown(i));
                    }
                },
            );
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for LpWindow {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            self.forward_click_gesture.connect_pressed(clone!(
                #[weak]
                obj,
                move |_, _, _, _| {
                    obj.image_view().navigate(Direction::Forward, false);
                }
            ));
            self.backward_click_gesture.connect_pressed(clone!(
                #[weak]
                obj,
                move |_, _, _, _| {
                    obj.image_view().navigate(Direction::Back, false);
                }
            ));
            self.properties_button.connect_toggled(clone!(
                #[weak]
                obj,
                move |_| {
                    obj.on_properties_button_toggled();
                }
            ));

            if config::APP_ID.ends_with(".Devel") {
                obj.add_css_class("devel");
            }

            // Limit effect of modal dialogs to this window
            // and keeps the others usable
            gtk::WindowGroup::new().add_window(&*obj);

            glib::timeout_add_local_once(
                std::time::Duration::from_millis(SHOW_WINDOW_AFTER),
                glib::clone!(
                    #[weak]
                    obj,
                    move || if !obj.is_visible() {
                        obj.present()
                    }
                ),
            );

            obj.on_current_page_changed();
            obj.on_fullscreen_changed();

            self.image_view.connect_current_page_notify(glib::clone!(
                #[weak]
                obj,
                move |_| obj.on_current_page_changed()
            ));

            obj.connect_fullscreened_notify(glib::clone!(
                #[weak]
                obj,
                move |_| obj.on_fullscreen_changed()
            ));

            obj.connect_map(|win| {
                win.resize_default();
            });

            let gesture_click = gtk::GestureClick::new();
            gesture_click.connect_pressed(glib::clone!(
                #[weak]
                obj,
                move |_, _, _, _| obj.on_click()
            ));
            obj.add_controller(gesture_click);

            self.motion_controller.connect_motion(glib::clone!(
                #[weak]
                obj,
                move |_, x, y| obj.on_motion((x, y))
            ));
            self.motion_controller.connect_enter(glib::clone!(
                #[weak]
                obj,
                move |_, x, y| obj.on_motion((x, y))
            ));
            obj.add_controller(self.motion_controller.clone());

            let current_image_signals = self.image_view.current_image_signals();

            current_image_signals.connect_local(
                "metadata-changed",
                true,
                glib::clone!(
                    #[weak]
                    obj,
                    #[upgrade_or_default]
                    move |_| {
                        obj.update_title();
                        None
                    }
                ),
            );

            current_image_signals.connect_closure(
                "notify::best-fit",
                false,
                // `closure_local!` is similar to `clone`, but you use `@watch` instead of clone.
                // `@watch` means that this signal will be disconnected when the watched object
                // is dropped.
                glib::closure_local!(
                    #[watch]
                    obj,
                    move |_: &LpImage, _: &glib::ParamSpec| {
                        obj.update_image_actions_status();
                    }
                ),
            );

            current_image_signals.connect_closure(
                "notify::is-max-zoom",
                false,
                glib::closure_local!(
                    #[watch]
                    obj,
                    move |_: &LpImage, _: &glib::ParamSpec| {
                        obj.update_image_actions_status();
                    }
                ),
            );

            current_image_signals.connect_closure(
                "notify::image-size-available",
                false,
                glib::closure_local!(
                    #[watch]
                    obj,
                    move |_: &LpImage, _: &glib::ParamSpec| {
                        obj.image_size_available();
                    }
                ),
            );

            current_image_signals.connect_closure(
                "notify::is-loaded",
                false,
                glib::closure_local!(
                    #[watch]
                    obj,
                    move |_: &LpImage, _: &glib::ParamSpec| {
                        obj.update_image_actions_status();
                    }
                ),
            );

            current_image_signals.connect_closure(
                "notify::error",
                false,
                glib::closure_local!(
                    #[watch]
                    obj,
                    move |_: &LpImage, _: &glib::ParamSpec| {
                        obj.image_error();
                        obj.update_image_actions_status();
                    }
                ),
            );

            // action win.previous status
            self.image_view
                .connect_is_previous_available_notify(glib::clone!(
                    #[weak]
                    obj,
                    move |_| {
                        obj.action_set_enabled(
                            &Action::Previous,
                            obj.imp().image_view.is_previous_available(),
                        );
                    }
                ));

            // action win.next status
            self.image_view
                .connect_is_next_available_notify(glib::clone!(
                    #[weak]
                    obj,
                    move |_| {
                        obj.action_set_enabled(
                            &Action::Next,
                            obj.imp().image_view.is_next_available(),
                        );
                    }
                ));

            // Make widgets visible when the focus moves
            obj.connect_move_focus(|obj, _| {
                obj.show_controls();
                obj.schedule_hide_controls();
            });

            obj.image_view()
                .zoom_menu_button()
                .connect_active_notify(glib::clone!(
                    #[weak]
                    obj,
                    move |button| {
                        if button.is_active() {
                            // Show overlay controls when zoom popover in them gets opened
                            obj.show_controls();
                        } else {
                            // Schedule hide overlay controls when zoom popover gets closed
                            obj.schedule_hide_controls();
                        }
                    }
                ));

            // Activate global shortcuts only if no dialog is open
            obj.connect_visible_dialog_notify(|obj| obj.update_accel_status());
            obj.connect_is_active_notify(|obj| obj.update_accel_status());

            self.status_page
                .set_icon_name(Some(&format!("{}-symbolic", config::APP_ID)));

            self.drop_target.set_types(&[gdk::FileList::static_type()]);

            self.drop_target.connect_accept(clone!(
                #[weak]
                obj,
                #[upgrade_or]
                false,
                move |_drop_target, drop| {
                    // Only accept drops from external sources or different windows
                    let different_source = drop.drag().is_none()
                        || drop.drag() != obj.image_view().drag_source().drag();
                    // We have to do this manually since we are overwriting the default handler
                    let correct_format = drop.formats().contains_type(gdk::FileList::static_type());

                    different_source && correct_format
                }
            ));

            // For callbacks, you will want to reference the GTK docs on
            // the relevant signal to see which parameters you need.
            // In this case, we need only need the GValue,
            // so we name it `value` then use `_` for the other spots.
            self.drop_target.connect_drop(clone!(
                #[weak]
                obj,
                #[upgrade_or]
                false,
                move |_, value, _, _| {
                    // Here we use a GValue, which is a dynamic object that can hold different
                    // types, e.g. strings, numbers, or in this case objects. In
                    // order to get the GdkFileList from the GValue, we need to
                    // use the `get()` method.
                    //
                    // We've added type annotations here, and written it as `let list: gdk::FileList
                    // = ...`, but you might also see places where type
                    // arguments are used. This line could have been written as
                    // `let list = value.get::<gdk::FileList>().unwrap()`.
                    let files = match value.get::<gdk::FileList>() {
                        Ok(list) => list.files(),
                        Err(err) => {
                            log::error!("Issue with drop value: {err}");
                            return false;
                        }
                    };

                    if !files.is_empty() {
                        obj.image_view().set_images_from_files(files);
                    } else {
                        log::error!("Dropped FileList was empty");
                        return false;
                    }

                    // Maybe one day this will actually work
                    obj.present();

                    true
                }
            ));
        }
    }

    impl WidgetImpl for LpWindow {}
    impl WindowImpl for LpWindow {}
    impl ApplicationWindowImpl for LpWindow {}
    impl AdwApplicationWindowImpl for LpWindow {}

    impl LpWindow {
        fn is_showing_image(&self) -> bool {
            self.image_view.try_get().map_or(false, |image_view| {
                image_view
                    .current_page()
                    .map_or(false, |page| page.image().error().is_none())
            })
        }
    }
}

glib::wrapper! {
    pub struct LpWindow(ObjectSubclass<imp::LpWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionMap, gio::ActionGroup, gtk::Native;
}

impl LpWindow {
    pub fn new<A: IsA<gtk::Application>>(app: &A) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    fn toggle_fullscreen(&self, fullscreen: bool) {
        self.set_fullscreened(fullscreen);
    }

    fn zoom_out_cursor(&self) {
        if let Some(image) = self.imp().image_view.current_image() {
            image.zoom_out_cursor();
        }
    }

    fn zoom_out_center(&self) {
        if let Some(image) = self.imp().image_view.current_image() {
            image.zoom_out_center();
        }
    }

    fn zoom_in_cursor(&self) {
        if let Some(image) = self.imp().image_view.current_image() {
            image.zoom_in_cursor();
        }
    }

    fn zoom_in_center(&self) {
        if let Some(image) = self.imp().image_view.current_image() {
            image.zoom_in_center();
        }
    }

    fn zoom_to_exact(&self, level: f64) {
        if let Some(image) = self.image_view().current_image() {
            image.zoom_to_exact(level);
        }
    }

    fn zoom_best_fit(&self) {
        if let Some(page) = self.imp().image_view.current_page() {
            page.image().zoom_best_fit();
        }
    }

    fn pan(&self, direction: &gtk::PanDirection) {
        if let Some(image) = self.imp().image_view.current_image() {
            image.pan(direction);
        }
    }

    async fn pick_file(&self) {
        let filter_list = gio::ListStore::new::<gtk::FileFilter>();

        let filter_supported_formats = gtk::FileFilter::new();
        filter_supported_formats.set_name(Some(&gettext("Supported image formats")));
        for mime_type in glycin::Loader::supported_mime_types().await {
            filter_supported_formats.add_mime_type(mime_type.as_str());
        }

        let filter_all_files = gtk::FileFilter::new();
        filter_all_files.set_name(Some(&gettext("All files")));
        filter_all_files.add_pattern("*");

        filter_list.append(&filter_supported_formats);
        filter_list.append(&filter_all_files);

        let chooser = gtk::FileDialog::builder()
            .title(gettext("Open Image"))
            .filters(&filter_list)
            .default_filter(&filter_supported_formats)
            .modal(true)
            .build();

        chooser.set_initial_folder(
            self.image_view()
                .current_image()
                .and_then(|x| x.file())
                .and_then(|x| x.parent())
                .as_ref(),
        );

        if let Ok(selected) = chooser.open_multiple_future(Some(self)).await {
            let images: Vec<_> = selected
                .into_iter()
                .filter_map(|files| {
                    files
                        .ok()
                        .and_then(|object| object.downcast::<gio::File>().ok())
                })
                .collect();
            self.image_view().set_images_from_files(images);
        } else {
            log::debug!("File dialog canceled or file not readable");
        }
    }

    async fn open_with(&self) {
        let imp = self.imp();

        if let Some(ref file) = imp.image_view.current_file() {
            let launcher = gtk::FileLauncher::new(Some(file));
            launcher.set_always_ask(true);
            if let Err(e) = launcher.launch_future(Some(self)).await {
                if !e.matches(gtk::DialogError::Dismissed) {
                    log::error!("Could not open image in external program: {}", e);
                }
            }
        } else {
            log::error!("Could not load a path for the current image.")
        }
    }

    fn rotate_image(&self, angle: f64) {
        self.imp().image_view.rotate_image(angle)
    }

    async fn set_background(&self) {
        let imp = self.imp();

        if let Err(e) = imp.image_view.set_background().await {
            log::error!("Failed to set background: {}", e);
        }
    }

    fn print(&self) {
        let imp = self.imp();

        if let Err(e) = imp.image_view.print() {
            log::error!("Failed to print file: {}", e);
        }
    }

    /// Returns true if some text in metadata is currently selected
    fn has_metadata_selected(&self) -> bool {
        if let Some(focus_widget) = self.focus() {
            if focus_widget.is_ancestor(&*self.imp().properties_view) {
                if let Ok(label) = focus_widget.downcast::<gtk::Label>() {
                    return label.selection_bounds().is_some();
                }
            }
        }

        false
    }

    /// Copy image to clipboard or metadata text if selected
    fn copy_image(&self) {
        let imp = self.imp();

        if let Err(e) = imp.image_view.copy() {
            log::error!("Failed to copy to clipboard: {}", e);
        } else {
            self.show_toast(
                gettext("Image copied to clipboard"),
                adw::ToastPriority::High,
            );
        }
    }

    async fn trash(&self) {
        let image_view = self.image_view();
        let (Some(file), Some(path)) = (
            image_view.current_file(),
            image_view.current_file().and_then(|x| x.path()),
        ) else {
            log::error!("No file to trash");
            return;
        };

        let result = file.trash_future(glib::Priority::default()).await;

        match result {
            Ok(()) => {
                let toast = adw::Toast::builder()
                    .title(gettext("Image moved to trash"))
                    .button_label(gettext("Undo"))
                    .priority(adw::ToastPriority::High)
                    .build();
                toast.connect_button_clicked(glib::clone!(
                    #[weak(rename_to = win)]
                    self,
                    move |_| {
                        let path = path.clone();
                        glib::spawn_future_local(async move {
                            win.image_view()
                                .set_trash_restore(Some(gio::File::for_path(&path)));
                            let result = crate::util::untrash(&path).await;
                            if let Err(err) = result {
                                log::error!("Failed to untrash {path:?}: {err}");
                                win.show_toast(
                                    gettext("Failed to restore image from trash"),
                                    adw::ToastPriority::High,
                                );
                            }
                        });
                    }
                ));
                self.imp().toast_overlay.add_toast(toast);
            }
            Err(err) => {
                if Some(gio::IOErrorEnum::NotSupported) == err.kind::<gio::IOErrorEnum>() {
                    self.delete_future(&path).await;
                } else {
                    log::error!("Failed to delete file {path:?}: {err}");
                    self.show_toast(
                        gettext("Failed to move image to trash"),
                        adw::ToastPriority::Normal,
                    );
                }
            }
        }
    }

    /// Delete action called
    async fn delete(&self) {
        let image_view = self.image_view();
        let Some(path) = image_view.current_file().and_then(|x| x.path()) else {
            log::error!("No file to delete");
            return;
        };

        self.delete_future(&path).await;
    }

    /// Permanently delete image
    ///
    /// Fallback for when trash not available or explicit call with shortcut
    async fn delete_future(&self, path: &Path) {
        let dialog = adw::AlertDialog::builder()
            .heading(gettext("Permanently Delete Image?"))
            .body(gettext_f(
                "After deleting the image “{}” it will be permanently lost.",
                [PathBuf::from(&path.file_name().unwrap_or_default())
                    .display()
                    .to_string()],
            ))
            .build();

        dialog.add_responses(&[
            ("cancel", &gettext("Cancel")),
            ("delete", &gettext("Delete")),
        ]);
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);

        if "delete" == dialog.choose_future(self).await {
            let file = gio::File::for_path(path);
            let result = file.delete_future(glib::Priority::default()).await;

            if let Err(err) = result {
                log::error!("Failed to delete file {path:?}: {err}");
                self.show_toast(
                    gettext("Failed to delete image"),
                    adw::ToastPriority::Normal,
                );
            }
        }
    }

    pub fn image_view(&self) -> LpImageView {
        self.imp().image_view.clone()
    }

    pub fn headerbar(&self) -> adw::HeaderBar {
        self.imp().headerbar.clone()
    }

    fn show_toast(&self, text: impl AsRef<str>, priority: adw::ToastPriority) {
        let imp = self.imp();

        let toast = adw::Toast::new(text.as_ref());
        toast.set_priority(priority);

        imp.toast_overlay.add_toast(toast);
    }

    /// Set status for actions that are related to the active image
    fn update_image_actions_status(&self) {
        // Actions that are enabled if the image is shown
        const ACTIONS_SHOWN: &[Action] = &[
            Action::SetBackground,
            Action::Print,
            Action::RotateCw,
            Action::RotateCcw,
            Action::ZoomBestFit,
            Action::ZoomToExact100,
            Action::ZoomToExact200,
            Action::ZoomToExact300,
        ];

        let enabled_shown = self
            .imp()
            .image_view
            .current_page()
            .is_some_and(|x| x.image().is_loaded() && x.image().error().is_none());

        for action in ACTIONS_SHOWN {
            self.action_set_enabled(action, enabled_shown);
        }

        // Zoom specific actions

        let can_zoom_out = self
            .image_view()
            .current_image()
            .is_some_and(|image| !image.is_best_fit());
        let can_zoom_in = self
            .image_view()
            .current_image()
            .is_some_and(|image| !image.is_max_zoom());

        self.action_set_enabled(&Action::ZoomOutCursor, can_zoom_out && enabled_shown);
        self.action_set_enabled(&Action::ZoomOutCenter, can_zoom_out && enabled_shown);
        self.action_set_enabled(&Action::ZoomInCursor, can_zoom_in && enabled_shown);
        self.action_set_enabled(&Action::ZoomInCenter, can_zoom_in && enabled_shown);

        self.image_view().set_zoom_toggle_state(
            self.image_view()
                .current_image()
                .is_some_and(|image| !image.is_best_fit()),
        );

        // Actions that are available if there is an current image, even if it's not
        // shown
        const ACTIONS_CURRENT: &[Action] = &[
            Action::OpenWith,
            Action::CopyImage,
            Action::ToggleProperties,
            Action::Reload,
            Action::ToggleFullscreen,
        ];

        let enabled_current = self.imp().image_view.current_page().is_some();

        for action in ACTIONS_CURRENT {
            self.action_set_enabled(action, enabled_current);
        }
    }

    /// Handles change in image and availability of images
    fn on_current_page_changed(&self) {
        let imp = self.imp();
        let was_showing_image =
            imp.stack.visible_child().as_ref() == Some(imp.image_view.upcast_ref::<Widget>());
        let current_page = imp.image_view.current_page();

        // HeaderBar style
        self.notify_is_showing_image();

        // Window title
        self.update_title();

        // Properties view
        let current_image = current_page.as_ref().map(|x| x.image());
        imp.properties_view.set_image(current_image.as_ref());

        let has_image = current_page.is_some();

        self.update_image_actions_status();
        self.action_set_enabled(
            &Action::Trash,
            imp.image_view
                .current_file()
                .is_some_and(|file| file.path().is_some()),
        );

        if has_image {
            // Properties buttons was not sensitive before
            if !imp.properties_button.is_sensitive() {
                let settings = LpApplication::default().settings();
                // Pickup config for setting it's state
                imp.properties_button
                    .set_active(settings.boolean("show-properties"));
            }
            if !was_showing_image {
                imp.stack.set_visible_child(&*imp.image_view);
                imp.image_view.grab_focus();
                self.show_controls();
                self.schedule_hide_controls();
            }
        } else {
            imp.properties_button.set_active(false);
            imp.stack.set_visible_child(&*imp.status_page);
            imp.status_page.grab_focus();
            // Leave fullscreen since status page has no controls to leave it
            self.set_fullscreened(false);
        }

        imp.properties_button.set_sensitive(has_image);
    }

    /// When the image-properties sidebar is displayed or hidden, we should
    /// update the "show-properties" setting.
    fn on_properties_button_toggled(&self) {
        let imp = self.imp();
        // When no image is shown, we skip this update as the sidebar should always be
        // hidden. This can happen when deleting a picture.
        if imp.image_view.current_page().is_none() {
            return;
        }
        let settings = LpApplication::default().settings();
        let result = settings.set_boolean("show-properties", imp.properties_button.is_active());
        if let Err(err) = result {
            log::warn!("Failed to save show-properties state, {}", err);
        }
    }

    pub fn update_title(&self) {
        let title = self
            .imp()
            .image_view
            .current_image()
            .and_then(|x| x.metadata().file_name())
            .unwrap_or_else(|| gettext("Image Viewer"));

        self.set_title(Some(&title));
    }

    pub fn image_size_available(&self) {
        // if visible for whatever reason, don't do any resize
        if self.is_visible() {
            return;
        }

        let image = self
            .imp()
            .image_view
            .current_page()
            .map(|page| page.image());

        if image.is_some_and(|img| img.image_size_available()) {
            log::debug!("Showing window because image size is ready");
            self.present();
        }
    }

    pub fn resize_default(&self) {
        let imp = self.imp();

        if self
            .image_view()
            .current_image()
            .is_some_and(|img| img.image_size_available())
        {
            let shows_properties = imp.properties_button.is_active();

            let (_, window_natural_width, _, _) = self.measure(gtk::Orientation::Horizontal, -1);
            let (_, window_natural_height, _, _) = self.measure(gtk::Orientation::Vertical, -1);

            // These have to be in sync with the "conditions" for the "overlay-properties"
            // breakpoint
            let min_width_for_overlay = adw::LengthUnit::Sp.to_px(590., None).ceil() as i32;
            let min_height_for_overlay = adw::LengthUnit::Sp.to_px(550., None).ceil() as i32;

            let (width, height) = if shows_properties
                && window_natural_width < min_width_for_overlay
                && window_natural_height < min_height_for_overlay
            {
                // Avoid overlaying bottom sheet being triggered for the image properties by
                // using a window wide enough to allow for a sidebar
                (min_width_for_overlay.saturating_add(1), -1)
            } else {
                // this lets the window determine the default size from LpImage's natural size
                (-1, -1)
            };

            self.set_default_size(width, height);
        }
    }

    pub fn image_error(&self) {
        self.notify_is_showing_image();

        if self.is_visible() {
            return;
        }

        let current_page = self.imp().image_view.current_page();

        if current_page.is_some_and(|page| page.image().error().is_some()) {
            log::debug!("Showing window because loading image failed");
            self.present();
        }
    }

    fn on_fullscreen_changed(&self) {
        let icon = if self.is_fullscreen() {
            "view-restore-symbolic"
        } else {
            "view-fullscreen-symbolic"
        };
        self.imp().fullscreen_button.set_icon_name(icon);

        if !self.is_fullscreen() {
            self.set_cursor(None);
            self.show_controls();
        }
        self.schedule_hide_controls();
    }

    fn is_headerbar_flat(&self) -> bool {
        self.imp().window_content.is_headerbar_flat()
    }

    fn is_content_extended_to_top(&self) -> bool {
        self.imp().window_content.is_content_extended_to_top()
    }

    fn update_accel_status(&self) {
        let Some(application) = self.application() else {
            log::error!("No application for window found");
            return;
        };

        // Only change status if active window
        if self.is_active() {
            if self.visible_dialog().is_some() {
                // If AdwDialog is visible, remove global accels that are for the main window
                ActionPartGlobal::remove_accels(&application);
            } else {
                // Add accels if viewing main window
                ActionPartGlobal::add_accels(&application);
            }
        }
    }

    async fn show_about(&self) {
        let about = crate::about::dialog().await;
        about.present(Some(self));
    }
}
