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

use std::ops::Deref;

use adw::subclass::prelude::*;
use gtk::prelude::*;
use strum::IntoEnumIterator;

use crate::deps::*;
use crate::util::{Direction, Position};

/// Actions that need some global accels
///
/// The app level accels are needed for arrow key bindings. Because the arrow
/// keys are usually used for widget navigation, it's not possible to overwrite
/// them on key-binding level.
#[derive(strum::Display, strum::AsRefStr, strum::EnumIter)]
pub enum ActionPartGlobal {
    #[strum(to_string = "win.image-left-instant")]
    ImageLeftInstant,
    #[strum(to_string = "win.image-right-instant")]
    ImageRightInstant,
    // Pan
    #[strum(to_string = "win.pan-up")]
    PanUp,
    #[strum(to_string = "win.pan-right")]
    PanRight,
    #[strum(to_string = "win.pan-down")]
    PanDown,
    #[strum(to_string = "win.pan-left")]
    PanLeft,
}

impl ActionPartGlobal {
    pub fn add_accels(application: &gtk::Application) {
        for action in Self::iter() {
            match action {
                ActionPartGlobal::ImageRightInstant => {
                    application.set_accels_for_action(&action, &["Right"])
                }
                ActionPartGlobal::ImageLeftInstant => {
                    application.set_accels_for_action(&action, &["Left"])
                }
                ActionPartGlobal::PanUp => {
                    application.set_accels_for_action(&action, &["<Ctrl>Up"])
                }
                ActionPartGlobal::PanRight => {
                    application.set_accels_for_action(&action, &["<Ctrl>Right"])
                }
                ActionPartGlobal::PanDown => {
                    application.set_accels_for_action(&action, &["<Ctrl>Down"])
                }
                ActionPartGlobal::PanLeft => {
                    application.set_accels_for_action(&action, &["<Ctrl>Left"])
                }
            }
        }
    }

    pub fn remove_accels(application: &gtk::Application) {
        for action in Self::iter() {
            application.set_accels_for_action(&action, &[]);
        }
    }

    pub fn init_actions_and_bindings(klass: &mut <super::imp::LpWindow as ObjectSubclass>::Class) {
        for action in Self::iter() {
            match action {
                ActionPartGlobal::ImageLeftInstant => {
                    klass.install_action(&action, None, move |win, _, _| {
                        if win.direction() == gtk::TextDirection::Rtl {
                            win.imp().image_view.navigate(Direction::Forward, false);
                        } else {
                            win.imp().image_view.navigate(Direction::Back, false);
                        }
                    });
                    klass.add_binding_action(
                        gdk::Key::Page_Down,
                        gdk::ModifierType::empty(),
                        &action,
                    );
                }

                ActionPartGlobal::ImageRightInstant => {
                    klass.install_action(&action, None, move |win, _, _| {
                        if win.direction() == gtk::TextDirection::Rtl {
                            win.imp().image_view.navigate(Direction::Back, false);
                        } else {
                            win.imp().image_view.navigate(Direction::Forward, false);
                        }
                    });
                    klass.add_binding_action(
                        gdk::Key::Page_Up,
                        gdk::ModifierType::empty(),
                        &action,
                    );
                }

                // Pan
                ActionPartGlobal::PanUp => {
                    klass.install_action(&action, None, move |win, _, _| {
                        win.pan(&gtk::PanDirection::Up);
                    });
                }
                ActionPartGlobal::PanRight => {
                    klass.install_action(&action, None, move |win, _, _| {
                        win.pan(&gtk::PanDirection::Right);
                    });
                }
                ActionPartGlobal::PanDown => {
                    klass.install_action(&action, None, move |win, _, _| {
                        win.pan(&gtk::PanDirection::Down);
                    });
                }
                ActionPartGlobal::PanLeft => {
                    klass.install_action(&action, None, move |win, _, _| {
                        win.pan(&gtk::PanDirection::Left);
                    });
                }
            }
        }
    }
}

