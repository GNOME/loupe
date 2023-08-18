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

//! Decode using librsvg
use super::*;
use crate::decoder::tiling::{self, FrameBufferExt};
use crate::decoder::TileRequest;
use crate::deps::*;

use anyhow::Context;
use arc_swap::ArcSwap;
use async_channel as mpsc;
use gtk::prelude::*;

use std::sync::Arc;

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
    pub fn new(
        file: gio::File,
        updater: UpdateSender,
        tiles: Arc<ArcSwap<tiling::FrameBuffer>>,
    ) -> Self {
        let current_request: Arc<std::sync::RwLock<Request>> = Default::default();
        let request_store = current_request.clone();
        let cancellable = gio::Cancellable::new();
        let cancellable_ = cancellable.clone();
        let (wakeup, wakeup_resv) = mpsc::unbounded();

        updater.clone().spawn_error_handled(async move {
            let mut image_request = glycin::ImageRequest::new(file);

            //#[cfg(feature = "disable-glycin-sandbox")]
            //image_request.sandbox_mechanism(Some(glycin::SandboxMechanism::NotSandboxed));

            image_request.cancellable(cancellable.clone());

            let image = image_request.request().await?;

            let dimensions = if let Some(string) = image.info().dimensions_text.as_ref() {
                ImageDimensionDetails::Svg(string.to_string())
            } else {
                ImageDimensionDetails::None
            };

            tiles.set_original_dimensions_full(
                (image.info().width, image.info().height),
                dimensions,
            );

            updater.send(DecoderUpdate::Format(ImageFormat::new(
                image.mime_type(),
                image.format_name(),
            )));

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

                            let frame = image.specific_frame(frame_request).await?;

                            let position = (
                                tile_instructions.area.x() as u32,
                                tile_instructions.area.y() as u32,
                            );

                            tiles.push_tile(tiling, position, frame.texture);
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

    pub fn render_print(file: &gio::File, width: i32, height: i32) -> anyhow::Result<gdk::Texture> {
        #[allow(unused_mut)]
        let mut image_request = glycin::ImageRequest::new(file.clone());

        //  #[cfg(feature = "disable-glycin-sandbox")]
        //  image_request.sandbox_mechanism(Some(glycin::SandboxMechanism::NotSandboxed));

        let image = async_std::task::block_on(image_request.request())?;
        let frame_request = glycin::FrameRequest::new().scale(width as u32, height as u32);
        let frame = async_std::task::block_on(image.specific_frame(frame_request))?;

        Ok(frame.texture)
    }
}
