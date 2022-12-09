use gtk::prelude::*;
use gtk::{gdk, glib};
use lcms2::*;

pub enum DecodedImage {
    Luma8(image::GrayImage),
    LumaA8(image::GrayAlphaImage),
    Rgb8(image::RgbImage),
    Rgba8(image::RgbaImage),
    Luma16(image::ImageBuffer<image::Luma<u16>, Vec<u16>>),
    LumaA16(image::ImageBuffer<image::LumaA<u16>, Vec<u16>>),
    Rgb16(image::ImageBuffer<image::Rgb<u16>, Vec<u16>>),
    Rgba16(image::ImageBuffer<image::Rgba<u16>, Vec<u16>>),
}

impl std::fmt::Debug for DecodedImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let sample_layout = self.sample_layout();
        write!(
            f,
            "{} ({}x{})",
            self.color_name(),
            sample_layout.width,
            sample_layout.height
        )
    }
}

impl From<image::DynamicImage> for DecodedImage {
    fn from(image: image::DynamicImage) -> Self {
        match image {
            image::DynamicImage::ImageLuma8(buffer) => Self::Luma8(buffer),
            image::DynamicImage::ImageLumaA8(buffer) => Self::LumaA8(buffer),
            image::DynamicImage::ImageRgb8(buffer) => Self::Rgb8(buffer),
            image::DynamicImage::ImageRgba8(buffer) => Self::Rgba8(buffer),
            image::DynamicImage::ImageLuma16(buffer) => Self::Luma16(buffer),
            image::DynamicImage::ImageLumaA16(buffer) => Self::LumaA16(buffer),
            image::DynamicImage::ImageRgb16(buffer) => Self::Rgb16(buffer),
            image::DynamicImage::ImageRgba16(buffer) => Self::Rgba16(buffer),
            img @ image::DynamicImage::ImageRgb32F(_) => Self::Rgb16(img.into_rgb16()),
            img @ image::DynamicImage::ImageRgba32F(_) => Self::Rgba16(img.into_rgba16()),
            img => Self::Rgba16(img.into_rgba16()),
        }
    }
}

type DynImg = image::DynamicImage;

use rgb::AsPixels;
impl DecodedImage {
    pub fn apply_icc_profile(&mut self, icc_profile: &[u8]) -> anyhow::Result<()> {
        match self {
            Self::Luma8(buffer) => {
                let mut new_buffer = rgb8_buf(buffer);
                new_trafo::<rgb::alt::GRAY8, rgb::RGB8>(icc_profile, lcms2::PixelFormat::GRAY_8)?
                    .transform_pixels(buffer.as_pixels(), new_buffer.as_pixels_mut());
                *self = Self::Rgb8(new_buffer);
            }
            Self::LumaA8(buffer) => {
                let mut new_buffer = rgba8_buf(buffer);
                new_trafo::<rgb::alt::GRAYA8, rgb::RGB8>(icc_profile, lcms2::PixelFormat::GRAYA_8)?
                    .transform_pixels(buffer.as_pixels(), new_buffer.as_pixels_mut());
                *self = Self::Rgba8(new_buffer);
            }
            Self::Rgb8(ref mut buffer) => {
                new_trafo::<rgb::RGB8, rgb::RGB8>(icc_profile, lcms2::PixelFormat::RGB_8)?
                    .transform_in_place(buffer.as_pixels_mut());
            }
            Self::Rgba8(ref mut buffer) => {
                new_trafo::<rgb::RGBA8, rgb::RGBA8>(icc_profile, lcms2::PixelFormat::RGBA_8)?
                    .transform_in_place(buffer.as_pixels_mut());
            }
            Self::Luma16(buffer) => {
                let mut new_buffer = rgb8_buf(buffer);
                new_trafo::<rgb::alt::GRAY16, rgb::RGB8>(icc_profile, lcms2::PixelFormat::GRAY_16)?
                    .transform_pixels(buffer.as_pixels(), new_buffer.as_pixels_mut());
                *self = Self::Rgb8(new_buffer);
            }
            Self::LumaA16(buffer) => {
                let mut new_buffer = rgba8_buf(buffer);
                new_trafo::<rgb::alt::GRAYA16, rgb::RGB8>(
                    icc_profile,
                    lcms2::PixelFormat::GRAYA_16,
                )?
                .transform_pixels(buffer.as_pixels(), new_buffer.as_pixels_mut());
                *self = Self::Rgba8(new_buffer);
            }
            Self::Rgb16(buffer) => {
                let mut new_buffer = rgb8_buf(buffer);
                new_trafo::<rgb::RGB16, rgb::RGB8>(icc_profile, lcms2::PixelFormat::RGB_16)?
                    .transform_pixels(buffer.as_pixels(), new_buffer.as_pixels_mut());
                *self = Self::Rgb8(new_buffer);
            }
            Self::Rgba16(buffer) => {
                let mut new_buffer = rgba8_buf(buffer);
                new_trafo::<rgb::RGBA16, rgb::RGB8>(icc_profile, lcms2::PixelFormat::RGBA_16)?
                    .transform_pixels(buffer.as_pixels(), new_buffer.as_pixels_mut());
                *self = Self::Rgba8(new_buffer);
            }
        }

        Ok(())
    }