#[derive(strum::Display, strum::AsRefStr, strum::EnumIter)]
pub enum Action {
    // Navigation
    #[strum(to_string = "win.image-left")]
    ImageLeft,
    #[strum(to_string = "win.image-right")]
    ImageRight,
    #[strum(to_string = "win.previous")]
    Previous,
    #[strum(to_string = "win.next")]
    Next,
    #[strum(to_string = "win.first")]
    First,
    #[strum(to_string = "win.last")]
    Last,
    // Zoom/Rotate
    #[strum(to_string = "win.rotate-cw")]
    RotateCw,
    #[strum(to_string = "win.rotate-ccw")]
    RotateCcw,
    #[strum(to_string = "win.zoom-out-center")]
    ZoomOutCenter,
    #[strum(to_string = "win.zoom-in-center")]
    ZoomInCenter,
    #[strum(to_string = "win.zoom-out-cursor")]
    ZoomOutCursor,
    #[strum(to_string = "win.zoom-in-cursor")]
    ZoomInCursor,
    #[strum(to_string = "win.zoom-best-fit")]
    ZoomBestFit,
    #[strum(to_string = "win.zoom-to-exact-1")]
    ZoomToExact1,
    #[strum(to_string = "win.zoom-to-exact-2")]
    ZoomToExact2,
    // Misc
    #[strum(to_string = "win.open")]
    Open,
    #[strum(to_string = "win.open-with")]
    OpenWith,
    #[strum(to_string = "win.print")]
    Print,
    #[strum(to_string = "win.copy-image")]
    CopyImage,
    #[strum(to_string = "win.set-background")]
    SetBackground,
    #[strum(to_string = "win.toggle-fullscreen")]
    ToggleFullscreen,
    #[strum(to_string = "win.leave-fullscreen")]
    LeaveFullscreen,
    #[strum(to_string = "win.trash")]
    Trash,
    #[strum(to_string = "win.delete")]
    Delete,
    #[strum(to_string = "win.toggle-properties")]
    ToggleProperties,
    #[strum(to_string = "win.about")]
    About,
    #[strum(to_string = "win.reload")]
    Reload,
}

