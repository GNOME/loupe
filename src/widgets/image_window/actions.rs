// Copyright (c) 2024-2025 Sophie Herold
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
use gdk::{Key, ModifierType};
use gtk::prelude::*;
use strum::IntoEnumIterator;

use super::LpImageWindow;
use crate::deps::*;
use crate::util::root::ParentWindow;
use crate::util::{Direction, Position};

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
    #[strum(to_string = "win.zoom-to-exact-100")]
    ZoomToExact100,
    #[strum(to_string = "win.zoom-to-exact-200")]
    ZoomToExact200,
    #[strum(to_string = "win.zoom-to-exact-300")]
    ZoomToExact300,
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
    #[strum(to_string = "win.edit")]
    Edit,
}

impl Action {
    pub fn add_bindings(win: &LpImageWindow) {
        for action in Self::iter() {
            for (key, modifiers) in action.keybindings() {
                let mut modifier = ModifierType::empty();
                for m in *modifiers {
                    modifier = modifier.union(*m);
                }

                let shortcut = gtk::Shortcut::new(
                    Some(gtk::KeyvalTrigger::new(*key, modifier)),
                    Some(gtk::NamedAction::new(&action)),
                );

                win.imp().shortcut_controller.add_shortcut(shortcut);
            }
        }
    }

    pub fn init_actions(klass: &mut <super::imp::LpImageWindow as ObjectSubclass>::Class) {
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
                }

                Action::Last => {
                    klass.install_action(&action, None, move |win, _, _| {
                        win.imp().image_view.jump(Position::Last);
                    });
                }

                // Rotation/Zoom
                Action::RotateCw => {
                    klass.install_action(&action, None, |win, _, _| {
                        win.rotate_image(-90.0);
                    });
                }

                Action::RotateCcw => {
                    klass.install_action(&action, None, |win, _, _| {
                        win.rotate_image(90.0);
                    });
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
                }

                Action::ZoomInCursor => {
                    klass.install_action(&action, None, move |win, _, _| {
                        win.zoom_in_cursor();
                    });
                }

                Action::ZoomBestFit => {
                    klass.install_action(&action, None, move |win, _, _| {
                        win.zoom_best_fit();
                    });
                }

                Action::ZoomToExact100 => {
                    klass.install_action(&action, None, move |win, _, _| {
                        win.zoom_to_exact(1.);
                    });
                }

                Action::ZoomToExact200 => {
                    klass.install_action(&action, None, move |win, _, _| {
                        win.zoom_to_exact(2.);
                    });
                }

                Action::ZoomToExact300 => {
                    klass.install_action(&action, None, move |win, _, _| {
                        win.zoom_to_exact(3.);
                    });
                }

                // Misc
                Action::Open => {
                    klass.install_action_async(&action, None, |win, _, _| async move {
                        win.pick_file().await;
                    });
                }

                Action::OpenWith => {
                    klass.install_action_async(&action, None, |win, _, _| async move {
                        win.open_with().await;
                    });
                }

                Action::Print => {
                    klass.install_action(&action, None, move |win, _, _| {
                        win.print();
                    });
                }

                Action::CopyImage => klass.install_action(&action, None, move |win, _, _| {
                    win.copy_image();
                }),

                Action::SetBackground => {
                    klass.install_action_async(&action, None, |win, _, _| async move {
                        win.set_background().await;
                    });
                }

                Action::ToggleFullscreen => {
                    klass.install_action(&action, None, move |obj, _, _| {
                        let win = obj.window();
                        win.toggle_fullscreen(!win.is_fullscreen());
                    });
                }

                Action::LeaveFullscreen => {
                    klass.install_action(&action, None, move |obj, _, _| {
                        obj.window().toggle_fullscreen(false);
                    });
                }

                Action::Trash => {
                    klass.install_action_async(&action, None, |win, _, _| async move {
                        win.trash().await;
                    });
                }

                Action::Delete => {
                    klass.install_action_async(&action, None, |win, _, _| async move {
                        win.delete().await;
                    });
                }

