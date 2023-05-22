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
use gtk::prelude::*;

use std::sync::Arc;

/// Current librsvg limit on maximum dimensions. See
/// <https://gitlab.gnome.org/GNOME/librsvg/-/issues/938>
pub const RSVG_MAX_SIZE: u32 = 32_767;

#[derive(Debug)]
pub struct Svg {
    thread_handle: std::thread::JoinHandle<()>,
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
        self.thread_handle.thread().unpark();
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

        let thread_handle = updater.spawn_error_handled(move || {
            let handle = rsvg::Loader::new().read_file(&file, Some(&cancellable))?;
            let renderer = rsvg::CairoRenderer::new(&handle);

            let (original_width, original_height) = svg_dimensions(&renderer);

            let intrisic_dimensions = renderer.intrinsic_dimensions();
            tiles.set_original_dimensions_full(
                (original_width, original_height),
                ImageDimensionDetails::Svg((intrisic_dimensions.width, intrisic_dimensions.height)),
            );

            loop {
                let tile_request = {
                    let mut request = request_store.write().ok().context("RwLock is poisoned")?;
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
                        std::thread::park();
                    }

                    Request::Tile(tile_request) => {
                        let tiling = tiles
                            .get_layer_tiling_or_default(tile_request.zoom, tile_request.viewport);

                        let relevant_tiles = tiling.relevant_tiles(&tile_request.area);

                        for tile_instructions in relevant_tiles {
                            if request_store
                                .read()
                                .ok()
                                .context("RwLock is poisoned")?
                                .is_waiting()
                            {
                                break;
                            }

                            if tiles
                                .load()
                                .contains(tile_request.zoom, tile_instructions.coordinates)
                            {
                                continue;
                            }

                            let area = tile_instructions.area_with_bleed();
                            let surface = cairo::ImageSurface::create(
                                cairo::Format::ARgb32,
                                area.width() as i32,
                                area.height() as i32,
                            )?;

                            let context = cairo::Context::new(&surface)?;
                            let (total_width, total_height) =
                                tile_instructions.tiling.scaled_dimensions();

                            // librsvg does not currently support larger images
                            if total_height > RSVG_MAX_SIZE || total_width > RSVG_MAX_SIZE {
                                continue;
                            }

                            renderer
                                .render_document(
                                    &context,
                                    &cairo::Rectangle::new(
                                        -area.x() as f64,
                                        -area.y() as f64,
                                        total_width as f64,
                                        total_height as f64,
                                    ),
                                )
                                .context("Failed to render image")?;
                            drop(context);

                            let decoded_image = Decoded { surface };

                            let position = (
                                tile_instructions.area.x() as u32,
                                tile_instructions.area.y() as u32,
                            );
                            let texture = decoded_image.into_texture()?;

                            tiles.push_tile(tiling, position, texture);
                        }
                    }
                }
            }

            Ok(())
        });

        Self {
            thread_handle,
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
        self.thread_handle.thread().unpark();

        Ok(())
    }

    pub fn render_print(
        file: &gio::File,
        width: i32,
        height: i32,
    ) -> anyhow::Result<cairo::ImageSurface> {
        let handle = rsvg::Loader::new().read_file(file, gio::Cancellable::NONE)?;
        let renderer = rsvg::CairoRenderer::new(&handle);
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height)?;
        let context = cairo::Context::new(&surface)?;
        renderer
            .render_document(
                &context,
                &cairo::Rectangle::new(0., 0., width as f64, height as f64),
            )
            .context("Failed to render image")?;
        drop(context);

        Ok(surface)
    }
}

pub fn svg_dimensions(renderer: &rsvg::CairoRenderer) -> (u32, u32) {
    if let Some((width, height)) = renderer.intrinsic_size_in_pixels() {
        (width.ceil() as u32, height.ceil() as u32)
    } else {
        match renderer.intrinsic_dimensions() {
            rsvg::IntrinsicDimensions {
                width:
                    rsvg::Length {
                        length: width,
                        unit: rsvg::LengthUnit::Percent,
                    },
                height:
                    rsvg::Length {
                        length: height,
                        unit: rsvg::LengthUnit::Percent,
                    },
                vbox: Some(vbox),
            } => (
                (width * vbox.width()).ceil() as u32,
                (height * vbox.height()).ceil() as u32,
            ),
            dimensions => {
                log::warn!("Failed to parse SVG dimensions: {dimensions:?}");
                (300, 300)
            }
        }
    }
}

struct Decoded {
    surface: cairo::ImageSurface,
}

impl Decoded {
    pub fn into_texture(self) -> anyhow::Result<gdk::Texture> {
        let memory_format = {
            #[cfg(target_endian = "little")]
            {
                gdk::MemoryFormat::B8g8r8a8
            }

            #[cfg(target_endian = "big")]
            {
                gdk::MemoryFormat::A8r8g8b8
            }
        };

        let width = self.surface.width();
        let height = self.surface.height();
        let stride = self.surface.stride() as usize;

        let bytes = glib::Bytes::from_owned(
            self.surface
                .take_data()
                .context("Cairo surface already taken")?
                .to_vec(),
        );

        Ok(gdk::MemoryTexture::new(width, height, memory_format, &bytes, stride).upcast())
    }
}
