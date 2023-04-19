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

mod heif;
mod image_rs_other;
mod svg;

pub use heif::Heif;
pub use image_rs_other::ImageRsOther;
pub use svg::{Svg, RSVG_MAX_SIZE};

use super::{DecoderUpdate, UpdateSender};
use crate::util::gettext::*;

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
            Self::AnimatedGif => write!(f, "{}", gettext("Animated GIF")),
            Self::AnimatedWebP => write!(f, "{}", gettext("Animated WebP")),
            Self::AnimatedPng => write!(f, "{}", gettext("Animated PNG")),
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
