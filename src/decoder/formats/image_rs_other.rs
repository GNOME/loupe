///! Decode using image-rs
use super::*;
use crate::decoder::tiling::{self, TilingStoreExt};
use crate::deps::*;

use anyhow::Context;
use arc_swap::ArcSwap;
use gtk::prelude::*;

use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug)]
pub struct ImageRsOther;

impl ImageRsOther {
    pub fn new(
        path: PathBuf,
        updater: UpdateSender,
        tiles: Arc<ArcSwap<tiling::TilingStore>>,
    ) -> Self {
        updater.spawn_error_handled(move || {
            let dimensions = image_rs::image_dimensions(&path).context("Failed to read image")?;

            tiles.set_original_dimensions(dimensions);

            let mut reader = image_rs::io::Reader::open(&path).context("Failed to open image")?;

            // TODO: Set something huge?
            reader.limits(image_rs::io::Limits::no_limits());

            let reader = reader
                .with_guessed_format()
                .context("Failed to open image")?;
            let dynamic_image = reader.decode().context("Failed to decode image")?;

            let decoded_image = Decoded { dynamic_image };

            let tile = tiling::Tile {
                position: (0, 0),
                zoom_level: tiling::zoom_to_level(1.),
                bleed: 0,
                texture: decoded_image.into_texture(),
            };

            tiles.push(tile);

            Ok(())
        });

        ImageRsOther
    }
}

pub struct Decoded {
    dynamic_image: image_rs::DynamicImage,
}

impl Decoded {
    pub fn into_texture(self) -> gdk::Texture {
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
            image_rs::DynamicImage::ImageRgb8(buffer) => (
                1,
                gdk::MemoryFormat::R8g8b8,
                buffer.sample_layout(),
                buffer.into_raw(),
            ),
            image_rs::DynamicImage::ImageRgba8(buffer) => (
                1,
                gdk::MemoryFormat::R8g8b8a8,
                buffer.sample_layout(),
                buffer.into_raw(),
            ),
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
                    safe_transmute::transmute_vec(buffer.into_raw()).unwrap(),
                )
            }
        };

        let bytes = glib::Bytes::from_owned(data);

        gdk::MemoryTexture::new(
            layout.width as i32,
            layout.height as i32,
            memory_format,
            &bytes,
            layout.height_stride * n_bytes,
        )
        .upcast()
    }
}
