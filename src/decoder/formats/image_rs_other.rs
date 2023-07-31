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

///! Decode using image-rs
use super::*;
use crate::decoder::tiling::{self, FrameBufferExt};
use crate::deps::*;
use crate::util;
use crate::util::gettext::*;
use crate::util::{BufReadSeek, ToBufRead};

use anyhow::Context;
use arc_swap::ArcSwap;
use gtk::prelude::*;
use image_rs::{AnimationDecoder, ImageDecoder};
use rgb::AsPixels;

use std::sync::Arc;

#[derive(Debug)]
pub struct ImageRsOther {
    thread_handle: Option<std::thread::JoinHandle<()>>,
    cancellable: gio::Cancellable,
}

impl Drop for ImageRsOther {
    fn drop(&mut self) {
        if let Some(handle) = &self.thread_handle {
            self.cancellable.cancel();
            handle.thread().unpark();
        }
    }
}

impl ImageRsOther {
    fn reader(
        file: &gio::File,
        format: image_rs::ImageFormat,
        cancellable: &gio::Cancellable,
    ) -> anyhow::Result<image_rs::io::Reader<Box<dyn BufReadSeek>>> {
        let buf_read = file.to_buf_read(cancellable)?;
        let mut reader = image_rs::io::Reader::new(buf_read);
        reader.set_format(format);
        Ok(reader)
    }

    pub fn new(
        file: gio::File,
        format: image_rs::ImageFormat,
        updater: UpdateSender,
        tiles: Arc<ArcSwap<tiling::FrameBuffer>>,
    ) -> Self {
        let cancellable = gio::Cancellable::new();

        let thread_handle = match format {
            image_rs::ImageFormat::Gif
            | image_rs::ImageFormat::WebP
            | image_rs::ImageFormat::Png => Some(Self::new_animated(
                file,
                format,
                updater,
                tiles,
                cancellable.clone(),
            )),
            _ => {
                Self::new_static(file, format, updater, tiles, cancellable.clone());
                None
            }
        };

        ImageRsOther {
            thread_handle,
            cancellable,
        }
    }

    fn new_static(
        file: gio::File,
        format: image_rs::ImageFormat,
        updater: UpdateSender,
        tiles: Arc<ArcSwap<tiling::FrameBuffer>>,
        cancellable: gio::Cancellable,
    ) {
        updater.spawn_error_handled(move || {
            let reader = Self::reader(&file, format, &cancellable)?;
            let dimensions = reader
                .into_dimensions()
                .context("Failed to read image dimensions")?;

            tiles.set_original_dimensions(dimensions);

            let (icc_profile, dynamic_image) = match format {
                image_rs::ImageFormat::Jpeg => {
                    let reader = file.to_buf_read(&cancellable)?;
                    let mut decoder = image_rs::codecs::jpeg::JpegDecoder::new(reader)?;
                    let _ = decoder.set_limits(image_rs::io::Limits::no_limits());
                    let icc_profile = decoder.icc_profile();
                    let dynamic_image = image_rs::DynamicImage::from_decoder(decoder)
                        .context(gettext("Failed to decode image"))?;
                    (icc_profile, dynamic_image)
                }
                image_rs::ImageFormat::Png => {
                    let reader = file.to_buf_read(&cancellable)?;
                    let mut decoder = image_rs::codecs::png::PngDecoder::new(reader)?;
                    let _ = decoder.set_limits(image_rs::io::Limits::no_limits());
                    let icc_profile = decoder.icc_profile();
                    let dynamic_image = image_rs::DynamicImage::from_decoder(decoder)
                        .context(gettext("Failed to decode image"))?;
                    (icc_profile, dynamic_image)
                }
                _ => {
                    let mut reader = Self::reader(&file, format, &cancellable)?;
                    reader.limits(image_rs::io::Limits::no_limits());
                    let dynamic_image =
                        reader.decode().context(gettext("Failed to decode image"))?;
                    (None, dynamic_image)
                }
            };

            let decoded_image = Decoded {
                dynamic_image,
                icc_profile,
            };

            let tile = tiling::Tile {
                position: (0, 0),
                zoom_level: tiling::zoom_to_level(1.),
                bleed: 0,
                texture: decoded_image.into_texture()?,
            };

            tiles.push(tile);

            Ok(())
        });
    }

