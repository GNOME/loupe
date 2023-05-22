// Copyright (c) 2023 Sophie Herold
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
use super::*;
use crate::decoder::tiling::{self, FrameBufferExt};
use crate::deps::*;
use crate::image_metadata::ImageMetadata;

use arc_swap::ArcSwap;
use gtk::prelude::*;

use std::sync::Arc;

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
    pub fn new(
        file: gio::File,
        updater: UpdateSender,
        tiles: Arc<ArcSwap<tiling::FrameBuffer>>,
    ) -> Self {
        let cancellable = gio::Cancellable::new();
        let cancellable_ = cancellable.clone();

        let (next_frame_send, next_frame_recv) = async_channel::bounded(2);

        glib::MainContext::default().spawn(async move {
            let update_sender = updater.clone();

            let result: Result<(), glycin::Error> = async move {
                let mut image_request = glycin::ImageRequest::new(file);

                #[cfg(feature = "disable-glycin-sandbox")]
                image_request.sandbox_mechanism(Some(glycin::SandboxMechanism::NotSandboxed));

                image_request.cancellable(cancellable_);

                let image = image_request.request().await?;

                if let Some(exif_raw) = image.info().exif.clone().into() {
                    updater.send(DecoderUpdate::Metadata(ImageMetadata::from_exif_bytes(
                        exif_raw,
                    )));
                }

                let dimensions = (image.info().width, image.info().height);
                tiles.set_original_dimensions(dimensions);

                updater.send(DecoderUpdate::Format(ImageFormat::new(
                    image.mime_type(),
                    image.format_name(),
                )));

                let frame = image.next_frame().await?;

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

                                tiles.push_frame(tile, dimensions, delay);

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
            }
            .await;

            if let Err(err) = result {
                if err.unsupported_format().is_some() {
                    update_sender.send(DecoderUpdate::UnsupportedFormat);
                }
                update_sender.send(DecoderUpdate::Error(err.into()));
            }
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