    pub fn sample_layout(&self) -> image::flat::SampleLayout {
        match self {
            Self::Luma8(buffer) => buffer.sample_layout(),
            Self::LumaA8(buffer) => buffer.sample_layout(),
            Self::Rgb8(buffer) => buffer.sample_layout(),
            Self::Rgba8(buffer) => buffer.sample_layout(),
            Self::Luma16(buffer) => buffer.sample_layout(),
            Self::LumaA16(buffer) => buffer.sample_layout(),
            Self::Rgb16(buffer) => buffer.sample_layout(),
            Self::Rgba16(buffer) => buffer.sample_layout(),
        }
    }

    pub fn color_name(&self) -> String {
        match self {
            Self::Luma8(_) => "Grayscale",
            Self::LumaA8(_) => "Grayscale Alpha",
            Self::Rgb8(_) => "RGB",
            Self::Rgba8(_) => "RGBA",
            Self::Luma16(_) => "Grayscale 16 bit",
            Self::LumaA16(_) => "Grayscale Alpha 16 bit",
            Self::Rgb16(_) => "RGB 16 bit",
            Self::Rgba16(_) => "RGBA 16 bit",
        }
        .into()
    }

    pub fn into_texture(self) -> gdk::Texture {
        log::debug!("Creating texture for {self:?}");

        let sample_layout = self.sample_layout();

        let (memory_format, pixels) = match self {
            Self::Luma8(buffer) => (
                gdk::MemoryFormat::R8g8b8,
                DynImg::from(DynImg::from(buffer).into_rgb8()).into_bytes(),
            ),
            Self::LumaA8(buffer) => (
                gdk::MemoryFormat::R8g8b8a8,
                DynImg::from(DynImg::from(buffer).into_rgba8()).into_bytes(),
            ),
            Self::Rgb8(buffer) => (gdk::MemoryFormat::R8g8b8, DynImg::from(buffer).into_bytes()),
            Self::Rgba8(buffer) => (
                gdk::MemoryFormat::R8g8b8a8,
                DynImg::from(buffer).into_bytes(),
            ),
            Self::Luma16(buffer) => (
                gdk::MemoryFormat::R8g8b8,
                DynImg::from(DynImg::from(buffer).into_rgb8()).into_bytes(),
            ),
            Self::LumaA16(buffer) => (
                gdk::MemoryFormat::R8g8b8a8,
                DynImg::from(DynImg::from(buffer).into_rgba8()).into_bytes(),
            ),
            Self::Rgb16(buffer) => (
                gdk::MemoryFormat::R8g8b8,
                DynImg::from(DynImg::from(buffer).into_rgb8()).into_bytes(),
            ),
            Self::Rgba16(buffer) => (
                gdk::MemoryFormat::R8g8b8a8,
                DynImg::from(DynImg::from(buffer).into_rgba8()).into_bytes(),
            ),
        };

        let bytes = glib::Bytes::from_owned(std::borrow::Cow::from(pixels));

        gdk::MemoryTexture::new(
            sample_layout.width as i32,
            sample_layout.height as i32,
            memory_format,
            &bytes,
            sample_layout.height_stride,
        )
        .upcast()
    }
}

pub fn rgb8_buf<P: image::Pixel, Container: std::ops::Deref<Target = [P::Subpixel]>>(
    buffer: &image::ImageBuffer<P, Container>,
) -> image::RgbImage {
    image::DynamicImage::new_rgb8(buffer.width(), buffer.height()).into_rgb8()
}

pub fn rgba8_buf<P: image::Pixel, Container: std::ops::Deref<Target = [P::Subpixel]>>(
    buffer: &image::ImageBuffer<P, Container>,
) -> image::RgbaImage {
    image::DynamicImage::new_rgba8(buffer.width(), buffer.height()).into_rgba8()
}

pub fn new_trafo<I: Copy, O: Copy>(
    icc_profile: &[u8],
    src_format: lcms2::PixelFormat,
) -> anyhow::Result<lcms2::Transform<I, O>> {
    let src_profile = Profile::new_icc(icc_profile)?;

    let target_profile = lcms2::Profile::new_srgb();
    let target_format = lcms2::PixelFormat::RGB_8;

    Ok(lcms2::Transform::new(
        &src_profile,
        src_format,
        &target_profile,
        target_format,
        lcms2::Intent::Perceptual,
    )?)
}
