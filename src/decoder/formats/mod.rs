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

mod glycin_proxy;
mod svg;

pub use glycin_proxy::Glycin;
pub use svg::{Svg, RSVG_MAX_SIZE};

use super::{DecoderUpdate, UpdateSender};

#[derive(Clone, Debug)]
pub struct ImageFormat {
    mime_type: glycin::MimeType,
    name: String,
}

impl ImageFormat {
    pub fn new(mime_type: glycin::MimeType, name: String) -> Self {
        Self { mime_type, name }
    }

    pub fn is_svg(&self) -> bool {
        matches!(
            self.mime_type.as_str(),
            "image/svg+xml" | "image/svg+xml-compressed"
        )
    }

    pub fn is_potentially_transparent(&self) -> bool {
        !matches!(self.mime_type.as_str(), "image/bmp" | "image/jpeg")
    }
}

impl std::fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}", self.name)
    }
}

#[derive(Clone, Debug, Default)]
pub enum ImageDimensionDetails {
    Svg(String),
    #[default]
    None,
}