    fn new_animated(
        file: gio::File,
        format: image_rs::ImageFormat,
        updater: UpdateSender,
        tiles: Arc<ArcSwap<tiling::FrameBuffer>>,
        cancellable: gio::Cancellable,
    ) -> std::thread::JoinHandle<()> {
        updater.clone().spawn_error_handled(move || {
            let mut nth_frame = 1;

            // We are currently decoding for each repetition of the animation
            // TODO: Check if/how that can be solved differently
            loop {
                let buf_reader = file.to_buf_read(&cancellable)?;

                let (frames, animated_format) = match format {
                    image_rs::ImageFormat::Gif => (
                        image_rs::codecs::gif::GifDecoder::new(buf_reader)?.into_frames(),
                        ImageFormat::AnimatedGif,
                    ),
                    image_rs::ImageFormat::WebP => {
                        let decoder = image_rs::codecs::webp::WebPDecoder::new(buf_reader)?;
                        if decoder.has_animation() {
                            (decoder.into_frames(), ImageFormat::AnimatedWebP)
                        } else {
                            // Static WebP images need a different decoder
                            Self::new_static(file, format, updater, tiles, cancellable);
                            return Ok(());
                        }
                    }
                    image_rs::ImageFormat::Png => {
                        let decoder = image_rs::codecs::png::PngDecoder::new(buf_reader)?;
                        if decoder.is_apng() {
                            (decoder.apng().into_frames(), ImageFormat::AnimatedPng)
                        } else {
                            // Static PNG images need a different decoder
                            Self::new_static(file, format, updater, tiles, cancellable);
                            return Ok(());
                        }
                    }
                    _ => todo!(),
                };

                for frame in frames {
                    match frame {
                        Err(err) => {
                            log::warn!("Skipping frame: {err}");
                        }
                        Ok(frame) => {
                            if nth_frame == 2 {
                                // We have at least two frames so this is animated
                                updater.send(DecoderUpdate::Format(animated_format));
                                nth_frame = 3;
                            }

                            let (delay_num, delay_den) = frame.delay().numer_denom_ms();
                            let delay = std::time::Duration::from_micros(f64::round(
                                delay_num as f64 * 1000. / delay_den as f64,
                            )
                                as u64);
                            let position = (frame.left(), frame.top());
                            let rgb_image = frame.into_buffer();
                            let dimensions = (
                                rgb_image.width() + position.0,
                                rgb_image.height() + position.1,
                            );

                            let dynamic_image = image_rs::DynamicImage::from(rgb_image);

                            let decoded_image = Decoded {
                                dynamic_image,
                                icc_profile: None,
                            };

                            let tile = tiling::Tile {
                                position,
                                zoom_level: tiling::zoom_to_level(1.),
                                bleed: 0,
                                texture: decoded_image.into_texture()?,
                            };

                            tiles.push_frame(tile, dimensions, delay);

                            if nth_frame == 1 {
                                updater.send(DecoderUpdate::Dimensions(Default::default()));
                                updater.send(DecoderUpdate::Redraw);
                                nth_frame = 2;
                            }
                        }
                    }

                    if tiles.n_frames() >= 3 {
                        std::thread::park();
                    }

                    if cancellable.is_cancelled() {
                        log::debug!("Terminating decoder thread.");
                        return Ok(());
                    }
                }
            }
        })
    }

    pub fn fill_frame_buffer(&self) {
        if let Some(handle) = &self.thread_handle {
            handle.thread().unpark();
        } else {
            log::error!("Trying to wake up one-shot decoder");
        }
    }
}

pub struct Decoded {
    dynamic_image: image_rs::DynamicImage,
    icc_profile: Option<Vec<u8>>,
}

