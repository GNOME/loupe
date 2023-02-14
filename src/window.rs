// window.rs
//
// Copyright 2020 Christopher Davis <christopherdavis@gnome.org>
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
use crate::i18n::*;

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::clone;
use gtk::CompositeTemplate;
use gtk_macros::spawn;

use once_cell::sync::OnceCell;

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use crate::config;
use crate::util::{self, Direction, Position};
use crate::widgets::{LpImage, LpImageView, LpPropertiesView};

/// The duration when fading out in milliseconds
const FADE_OUT_DURATION: u32 = 2000;
/// The duration when fading in milliseconds
const FADE_IN_DURATION: u32 = 200;
/// The timeout in seconds before fading out various widgets
const FADE_IDLE_TIMEOUT: u32 = 2;

type IdHandle = RefCell<Option<glib::SourceId>>;

#[derive(Debug, Copy, Clone)]
enum FadingWidget {
    Header,
    ViewControls,
}

#[derive(Debug, Copy, Clone)]
enum FadeDirection {
    Out,
    In,
}

mod imp {
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
    #[derive(Default, Debug, CompositeTemplate)]
    #[template(file = "../data/gtk/window.ui")]
    pub struct LpWindow {
        // Template children are used with the
        // TemplateChild<T> wrapper, where T is the
        // object type of the template child.
        #[template_child]
        pub flap: TemplateChild<adw::Flap>,
        #[template_child]
        pub headerbar: TemplateChild<gtk::HeaderBar>,
        #[template_child]
        pub properties_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub menu_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub menu: TemplateChild<gtk::PopoverMenu>,
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub status_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub image_view: TemplateChild<LpImageView>,
        #[template_child]
        pub properties_view: TemplateChild<LpPropertiesView>,
        #[template_child]
        pub drop_target: TemplateChild<gtk::DropTarget>,

        pub fade_animation: OnceCell<adw::TimedAnimation>,
        pub motion_timeout_id: IdHandle,

        pub header_fade_animation: OnceCell<adw::TimedAnimation>,
        pub header_timeout_id: IdHandle,
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
            Self::Type::bind_template_callbacks(klass);

            // Set up actions
            klass.install_action("win.toggle-fullscreen", None, move |win, _, _| {
                win.toggle_fullscreen(!win.is_fullscreened());
            });

            klass.install_action("win.next", None, move |win, _, _| {
                win.imp().image_view.navigate(Direction::Forward);
            });

            klass.install_action("win.previous", None, move |win, _, _| {
                win.imp().image_view.navigate(Direction::Back);
            });

            klass.install_action("win.image-right", None, move |win, _, _| {
                if win.direction() == gtk::TextDirection::Rtl {
                    win.imp().image_view.navigate(Direction::Back);
                } else {
                    win.imp().image_view.navigate(Direction::Forward);
                }
            });

            klass.install_action("win.image-left", None, move |win, _, _| {
                if win.direction() == gtk::TextDirection::Rtl {
                    win.imp().image_view.navigate(Direction::Forward);
                } else {
                    win.imp().image_view.navigate(Direction::Back);
                }
            });

            klass.install_action("win.first", None, move |win, _, _| {
                win.imp().image_view.jump(Position::First);
            });

            klass.install_action("win.last", None, move |win, _, _| {
                win.imp().image_view.jump(Position::Last);
            });

            klass.install_action("win.zoom-out", None, move |win, _, _| {
                win.zoom_out();
            });

            klass.install_action("win.zoom-in", None, move |win, _, _| {
                win.zoom_in();
            });

            klass.install_action("win.zoom-to", Some("d"), move |win, _, level| {
                win.zoom_to(level.unwrap().get().unwrap());
            });

            klass.install_action("win.zoom-best-fit", None, move |win, _, _| {
                win.zoom_best_fit();
            });

            klass.install_action("win.pan-up", None, move |win, _, _| {
                win.pan(&gtk::PanDirection::Up);
            });

            klass.install_action("win.pan-down", None, move |win, _, _| {
                win.pan(&gtk::PanDirection::Down);
            });

            klass.install_action("win.pan-left", None, move |win, _, _| {
                win.pan(&gtk::PanDirection::Left);
            });

