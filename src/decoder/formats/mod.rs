mod heif;
mod image_rs_other;
mod svg;

pub use heif::Heif;
pub use image_rs_other::ImageRsOther;
pub use svg::{Svg, RSVG_MAX_SIZE};

use super::{Decoder, UpdateSender};

#[derive(Clone, Copy, Debug)]
pub enum ImageFormat {
    ImageRs(image_rs::ImageFormat),
    // TODO: Add details about contained format
    Heif,
    Svg,
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
            Self::Heif => write!(f, "HEIF Container"),
            Self::Svg => write!(f, "SVG"),
        }
    }
}
