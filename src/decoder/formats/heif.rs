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

///! Decode using libheif-rs
use super::*;
use crate::decoder::tiling::{self, FrameBufferExt};
use crate::deps::*;
use crate::util;
use crate::util::gettext::*;
use crate::util::ToBufRead;

use anyhow::Context;
use arc_swap::ArcSwap;
use gtk::prelude::*;
use libheif_rs::{ColorProfile, ColorSpace, HeifContext, LibHeif, Plane, RgbChroma};
use once_cell::sync::Lazy;
use rgb::AsPixels;

use std::sync::Arc;

#[derive(Debug)]
pub struct Heif;

/// Loads plugins on first use
static LIBHEIF: Lazy<LibHeif> = Lazy::new(LibHeif::new);

impl Heif {
    pub fn new(
        file: gio::File,
        updater: UpdateSender,
        tiles: Arc<ArcSwap<tiling::FrameBuffer>>,
    ) -> Self {
        log::debug!("Loading HEIF");
        updater.spawn_error_handled(move || {
            let file_size = file
                .query_info(
                    gio::FILE_ATTRIBUTE_STANDARD_SIZE,
                    gio::FileQueryInfoFlags::NONE,
                    gio::Cancellable::NONE,
                )
                .context(gettext("Failed to read image file information"))?
                .size();
            let buf_reader = file.to_buf_read()?;
            let stream_reader = libheif_rs::StreamReader::new(buf_reader, file_size as u64);
            let ctx = HeifContext::read_from_reader(Box::new(stream_reader))
                .context(gettext("Failed to decode image"))?;
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

            let mut image = LIBHEIF
                .decode(&handle, ColorSpace::Rgb(rgb_chroma), None)
                .context(gettext("Failed to decode image"))?;

            let icc_profile = if let Some(profile) = handle.color_profile_raw() {
                if [
                    libheif_rs::color_profile_types::R_ICC,
                    libheif_rs::color_profile_types::PROF,
                ]
                .contains(&profile.profile_type())
                {
                    Some(profile.data)
                } else {
                    None
                }
            } else {
                None
            };

            let plane = image
                .planes_mut()
                .interleaved
                .context("Interleaved plane does not exist")?;

            let decoded = Decoded {
                plane,
                has_alpha_channel: handle.has_alpha_channel(),
                pre_multiplied_alpha: handle.is_premultiplied_alpha(),
                rgb_chroma,
                icc_profile,
            };

            let tile = tiling::Tile {
                position: (0, 0),
                zoom_level: tiling::zoom_to_level(1.),
                bleed: 0,
                texture: decoded.into_texture()?,
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
    icc_profile: Option<Vec<u8>>,
}

impl<'a> Decoded<'a> {
    pub fn into_texture(self) -> anyhow::Result<gdk::Texture> {
        let memory_format = match self.rgb_chroma {
            RgbChroma::HdrRgbBe
            | RgbChroma::HdrRgbaBe
            | RgbChroma::HdrRgbLe
            | RgbChroma::HdrRgbaLe => {
                if let Ok(transmuted) =
                    safe_transmute::transmute_many_pedantic_mut::<u16>(self.plane.data)
                {
                    // Scale HDR pixels to 16bit (they are usually 10bit or 12bit)
                    for pixel in transmuted.iter_mut() {
                        *pixel <<= 16 - self.plane.bits_per_pixel;
                    }
                } else {
                    log::error!("Could not transform HDR (16bit) data to u16");
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

        let mut buffer = self.plane.data.to_vec();

        if let Some(icc_profile) = &self.icc_profile {
            if memory_format == gdk::MemoryFormat::R8g8b8 {
                util::appply_icc_profile::<rgb::RGB8>(
                    icc_profile,
                    lcms2::PixelFormat::RGB_8,
                    buffer.as_pixels_mut(),
                );
            } else if memory_format == gdk::MemoryFormat::R8g8b8a8 {
                util::appply_icc_profile::<rgb::RGBA8>(
                    icc_profile,
                    lcms2::PixelFormat::RGBA_8,
                    buffer.as_pixels_mut(),
                );
            }
        }

        let bytes = glib::Bytes::from_owned(buffer);

        let tex = gdk::MemoryTexture::new(
            self.plane.width as i32,
            self.plane.height as i32,
            memory_format,
            &bytes,
            self.plane.stride,
        );

        Ok(tex.upcast())
    }
}
