mod heif;
mod image_rs_other;
mod svg;

pub use heif::Heif;
pub use image_rs_other::ImageRsOther;
pub use svg::{Svg, RSVG_MAX_SIZE};

use super::{Decoder, DecoderUpdate, UpdateSender};
use crate::i18n::*;

#[derive(Clone, Copy, Debug)]
pub enum ImageFormat {
    ImageRs(image_rs::ImageFormat),
    AnimatedGif,
    AnimatedWebP,
    AnimatedPng,
    // TODO: Add details about contained format
    Heif,
    Svg,
}

impl ImageFormat {
    pub fn is_animated(&self) -> bool {
        matches!(
            self,
            ImageFormat::AnimatedGif | ImageFormat::AnimatedWebP | ImageFormat::AnimatedPng
        )
    }

    pub fn is_svg(&self) -> bool {
        matches!(self, Self::Svg)
    }

    pub fn is_potentially_transparent(&self) -> bool {
        !matches!(
            self,
            Self::ImageRs(image_rs::ImageFormat::Bmp) | Self::ImageRs(image_rs::ImageFormat::Jpeg)
        )
    }
}

impl std::fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Self::ImageRs(format) => {
                let debug = format!("{:?}", format);
                if debug.len() <= 4 {
                    // uppercase abbreviations
                    write!(f, "{}", debug.to_uppercase())
                } else {
                    write!(f, "{}", debug)
                }
            }
            Self::AnimatedGif => write!(f, "{}", i18n("Animated GIF")),
            Self::AnimatedWebP => write!(f, "{}", i18n("Animated WebP")),
            Self::AnimatedPng => write!(f, "{}", i18n("Animated PNG")),
            Self::Heif => write!(f, "HEIF Container"),
            Self::Svg => write!(f, "SVG"),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum ImageDimensionDetails {
    Svg((rsvg::Length, rsvg::Length)),
    #[default]
    None,
}