impl Action {
    pub fn init_actions_and_bindings(klass: &mut <super::imp::LpWindow as ObjectSubclass>::Class) {
        for action in Self::iter() {
            match action {
                // Navigation
                Action::ImageLeft => klass.install_action(&action, None, move |win, _, _| {
                    if win.direction() == gtk::TextDirection::Rtl {
                        win.imp().image_view.navigate(Direction::Forward, true);
                    } else {
                        win.imp().image_view.navigate(Direction::Back, true);
                    }
                }),

                Action::ImageRight => klass.install_action(&action, None, move |win, _, _| {
                    if win.direction() == gtk::TextDirection::Rtl {
                        win.imp().image_view.navigate(Direction::Back, true);
                    } else {
                        win.imp().image_view.navigate(Direction::Forward, true);
                    }
                }),

                Action::Previous => klass.install_action(&action, None, move |win, _, _| {
                    win.imp().image_view.navigate(Direction::Back, true);
                }),

                Action::Next => klass.install_action(&action, None, move |win, _, _| {
                    win.imp().image_view.navigate(Direction::Forward, true);
                }),

                Action::First => {
                    klass.install_action(&action, None, move |win, _, _| {
                        win.imp().image_view.jump(Position::First);
                    });
                    klass.add_binding_action(gdk::Key::Home, gdk::ModifierType::empty(), &action);
                }

                Action::Last => {
                    klass.install_action(&action, None, move |win, _, _| {
                        win.imp().image_view.jump(Position::Last);
                    });
                    klass.add_binding_action(gdk::Key::End, gdk::ModifierType::empty(), &action);
                }

                // Rotation/Zoom
                Action::RotateCw => {
                    klass.install_action(&action, None, |win, _, _| {
                        win.rotate_image(90.0);
                    });
                    klass.add_binding_action(gdk::Key::r, gdk::ModifierType::CONTROL_MASK, &action);
                }

                Action::RotateCcw => {
                    klass.install_action(&action, None, |win, _, _| {
                        win.rotate_image(-90.0);
                    });
                    klass.add_binding_action(
                        gdk::Key::r,
                        gdk::ModifierType::CONTROL_MASK.union(gdk::ModifierType::SHIFT_MASK),
                        &action,
                    );
                }

                Action::ZoomOutCenter => klass.install_action(&action, None, move |win, _, _| {
                    win.zoom_out_center();
                }),

                Action::ZoomInCenter => klass.install_action(&action, None, move |win, _, _| {
                    win.zoom_in_center();
                }),

                Action::ZoomOutCursor => {
                    klass.install_action(&action, None, move |win, _, _| {
                        win.zoom_out_cursor();
                    });
                    klass.add_binding_action(gdk::Key::minus, gdk::ModifierType::empty(), &action);
                    klass.add_binding_action(
                        gdk::Key::minus,
                        gdk::ModifierType::CONTROL_MASK,
                        &action,
                    );
                    klass.add_binding_action(
                        gdk::Key::KP_Subtract,
                        gdk::ModifierType::empty(),
                        &action,
                    );
                    klass.add_binding_action(
                        gdk::Key::KP_Subtract,
                        gdk::ModifierType::CONTROL_MASK,
                        &action,
                    );
                }

                Action::ZoomInCursor => {
                    klass.install_action(&action, None, move |win, _, _| {
                        win.zoom_in_cursor();
                    });
                    klass.add_binding_action(gdk::Key::equal, gdk::ModifierType::empty(), &action);
                    klass.add_binding_action(
                        gdk::Key::equal,
                        gdk::ModifierType::CONTROL_MASK,
                        &action,
                    );
                    klass.add_binding_action(gdk::Key::plus, gdk::ModifierType::empty(), &action);
                    klass.add_binding_action(
                        gdk::Key::plus,
                        gdk::ModifierType::CONTROL_MASK,
                        &action,
                    );
                    klass.add_binding_action(gdk::Key::KP_Add, gdk::ModifierType::empty(), &action);
                    klass.add_binding_action(
                        gdk::Key::KP_Add,
                        gdk::ModifierType::CONTROL_MASK,
                        &action,
                    );
                }

                Action::ZoomBestFit => {
                    klass.install_action(&action, None, move |win, _, _| {
                        win.zoom_best_fit();
                    });
                    klass.add_binding_action(gdk::Key::_0, gdk::ModifierType::empty(), &action);
                    klass.add_binding_action(
                        gdk::Key::_0,
                        gdk::ModifierType::CONTROL_MASK,
                        &action,
                    );
                    klass.add_binding_action(gdk::Key::KP_0, gdk::ModifierType::empty(), &action);
                    klass.add_binding_action(
                        gdk::Key::KP_0,
                        gdk::ModifierType::CONTROL_MASK,
                        &action,
                    );
                }

                Action::ZoomToExact1 => {
                    klass.install_action(&action, None, move |win, _, _| {
                        win.zoom_to_exact(1.);
                    });
                    klass.add_binding_action(gdk::Key::_1, gdk::ModifierType::empty(), &action);
                    klass.add_binding_action(gdk::Key::KP_1, gdk::ModifierType::empty(), &action);
                    klass.add_binding_action(
                        gdk::Key::_1,
                        gdk::ModifierType::CONTROL_MASK,
                        &action,
                    );
                    klass.add_binding_action(
                        gdk::Key::KP_1,
                        gdk::ModifierType::CONTROL_MASK,
                        &action,
                    );
                }

                Action::ZoomToExact2 => {
                    klass.install_action(&action, None, move |win, _, _| {
                        win.zoom_to_exact(2.);
                    });
                    klass.add_binding_action(gdk::Key::_2, gdk::ModifierType::empty(), &action);
                    klass.add_binding_action(gdk::Key::KP_2, gdk::ModifierType::empty(), &action);
                    klass.add_binding_action(
                        gdk::Key::_2,
                        gdk::ModifierType::CONTROL_MASK,
                        &action,
                    );
                    klass.add_binding_action(
                        gdk::Key::KP_2,
                        gdk::ModifierType::CONTROL_MASK,
                        &action,
                    );
                }

                // Misc
                Action::Open => {
                    klass.install_action_async(&action, None, |win, _, _| async move {
                        win.pick_file().await;
                    });
                    klass.add_binding_action(gdk::Key::O, gdk::ModifierType::CONTROL_MASK, &action);
                }

                Action::OpenWith => {
                    klass.install_action_async(&action, None, |win, _, _| async move {
                        win.open_with().await;
                    });
                    klass.add_binding_action(
                        gdk::Key::O,
                        gdk::ModifierType::CONTROL_MASK.union(gdk::ModifierType::SHIFT_MASK),
                        &action,
                    );
                }

                Action::Print => {
                    klass.install_action(&action, None, move |win, _, _| {
                        win.print();
                    });
                    klass.add_binding_action(gdk::Key::P, gdk::ModifierType::CONTROL_MASK, &action);
                }

                Action::CopyImage => klass.install_action(&action, None, move |win, _, _| {
                    win.copy_image();
                }),

                Action::SetBackground => {
                    klass.install_action_async(&action, None, |win, _, _| async move {
                        win.set_background().await;
                    });
                    klass.add_binding_action(
                        gdk::Key::F8,
                        gdk::ModifierType::CONTROL_MASK,
                        &action,
                    );
                }

                Action::ToggleFullscreen => {
                    klass.install_action(&action, None, move |win, _, _| {
                        win.toggle_fullscreen(!win.is_fullscreen());
                    });
                    klass.add_binding_action(gdk::Key::F11, gdk::ModifierType::empty(), &action);
                }

                Action::LeaveFullscreen => {
                    klass.install_action(&action, None, move |win, _, _| {
                        win.toggle_fullscreen(false);
                    });
                    klass.add_binding_action(gdk::Key::Escape, gdk::ModifierType::empty(), &action);
                }

                Action::Trash => {
                    klass.install_action_async(&action, None, |win, _, _| async move {
                        win.trash().await;
                    });
                    klass.add_binding_action(gdk::Key::Delete, gdk::ModifierType::empty(), &action);
                    klass.add_binding_action(
                        gdk::Key::KP_Delete,
                        gdk::ModifierType::empty(),
                        &action,
                    );
                }

                Action::Delete => {
                    klass.install_action_async(&action, None, |win, _, _| async move {
                        win.delete().await;
                    });
                    klass.add_binding_action(
                        gdk::Key::Delete,
                        gdk::ModifierType::SHIFT_MASK,
                        &action,
                    );
                    klass.add_binding_action(
                        gdk::Key::KP_Delete,
                        gdk::ModifierType::SHIFT_MASK,
                        &action,
                    );
                }

                Action::ToggleProperties => {
                    klass.install_action(&action, None, move |win, _, _| {
                        win.imp()
                            .properties_button
                            .set_active(!win.imp().properties_button.is_active());
                    });
                    klass.add_binding_action(gdk::Key::F9, gdk::ModifierType::empty(), &action);
                    klass.add_binding_action(
                        gdk::Key::Return,
                        gdk::ModifierType::ALT_MASK,
                        &action,
                    );
                }

                Action::About => {
                    klass.install_action_async(&action, None, |win, _, _| async move {
                        win.show_about().await
                    });
                }

                Action::Reload => {
                    klass.install_action_async(&action, None, move |win, _, _| async move {
                        if let Some(current_page) = win.imp().image_view.current_page() {
                            current_page.image().reload().await;
                        } else {
                            log::error!("No current image to reload");
                        }
                    });
                    klass.add_binding_action(gdk::Key::F5, gdk::ModifierType::empty(), &action);
                }
            }
        }
    }
}

impl Deref for ActionPartGlobal {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl Deref for Action {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}
