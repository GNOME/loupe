///! Decode using librsvg
use super::*;
use crate::decoder::tiling::{self, TilingStoreExt};
use crate::decoder::TileRequest;
use crate::deps::*;

use anyhow::Context;
use arc_swap::ArcSwap;
use gtk::prelude::*;

use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug)]
pub struct Svg {
    thread_handle: std::thread::JoinHandle<()>,
    current_request: Arc<std::sync::RwLock<Request>>,
}

#[derive(Default, Debug, Clone)]
enum Request {
    #[default]
    None,
    Exit,
    Tile(TileRequest),
}

impl Request {
    fn is_waiting(&self) -> bool {
        !matches!(self, Self::None)
    }
}

impl Drop for Svg {
    fn drop(&mut self) {
        let mut request = self.current_request.write().unwrap();
        *request = Request::Exit;
        self.thread_handle.thread().unpark();
    }
}

impl Svg {
    pub fn new(
        path: PathBuf,
        updater: UpdateSender,
        tiles: Arc<ArcSwap<tiling::TilingStore>>,
    ) -> Self {
        let current_request: Arc<std::sync::RwLock<Request>> = Default::default();
        let request_store = current_request.clone();

        let thread_handle = updater.spawn_error_handled(move || {
            let handle = rsvg::Loader::new().read_path(path)?;
            let renderer = rsvg::CairoRenderer::new(&handle);

            let (original_width, original_height) = svg_dimensions(&renderer);

            tiles.set_original_dimensions((original_width, original_height));

            loop {
                let tile_request = {
                    let mut request = request_store.write().unwrap();
                    let value = request.clone();
                    *request = Request::None;
                    value
                };

                match tile_request {
                    Request::None => {
                        std::thread::park();
                    }
                    Request::Exit => {
                        log::debug!("Terminating decoder thread.");
                        break;
                    }
                    Request::Tile(tile_request) => {
                        let tiling = tiling::Tiling {
                            tile_size: tiling::TILE_SIZE,
                            original_dimensions: (original_width, original_height),
                            zoom: tile_request.zoom,
                            bleed: 2,
                        };

                        let relevant_tiles = tiling.relevant_tiles(&tile_request.viewport);

                        for tile_instructions in relevant_tiles {
                            if request_store.read().unwrap().is_waiting() {
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

                            let context = cairo::Context::new(surface.clone())?;
                            let (total_width, total_height) =
                                tile_instructions.tiling.scaled_dimensions();

                            // librsvg does not currently support larger images
                            //
                            // TODO: We are only creating tiles, maybe this check in librsvg is wrong?
                            if total_height > 32_767 || total_width > 32_767 {
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

                            let tile = tiling::Tile {
                                position: (
                                    tile_instructions.area.x() as u32,
                                    tile_instructions.area.y() as u32,
                                ),
                                zoom_level: tiling::zoom_to_level(tile_instructions.tiling.zoom),
                                bleed: 2,
                                texture: decoded_image.into_texture(),
                            };

                            tiles.push(tile.clone());
                        }
                    }
                }
            }

            Ok(())
        });

        Self {
            thread_handle,
            current_request,
        }
    }

    pub fn request(&self, request: TileRequest, _abstract_decoder: &Decoder) -> anyhow::Result<()> {
        let mut current_request = self.current_request.write().unwrap();
        *current_request = Request::Tile(request);
        self.thread_handle.thread().unpark();

        Ok(())
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
    pub fn into_texture(self) -> gdk::Texture {
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

        let bytes = glib::Bytes::from_owned(self.surface.take_data().unwrap().to_vec());

        gdk::MemoryTexture::new(width, height, memory_format, &bytes, stride).upcast()
    }
}
