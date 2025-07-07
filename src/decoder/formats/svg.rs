// Copyright (c) 2023-2025 Sophie Herold
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

//! Decode using librsvg

use std::sync::Arc;

use anyhow::Context;
use async_channel as mpsc;
use gtk::prelude::*;

use super::*;
use crate::decoder::tiling::SharedFrameBuffer;
use crate::decoder::TileRequest;
use crate::deps::*;
use crate::metadata::Metadata;

/// Current librsvg limit on maximum dimensions. See
/// <https://gitlab.gnome.org/GNOME/librsvg/-/issues/938>
pub const RSVG_MAX_SIZE: u32 = 32_767;

#[derive(Debug)]
pub struct Svg {
    wakeup: mpsc::Sender<()>,
    current_request: Arc<std::sync::RwLock<Request>>,
    cancellable: gio::Cancellable,
}

#[derive(Default, Debug, Clone)]
enum Request {
    #[default]
    None,
    Tile(TileRequest),
}

impl Request {
    fn is_waiting(&self) -> bool {
        !matches!(self, Self::None)
    }
}

impl Drop for Svg {
    fn drop(&mut self) {
        self.cancellable.cancel();
        let _ = self.wakeup.try_send(());
    }
}

impl Svg {
    pub fn new(file: gio::File, updater: UpdateSender, tiles: Arc<SharedFrameBuffer>) -> Self {
        let current_request: Arc<std::sync::RwLock<Request>> = Default::default();
        let request_store = current_request.clone();
        let cancellable = gio::Cancellable::new();
        let cancellable_ = cancellable.clone();
        let (wakeup, wakeup_resv) = mpsc::unbounded();

        updater.clone().spawn_error_handled(async move {
            log::trace!("Setting up SVG loader");
            let mut loader = glycin::Loader::new(file);

            loader.cancellable(cancellable.clone());

            let image = loader.load().await?;

            let mut metadata: Metadata = Metadata::default();
            metadata.set_image_info(image.details().clone());
            metadata.set_mime_type(image.mime_type().to_string());
            updater.send(DecoderUpdate::Metadata(Box::new(metadata)));

            tiles.set_original_dimensions((image.details().width(), image.details().height()));

            let mut is_first_frame = true;

            loop {
                let tile_request = {
                    let mut request = request_store.write().unwrap();
                    let value = request.clone();
                    *request = Request::None;
                    value
                };

                if cancellable.is_cancelled() {
                    log::debug!("Terminating SVG decoder thread");
                    break;
                }

                match tile_request {
                    Request::None => {
                        let result = wakeup_resv.recv().await;
                        if result.is_err() {
                            return Ok(());
                        }
                    }

                    Request::Tile(tile_request) => {
                        let tiling = tiles
                            .get_layer_tiling_or_default(tile_request.zoom, tile_request.viewport);

                        let relevant_tiles = tiling.relevant_tiles(&tile_request.area);

                        for tile_instructions in relevant_tiles {
                            if request_store.read().ok().unwrap().is_waiting() {
                                break;
                            }

                            if tiles
                                .load()
                                .contains(tile_request.zoom, tile_instructions.coordinates)
                            {
                                continue;
                            }

                            let (total_width, total_height) =
                                tile_instructions.tiling.scaled_dimensions();
                            let area = tile_instructions.area_with_bleed();

                            let frame_request = glycin::FrameRequest::new()
                                .scale(total_width, total_height)
                                .clip(
                                    area.x() as u32,
                                    area.y() as u32,
                                    area.width() as u32,
                                    area.height() as u32,
                                );

                            match image.specific_frame(frame_request).await {
                                Ok(frame) => {
                                    if is_first_frame {
                                        is_first_frame = false;

                                        let mut metadata: Metadata = Metadata::default();
                                        metadata.set_frame_metadata(&frame);

                                        updater.send(DecoderUpdate::Metadata(Box::new(metadata)));
                                    }

                                    let position = (
                                        tile_instructions.area.x() as u32,
                                        tile_instructions.area.y() as u32,
                                    );

                                    tiles.push_tile(tiling, position, frame.texture());
                                }
                                Err(err) => {
                                    updater.send_error(err);
                                }
                            }
                        }
                    }
                }
            }

            Ok(())
        });

        Self {
            wakeup,
            current_request,
            cancellable: cancellable_,
        }
    }

    pub fn request(&self, request: TileRequest) -> anyhow::Result<()> {
        let mut current_request = self
            .current_request
            .write()
            .ok()
            .context("RwLock is poisoned")?;
        *current_request = Request::Tile(request);
        self.wakeup.try_send(())?;

        Ok(())
    }

    pub async fn render_print(
        file: &gio::File,
        width: i32,
        height: i32,
    ) -> anyhow::Result<gdk::Texture> {
        #[allow(unused_mut)]
        let loader = glycin::Loader::new(file.clone());

        let image = loader.load().await?;
        let frame_request = glycin::FrameRequest::new().scale(width as u32, height as u32);
        let frame = image.specific_frame(frame_request).await?;

        Ok(frame.texture())
    }
}