            klass.install_action("win.pan-right", None, move |win, _, _| {
                win.pan(&gtk::PanDirection::Right);
            });

            klass.install_action("win.leave-fullscreen", None, move |win, _, _| {
                win.toggle_fullscreen(false);
            });

            klass.install_action("win.toggle-properties", None, move |win, _, _| {
                win.imp()
                    .properties_button
                    .set_active(!win.imp().properties_button.is_active());
            });

            klass.install_action_async("win.open", None, |win, _, _| async move {
                win.pick_file().await;
            });

            klass.install_action_async("win.open-with", None, |win, _, _| async move {
                win.open_with().await;
            });

            klass.install_action("win.rotate", Some("d"), move |win, _, angle| {
                win.rotate_image(angle.unwrap().get().unwrap());
            });

            klass.install_action_async("win.set-background", None, |win, _, _| async move {
                win.set_background().await;
            });

            klass.install_action("win.print", None, move |win, _, _| {
                win.print();
            });

            klass.install_action("win.copy", None, move |win, _, _| {
                win.copy();
            });

            klass.install_action_async("win.trash", None, |win, _, _| async move {
                win.trash().await;
            });

            klass.install_action("win.show-toast", Some("(si)"), move |win, _, var| {
                if let Some((ref toast, i)) = var.and_then(|v| v.get::<(String, i32)>()) {
                    win.show_toast(toast, adw::ToastPriority::__Unknown(i));
                }
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for LpWindow {
        fn constructed(&self) {
            let obj = self.obj();

            self.parent_constructed();

            if config::PROFILE == ".Devel" {
                obj.add_css_class("devel");
            }

            // Limit effect of modal dialogs to this window
            // and keeps the others usable
            gtk::WindowGroup::new().add_window(&*obj);

            obj.set_actions_enabled(false);

            let current_image_signals = self.image_view.current_image_signals();
            // clone! is a macro from glib-rs that allows
            // you to easily handle references in callbacks
            // without refcycles or leaks.
            //
            // When you don't want the callback to keep the
            // Object alive, pass as @weak. Otherwise, pass
            // as @strong. Most of the time you will want
            // to use @weak.
            current_image_signals.connect_bind_local(glib::clone!(@weak obj => move |_, _| {
                obj.on_zoom_status_changed()
            }));

            let win = &*obj;
            current_image_signals.connect_closure(
                "notify::best-fit",
                false,
                // `closure_local!` is similar to `clone`, but you use `@watch` instead of clone.
                // `@watch` means that this signal will be disconnected when the watched object
                // is dropped.
                glib::closure_local!(@watch win => move |_: &LpImage, _: &glib::ParamSpec| {
                    win.on_zoom_status_changed();
                }),
            );

            current_image_signals.connect_closure(
                "notify::is-max-zoom",
                false,
                glib::closure_local!(@watch win => move |_: &LpImage, _: &glib::ParamSpec| {
                    win.on_zoom_status_changed();
                }),
            );

            current_image_signals.connect_closure(
                "notify::image-size",
                false,
                glib::closure_local!(@watch win => move |_: &LpImage, _: &glib::ParamSpec| {
                    win.image_size_ready();
                }),
            );

            current_image_signals.connect_closure(
                "notify::error",
                false,
                glib::closure_local!(@watch win => move |_: &LpImage, _: &glib::ParamSpec| {
                    win.image_error();
                }),
            );

            self.image_view.connect_notify_local(
                Some("current-page"),
                glib::clone!(@weak obj => move |_, _| {
                    obj.images_available();
                }),
            );

            // action win.previous status
            self.image_view.connect_notify_local(
                Some("is-previous-available"),
                glib::clone!(@weak obj => move |_, _| {
                    obj.action_set_enabled(
                        "win.previous",
                        obj.imp().image_view.is_previous_available(),
                    );
                }),
            );

            // action win.next status
            self.image_view.connect_notify_local(
                Some("is-next-available"),
                glib::clone!(@weak obj => move |_, _| {
                    obj.action_set_enabled(
                        "win.next",
                        obj.imp().image_view.is_next_available(),
                    );
                }),
            );

            // Properties status
            self.properties_button.connect_active_notify(
                glib::clone!(@weak obj => move |props_btn| {
                    let imp = obj.imp();
                    if props_btn.is_active() {
                        imp.headerbar.remove_css_class("osd");
                        obj.fade_all_in();
                    } else {
                        imp.headerbar.add_css_class("osd");
                        obj.queue_fade_out_all();
                    }
                }),
            );

            // Listen to whether or not the menu opens
            self.menu
                .connect_visible_notify(glib::clone!(@weak obj => move |popover| {
                    if popover.is_visible() {
                        obj.fade_all_in();
                    } else {
                        // The popover was just hidden, so fade after timeout
                        obj.queue_fade_out_all();
                    }
                }));

            // Make widgets visible when the focus moves
            obj.connect_move_focus(|obj, _| {
                let imp = obj.imp();

                if obj.images_showing() && !imp.properties_button.is_active() {
                    obj.fade_all_in();
                    obj.queue_fade_out_all();
                }
            });

            self.status_page
                .set_icon_name(Some(&format!("{}-symbolic", config::APP_ID)));

            // Set help overlay
            let builder = gtk::Builder::from_resource("/org/gnome/Loupe/gtk/help_overlay.ui");
            let help_overlay = builder.object("help_overlay").unwrap();
            obj.set_help_overlay(Some(&help_overlay));

            self.drop_target.set_types(&[gdk::FileList::static_type()]);

            // For callbacks, you will want to reference the GTK docs on
            // the relevant signal to see which parameters you need.
            // In this case, we need only need the GValue,
            // so we name it `value` then use `_` for the other spots.
            self.drop_target.connect_drop(
                clone!(@weak obj => @default-return false, move |_, value, _, _| {
                    // Here we use a GValue, which is a dynamic object that can hold different types,
                    // e.g. strings, numbers, or in this case objects. In order to get the GdkFileList
                    // from the GValue, we need to use the `get()` method.
                    //
                    // We've added type annotations here, and written it as `let list: gdk::FileList = ...`,
                    // but you might also see places where type arguments are used.
                    // This line could have been written as `let list = value.get::<gdk::FileList>().unwrap()`.
                    let list: gdk::FileList = match value.get() {
                        Ok(list) => list,
                        Err(err) => {
                            log::error!("Issue with drop value: {err}");
                            return false;
                        }
                    };

                    // TODO: Handle this like EOG and make a "directory" out of the given files
                    let file = list.files().get(0).unwrap().clone();
                    let info = util::query_attributes(
                        &file,
                        vec![
                            gio::FILE_ATTRIBUTE_STANDARD_CONTENT_TYPE,
                            gio::FILE_ATTRIBUTE_STANDARD_DISPLAY_NAME,
                        ],
                    )
                    .expect("Could not query file info");

                    if info
                        .content_type()
                        .map(|t| t.to_string())
                        .filter(|t| t.starts_with("image/"))
                        .is_some() {
                        obj.set_image_from_path(&file.path().unwrap());
                    } else {
                        obj.show_toast(
                            i18n_f("“{}” is not a valid image.", &[&info.display_name()]),
                            adw::ToastPriority::High,
                        );
                    }

                    true
                }),
            );
        }
    }

    impl WidgetImpl for LpWindow {}
    impl WindowImpl for LpWindow {}
    impl ApplicationWindowImpl for LpWindow {}
    impl AdwApplicationWindowImpl for LpWindow {}
}

glib::wrapper! {
    pub struct LpWindow(ObjectSubclass<imp::LpWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionMap, gio::ActionGroup, gtk::Native;
}

#[gtk::template_callbacks]
impl LpWindow {
    pub fn new<A: IsA<gtk::Application>>(app: &A) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    fn toggle_fullscreen(&self, fullscreen: bool) {
        self.set_fullscreened(fullscreen);
    }

    fn zoom_out(&self) {
        self.imp().image_view.zoom_out();
    }

    fn zoom_in(&self) {
        self.imp().image_view.zoom_in();
    }

    fn zoom_to(&self, level: f64) {
        self.imp().image_view.zoom_to(level);
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
        let filter_store = gio::ListStore::new(gtk::FileFilter::static_type());

        let filter = gtk::FileFilter::new();
        filter.set_property("name", &String::from("Supported image files"));
        filter.add_mime_type("image/*");
        filter_store.append(&filter);

        let chooser = gtk::FileDialog::builder()
            .title(i18n("Open Image"))
            .filters(&filter_store)
            .modal(true)
            .build();

        if let Ok(file) = chooser.open_future(Some(self)).await {
            self.set_image_from_path(&file.path().unwrap());
        } else {
            // TODO: ui error
            log::error!("Failed to choose file");
        }
    }

    async fn open_with(&self) {
        let imp = self.imp();

        if let Some(ref file) = imp.image_view.current_path().map(gio::File::for_path) {
            let launcher = gtk::FileLauncher::new(Some(file));
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

    fn copy(&self) {
        let imp = self.imp();

        if let Err(e) = imp.image_view.copy() {
            log::error!("Failed to copy to clipboard: {}", e);
        } else {
            self.show_toast(i18n("Image copied to clipboard"), adw::ToastPriority::High);
        }
    }

    async fn trash(&self) {
        let image_view = self.image_view();
        let (Some(file), Some(path)) = (image_view.current_file(), image_view.current_path())
            else { log::error!("No file to trash"); return; };

        let result = file.trash_future(glib::Priority::default()).await;

        match result {
            Ok(()) => {
                let toast = adw::Toast::builder()
                    .title(i18n("Image moved to trash"))
                    .button_label(i18n("Undo"))
                    .build();
                toast.connect_button_clicked(glib::clone!(@weak self as win => move |_| {
                    let path = path.clone();
                    spawn!(async move {
                        let result = crate::util::untrash(&path).await;
                        match result {
                            Ok(()) => win.image_view().set_image_from_path(&path),
                            Err(err) => {
                                log::error!("Failed to untrash {path:?}: {err}");
                                win.show_toast(
                                    i18n("Failed to restore image from trash"),
                                    adw::ToastPriority::High,
                                );
                            }
                        }
                    });
                }));
                self.imp().toast_overlay.add_toast(toast);
            }
            Err(err) => {
                if Some(gio::IOErrorEnum::NotSupported) == err.kind::<gio::IOErrorEnum>() {
                    self.delete(&path).await;
                } else {
                    log::error!("Failed to delete file {path:?}: {err}");
                    self.show_toast(
                        i18n("Failed to move image to trash"),
                        adw::ToastPriority::Normal,
                    );
                }
            }
        }
    }

    /// Permanently delete image
    ///
    /// Fallback for when trash no available
    async fn delete(&self, path: &Path) {
        let dialog = adw::MessageDialog::builder()
            .modal(true)
            .transient_for(self)
            .heading(i18n("Permanently Delete Image?"))
            .body(i18n_f(
                "The image “{}” can only be deleted permanently.",
                &[&PathBuf::from(&path.file_name().unwrap_or_default())
                    .display()
                    .to_string()],
            ))
            .build();

        dialog.add_responses(&[("cancel", &i18n("Cancel")), ("delete", &i18n("Delete"))]);
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);

        if "delete" == dialog.choose_future().await {
            let file = gio::File::for_path(path);
            let result = file.delete_future(glib::Priority::default()).await;

            if let Err(err) = result {
                log::error!("Failed to delete file {path:?}: {err}");
                self.show_toast(i18n("Failed to delete image"), adw::ToastPriority::Normal);
            }
        }
    }

    fn image_view(&self) -> LpImageView {
        self.imp().image_view.clone()
    }

    fn show_toast(&self, text: impl AsRef<str>, priority: adw::ToastPriority) {
        let imp = self.imp();

        let toast = adw::Toast::new(text.as_ref());
        toast.set_priority(priority);

        imp.toast_overlay.add_toast(toast);
    }

    pub fn set_image_from_path(&self, path: &Path) {
        let imp = self.imp();

        log::debug!("Loading file: {path:?}");
        imp.image_view.set_image_from_path(path);
        self.set_actions_enabled(true);
    }

    pub fn set_actions_enabled(&self, enabled: bool) {
        self.action_set_enabled("win.open-with", enabled);
        self.action_set_enabled("win.set-background", enabled);
        self.action_set_enabled("win.toggle-fullscreen", enabled);
        self.action_set_enabled("win.print", enabled);
        self.action_set_enabled("win.rotate", enabled);
        self.action_set_enabled("win.copy", enabled);
        self.action_set_enabled("win.trash", enabled);
        self.action_set_enabled("win.zoom-best-fit", enabled);
        self.action_set_enabled("win.zoom-to", enabled);
        self.action_set_enabled("win.toggle-properties", enabled);
    }

    /// Handles change in availability of images
    fn images_available(&self) {
        let imp = self.imp();

        if imp.image_view.current_page().is_some() {
            imp.stack.set_visible_child(&*imp.image_view);
            imp.image_view.grab_focus();

            if !imp.properties_button.is_active() {
                imp.headerbar.add_css_class("osd");
            }

            self.queue_fade_out_all();
        } else {
            imp.stack.set_visible_child(&*imp.status_page);
            imp.status_page.grab_focus();
        }
    }

    pub fn image_size_ready(&self) {
        // if visible for whatever reason, don't do any resize
        if self.is_visible() {
            return;
        }

        let image = self
            .imp()
            .image_view
            .current_page()
            .map(|page| page.image());

        if let Some(image) = image {
            if image.image_size() > (0, 0) {
                log::debug!("Showing window because image size is ready");
                // this let's the window determine the default size from LpImage's natural size
                self.set_default_size(-1, -1);
                self.present();
            }
        }
    }

    pub fn image_error(&self) {
        if self.is_visible() {
            return;
        }

        let current_page = self.imp().image_view.current_page();

        if let Some(page) = current_page {
            if page.image().error().is_some() {
                log::debug!("Showing window because loading image failed");
                self.present();
            }
        }
    }

    fn on_zoom_status_changed(&self) {
        let can_zoom_out = self
            .image_view()
            .current_image()
            .map(|image| !image.is_best_fit())
            .unwrap_or_default();
        let can_zoom_in = self
            .image_view()
            .current_image()
            .map(|image| !image.is_max_zoom())
            .unwrap_or_default();

        self.action_set_enabled("win.zoom-out", can_zoom_out);
        self.action_set_enabled("win.zoom-in", can_zoom_in);
    }

    /// Retrieves or initializes the fade animation for the bottom controls
    fn fade_animation(&self) -> &adw::TimedAnimation {
        let imp = self.imp();

        let image_view = &*imp.image_view;

        imp.fade_animation.get_or_init(|| {
            let target = adw::PropertyAnimationTarget::new(image_view, "control-opacity");

            let animation = adw::TimedAnimation::builder()
                .duration(FADE_IN_DURATION)
                .widget(image_view)
                .target(&target)
                .build();

            animation
        })
    }

    /// Retrieves or initializes the fade animation for the header
    fn header_fade_animation(&self) -> &adw::TimedAnimation {
        let imp = self.imp();

        imp.header_fade_animation.get_or_init(|| {
            let target = adw::PropertyAnimationTarget::new(&*imp.headerbar, "opacity");

            let animation = adw::TimedAnimation::builder()
                .duration(FADE_IN_DURATION)
                .widget(&*imp.headerbar)
                .target(&target)
                .build();

            animation
        })
    }

    /// Trigger a fade animation or change its direction
    fn fade(&self, animation: &adw::TimedAnimation, direction: FadeDirection) {
        let widget = animation.widget();
        let current_target_value: f64 = animation
            .target()
            .downcast_ref::<adw::PropertyAnimationTarget>()
            .map(|t| t.pspec())
            .map(|p| widget.property(p.name()))
            .expect(
                "The animation target has been improperly configured. This should never happen.",
            );

        let (duration, value) = match direction {
            FadeDirection::Out => (FADE_OUT_DURATION, 0.),
            FadeDirection::In => (FADE_IN_DURATION, 1.),
        };

        // Do nothing if we're already animating in the same direction
        if animation.value_to() == value && animation.state() == adw::AnimationState::Playing {
            return;
        }

        animation.set_duration(duration);
        animation.set_value_from(current_target_value);
        animation.set_value_to(value);

        animation.play();
    }

    /// Queue a fade animation to play after `FADE_IDLE_TIMEOUT` seconds of inactivity
    fn queue_fade_out(&self, animation: &adw::TimedAnimation, widget: FadingWidget) {
        fn handle_for_widget(win: &LpWindow, widget: FadingWidget) -> &IdHandle {
            let imp = win.imp();

            match widget {
                FadingWidget::Header => &imp.header_timeout_id,
                FadingWidget::ViewControls => &imp.motion_timeout_id,
            }
        }

        let id_handle = handle_for_widget(self, widget);

        if let Some(id) = id_handle.take() {
            id.remove();
        }

        let id = glib::timeout_add_seconds_local(
            FADE_IDLE_TIMEOUT,
            clone!(@weak self as win, @weak animation => @default-return glib::Continue(false), move || {
                let imp = win.imp();

                // Don't fade when the properties or primary menu are showing
                if !imp.properties_button.is_active() && !imp.menu.is_visible() {
                    win.fade(&animation, FadeDirection::Out);
                }

                let id_handle = handle_for_widget(&win, widget);

                // We don't want to queue up multiple timeouts here.
                if let Some(id) = id_handle.take() {
                    id.remove();
                }
                glib::Continue(false)
            }),
        );

        id_handle.replace(Some(id));
    }

    fn fade_all_in(&self) {
        self.fade(self.header_fade_animation(), FadeDirection::In);
        self.fade(self.fade_animation(), FadeDirection::In);
    }

    /// Queue all controls to fade out after a timeout
    fn queue_fade_out_all(&self) {
        self.queue_fade_out(self.header_fade_animation(), FadingWidget::Header);
        self.queue_fade_out(self.fade_animation(), FadingWidget::ViewControls);
    }

    /// Whether or not the window is showing images
    fn images_showing(&self) -> bool {
        let imp = self.imp();
        imp.stack.visible_child().as_ref() == Some(&*imp.image_view.upcast_ref())
    }

    /// Handle motion, fading in `animation` and queing it to be faded out
    fn handle_motion(&self, animation: &adw::TimedAnimation, widget: FadingWidget) {
        if !self.images_showing() {
            return;
        }

        self.fade(&animation, FadeDirection::In);

        self.queue_fade_out(&animation, widget);
    }

    // In the LpWindow UI file we define a `gtk::Expression`s
    // that is a closure. This closure takes the current `gio::File`
    // and processes it to return a window title.
    //
    // In this function we chain `Option`s with `and_then()` in order
    // to handle optional results with a fallback, without needing to
    // have multiple `match` or `if let` branches, and without needing
    // to unwrap.
    #[template_callback]
    fn window_title(&self, path: glib::variant::Variant) -> String {
        // ensure that templates are initialized
        if path.as_maybe().is_none() {
            i18n("Loupe")
        } else {
            self.imp()
                .image_view
                .current_file()
                .and_then(|f| util::get_file_display_name(&f)) // If the file exists, get display name
                .unwrap_or_else(|| i18n("Loupe")) // Return that or the default if there's nothing
        }
    }

    // We also have a closure that returns `adw::FlapFoldPolicy`.
    // if we aren't fullscreened, or if the properties are revealed,
    // we unfold the main flap. Otherwise we're always folded.
    #[template_callback]
    fn fold_policy(&self, properties_revealed: bool) -> adw::FlapFoldPolicy {
        if properties_revealed {
            adw::FlapFoldPolicy::Never
        } else {
            adw::FlapFoldPolicy::Always
        }
    }

    #[template_callback]
    fn on_motion_cb(&self) {
        self.handle_motion(&self.fade_animation(), FadingWidget::ViewControls);
    }

    #[template_callback]
    fn on_header_motion_cb(&self) {
        self.handle_motion(&self.header_fade_animation(), FadingWidget::Header);
    }

    #[template_callback]
    fn on_tapped_cb(&self) {
        self.fade_all_in();
        self.queue_fade_out_all();
    }
}
