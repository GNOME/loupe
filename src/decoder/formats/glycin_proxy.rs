// Copyright (c) 2023-2024 Sophie Herold
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

//! Decode using glycin

use std::sync::Arc;

use gtk::prelude::*;

use super::*;
use crate::decoder::tiling::{self, SharedFrameBuffer};
use crate::deps::*;
use crate::metadata::Metadata;

pub const FRAME_BUFFER: usize = 3;

#[derive(Debug)]
pub struct Glycin {
    cancellable: gio::Cancellable,
    next_frame_send: async_channel::Sender<()>,
}

impl Drop for Glycin {
    fn drop(&mut self) {
        self.cancellable.cancel();
    }
}

impl Glycin {
    pub fn new(file: gio::File, updater: UpdateSender, tiles: Arc<SharedFrameBuffer>) -> Self {
        let cancellable = gio::Cancellable::new();
        let cancellable_ = cancellable.clone();

        let (next_frame_send, next_frame_recv) = async_channel::bounded(2);

        updater.clone().spawn_error_handled(async move {
            let mut image_request = glycin::ImageRequest::new(file);

            #[cfg(feature = "disable-glycin-sandbox")]
            image_request.sandbox_mechanism(Some(glycin::SandboxMechanism::NotSandboxed));

            image_request.cancellable(cancellable_);

            let image = image_request.request().await?;

            let mut metadata: Metadata = Metadata::default();
            metadata.set_image_info(image.info().details.clone());
            metadata.set_mime_type(image.mime_type());
            updater.send(DecoderUpdate::Metadata(Box::new(metadata)));

            let dimensions = (image.info().width, image.info().height);
            tiles.set_original_dimensions(dimensions);

            let frame = image.next_frame().await?;

            let mut metadata: Metadata = Metadata::default();
            metadata.set_frame_info(frame.details);
            updater.send(DecoderUpdate::Metadata(Box::new(metadata)));
            tiles.set_original_dimensions((
                frame.texture.width() as u32,
                frame.texture.height() as u32,
            ));

            if let Some(delay) = frame.delay {
                updater.send(DecoderUpdate::Animated);

                let position = (0, 0);

                let tile = tiling::Tile {
                    position,
                    zoom_level: tiling::zoom_to_level(1.),
                    bleed: 0,
                    texture: frame.texture,
                };

                tiles.push_frame(tile, dimensions, delay);
                updater.send(DecoderUpdate::Redraw);

                loop {
                    if next_frame_recv.recv().await.is_ok() {
                        loop {
                            let frame = image.next_frame().await?;

                            let position = (0, 0);

                            let tile = tiling::Tile {
                                position,
                                zoom_level: tiling::zoom_to_level(1.),
                                bleed: 0,
                                texture: frame.texture,
                            };

                            tiles.push_frame(tile, dimensions, frame.delay.unwrap_or(delay));

                            if tiles.n_frames() >= FRAME_BUFFER {
                                break;
                            }
                        }
                    } else {
                        log::debug!("Animation handler gone");
                        return Ok(());
                    }
                }
            } else {
                let tile = tiling::Tile {
                    position: (0, 0),
                    zoom_level: tiling::zoom_to_level(1.),
                    bleed: 0,
                    texture: frame.texture,
                };

                tiles.push(tile);
            }

            Ok(())
        });

        Self {
            cancellable,
            next_frame_send,
        }
    }

    pub fn fill_frame_buffer(&self) {
        let _result = self.next_frame_send.try_send(());
    }
}
