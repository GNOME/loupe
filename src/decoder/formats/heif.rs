///! Decode using libheif-rs
use super::*;
use crate::decoder::tiling::{self, TilingStoreExt};
use crate::deps::*;
use crate::i18n::*;
use gtk::prelude::*;

use anyhow::Context;
use arc_swap::ArcSwap;
use libheif_rs::{ColorSpace, HeifContext, Plane, RgbChroma};

use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug)]
pub struct Heif;

impl Heif {
    pub fn new(
        path: PathBuf,
        updater: UpdateSender,
        tiles: Arc<ArcSwap<tiling::TilingStore>>,
    ) -> Self {
        log::debug!("Loading HEIF");
        updater.spawn_error_handled(move || {
            let ctx = HeifContext::read_from_file(&path.display().to_string())
                .context(i18n("Failed to read image"))?;
            let handle = ctx.primary_image_handle()?;

            tiles.set_original_dimensions((handle.width(), handle.height()));

            let rgb_chroma = if handle.luma_bits_per_pixel() > 8 {
                if handle.has_alpha_channel() {
                    #[cfg(target_endian = "little")]
                    {
                        RgbChroma::HdrRgbaLe
                    }
                    #[cfg(target_endian = "big")]
                    {
                        RgbChroma::HdrRgbaBe
                    }
                } else {
                    #[cfg(target_endian = "little")]
                    {
                        RgbChroma::HdrRgbLe
                    }
                    #[cfg(target_endian = "big")]
                    {
                        RgbChroma::HdrRgbBe
                    }
                }
            } else if handle.has_alpha_channel() {
                RgbChroma::Rgba
            } else {
                RgbChroma::Rgb
            };

            let mut image = handle
                .decode(ColorSpace::Rgb(rgb_chroma), None)
                .context(i18n("Failed to decode image"))?;

            let plane = image
                .planes_mut()
                .interleaved
                .context("Interleaved plane does not exist")?;

            let decoded = Decoded {
                plane,
                has_alpha_channel: handle.has_alpha_channel(),
                pre_multiplied_alpha: handle.is_premultiplied_alpha(),
                rgb_chroma,
            };

            let tile = tiling::Tile {
                position: (0, 0),
                zoom_level: tiling::zoom_to_level(1.),
                bleed: 0,
                texture: decoded.into_texture(),
            };

            tiles.push(tile);

            Ok(())
        });

        Heif
    }
}

pub struct Decoded<'a> {
    plane: Plane<&'a mut [u8]>,
    has_alpha_channel: bool,
    pre_multiplied_alpha: bool,
    rgb_chroma: RgbChroma,
}

impl<'a> Decoded<'a> {
    pub fn into_texture(self) -> gdk::Texture {
        let memory_format = match self.rgb_chroma {
            RgbChroma::HdrRgbBe
            | RgbChroma::HdrRgbaBe
            | RgbChroma::HdrRgbLe
            | RgbChroma::HdrRgbaLe => {
                let transmuted: &mut [u16] =
                    safe_transmute::transmute_many_pedantic_mut(self.plane.data).unwrap();

                // Scale HDR pixels to 16bit (they are usually 10bit or 12bit)
                for pixel in transmuted.iter_mut() {
                    *pixel <<= 16 - self.plane.bits_per_pixel;
                }

                if self.has_alpha_channel {
                    if self.pre_multiplied_alpha {
                        gdk::MemoryFormat::R16g16b16a16Premultiplied
                    } else {
                        gdk::MemoryFormat::R16g16b16a16
                    }
                } else {
                    gdk::MemoryFormat::R16g16b16
                }
            }
            RgbChroma::Rgb | RgbChroma::Rgba => {
                if self.has_alpha_channel {
                    if self.pre_multiplied_alpha {
                        gdk::MemoryFormat::R8g8b8a8Premultiplied
                    } else {
                        gdk::MemoryFormat::R8g8b8a8
                    }
                } else {
                    gdk::MemoryFormat::R8g8b8
                }
            }
            RgbChroma::C444 => unreachable!(),
        };

        let bytes = glib::Bytes::from_owned(self.plane.data.to_vec());

        let tex = gdk::MemoryTexture::new(
            self.plane.width as i32,
            self.plane.height as i32,
            memory_format,
            &bytes,
            self.plane.stride,
        );

        tex.upcast()
    }
}
