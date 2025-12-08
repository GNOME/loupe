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
use gtk::prelude::*;
use strum::IntoEnumIterator;

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
}

impl Deref for Action {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}