impl Decoded {
    pub fn into_texture(self) -> anyhow::Result<gdk::Texture> {
        let (n_bytes, memory_format, layout, data) = match self.dynamic_image {
            img @ image_rs::DynamicImage::ImageLuma8(_) => {
                let buffer = img.to_rgb8();
                (
                    1,
                    gdk::MemoryFormat::R8g8b8,
                    buffer.sample_layout(),
                    buffer.into_raw(),
                )
            }
            img @ image_rs::DynamicImage::ImageLumaA8(_) => {
                let buffer = img.to_rgba8();
                (
                    1,
                    gdk::MemoryFormat::R8g8b8a8,
                    buffer.sample_layout(),
                    buffer.into_raw(),
                )
            }
            image_rs::DynamicImage::ImageRgb8(mut buffer) => {
                if let Some(icc_profile) = &self.icc_profile {
                    util::appply_icc_profile::<rgb::RGB8>(
                        icc_profile,
                        lcms2::PixelFormat::RGB_8,
                        buffer.as_pixels_mut(),
                    );
                }
                (
                    1,
                    gdk::MemoryFormat::R8g8b8,
                    buffer.sample_layout(),
                    buffer.into_raw(),
                )
            }
            image_rs::DynamicImage::ImageRgba8(mut buffer) => {
                if let Some(icc_profile) = &self.icc_profile {
                    util::appply_icc_profile::<rgb::RGBA8>(
                        icc_profile,
                        lcms2::PixelFormat::RGBA_8,
                        buffer.as_pixels_mut(),
                    );
                }
                (
                    1,
                    gdk::MemoryFormat::R8g8b8a8,
                    buffer.sample_layout(),
                    buffer.into_raw(),
                )
            }
            img @ image_rs::DynamicImage::ImageLuma16(_) => {
                let buffer = img.to_rgb16();
                (
                    2,
                    gdk::MemoryFormat::R16g16b16,
                    buffer.sample_layout(),
                    image_rs::DynamicImage::from(buffer).into_bytes(),
                )
            }
            img @ image_rs::DynamicImage::ImageLumaA16(_) => {
                let buffer = img.to_rgba16();
                (
                    2,
                    gdk::MemoryFormat::R16g16b16a16,
                    buffer.sample_layout(),
                    image_rs::DynamicImage::from(buffer).into_bytes(),
                )
            }
            image_rs::DynamicImage::ImageRgb16(buffer) => (
                2,
                gdk::MemoryFormat::R16g16b16,
                buffer.sample_layout(),
                image_rs::DynamicImage::from(buffer).into_bytes(),
            ),
            image_rs::DynamicImage::ImageRgba16(buffer) => (
                2,
                gdk::MemoryFormat::R16g16b16a16,
                buffer.sample_layout(),
                image_rs::DynamicImage::from(buffer).into_bytes(),
            ),
            image_rs::DynamicImage::ImageRgb32F(buffer) => (
                4,
                gdk::MemoryFormat::R32g32b32Float,
                buffer.sample_layout(),
                image_rs::DynamicImage::from(buffer).into_bytes(),
            ),
            image_rs::DynamicImage::ImageRgba32F(buffer) => (
                4,
                gdk::MemoryFormat::R32g32b32a32Float,
                buffer.sample_layout(),
                image_rs::DynamicImage::from(buffer).into_bytes(),
            ),
            img => {
                let buffer = img.to_rgba16();
                (
                    2,
                    gdk::MemoryFormat::R16g16b16a16,
                    buffer.sample_layout(),
                    safe_transmute::transmute_vec(buffer.into_raw())
                        .context("Failed to transform into 16 bit texture")?,
                )
            }
        };

        let bytes = glib::Bytes::from_owned(data);

        let texture = gdk::MemoryTexture::new(
            layout.width as i32,
            layout.height as i32,
            memory_format,
            &bytes,
            layout.height_stride * n_bytes,
        )
        .upcast();

        Ok(texture)
    }
}
