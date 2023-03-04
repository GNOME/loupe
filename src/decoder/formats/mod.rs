mod heif;
mod image_rs_other;
mod svg;

pub use heif::Heif;
pub use image_rs_other::ImageRsOther;
pub use svg::Svg;

use super::{Decoder, UpdateSender};