                Action::ToggleProperties => {
                    klass.install_action(&action, None, move |win, _, _| {
                        win.imp()
                            .properties_button
                            .set_active(!win.imp().properties_button.is_active());
                    });
                }

                Action::About => {
                    klass.install_action_async(&action, None, |obj, _, _| async move {
                        obj.window().show_about().await
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
                }

                Action::Edit => {
                    klass.install_action_async(&action, None, move |obj, _, _| async move {
                        obj.window().show_edit();
                    });
                }
            }
        }
    }

    pub fn keybindings(&self) -> &[(gdk::Key, &[gdk::ModifierType])] {
        match self {
            Action::First => &[(Key::Home, &[])],

            Action::Last => &[(Key::End, &[])],

            // Rotation/Zoom
            Action::RotateCw => &[(Key::r, &[ModifierType::CONTROL_MASK])],
            Action::RotateCcw => &[(
                Key::r,
                &[ModifierType::CONTROL_MASK, ModifierType::SHIFT_MASK],
            )],

            Action::ZoomOutCursor => &[
                (Key::minus, &[]),
                (Key::minus, &[ModifierType::CONTROL_MASK]),
                (Key::KP_Subtract, &[]),
                (Key::KP_Subtract, &[ModifierType::CONTROL_MASK]),
            ],
            Action::ZoomInCursor => &[
                (Key::equal, &[]),
                (Key::equal, &[ModifierType::CONTROL_MASK]),
                (Key::plus, &[]),
                (Key::plus, &[ModifierType::CONTROL_MASK]),
                (Key::KP_Add, &[]),
                (Key::KP_Add, &[ModifierType::CONTROL_MASK]),
            ],
            Action::ZoomBestFit => &[
                (Key::_0, &[]),
                (Key::_0, &[ModifierType::CONTROL_MASK]),
                (Key::KP_0, &[]),
                (Key::KP_0, &[ModifierType::CONTROL_MASK]),
            ],
            Action::ZoomToExact100 => &[
                (Key::_1, &[]),
                (Key::_1, &[ModifierType::CONTROL_MASK]),
                (Key::KP_1, &[]),
                (Key::KP_1, &[ModifierType::CONTROL_MASK]),
            ],
            Action::ZoomToExact200 => &[
                (Key::_2, &[]),
                (Key::_2, &[ModifierType::CONTROL_MASK]),
                (Key::KP_2, &[]),
                (Key::KP_2, &[ModifierType::CONTROL_MASK]),
            ],
            Action::ZoomToExact300 => &[
                (Key::_3, &[]),
                (Key::_3, &[ModifierType::CONTROL_MASK]),
                (Key::KP_3, &[]),
                (Key::KP_3, &[ModifierType::CONTROL_MASK]),
            ],

            // Misc
            Action::Open => &[(Key::o, &[ModifierType::CONTROL_MASK])],
            Action::OpenWith => &[(
                Key::o,
                &[ModifierType::CONTROL_MASK, gdk::ModifierType::SHIFT_MASK],
            )],
            Action::Print => &[(Key::p, &[ModifierType::CONTROL_MASK])],
            Action::SetBackground => &[(Key::F8, &[ModifierType::CONTROL_MASK])],
            Action::ToggleFullscreen => &[(Key::F11, &[])],
            Action::LeaveFullscreen => &[(Key::Escape, &[])],
            Action::Trash => &[
                (Key::KP_Delete, &[]),
                // The binding added last is shown in menus
                (Key::Delete, &[]),
            ],
            Action::Delete => &[
                (Key::Delete, &[ModifierType::SHIFT_MASK]),
                (Key::KP_Delete, &[ModifierType::SHIFT_MASK]),
            ],
            Action::ToggleProperties => &[(Key::F9, &[]), (Key::Return, &[ModifierType::ALT_MASK])],
            Action::Reload => &[(Key::F5, &[])],
            Action::Edit => &[(Key::e, &[])],
            _ => &[],
        }
    }
}

impl Deref for Action {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}
