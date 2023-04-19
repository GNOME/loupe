// Copyright (c) 2022-2023 Sophie Herold
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

/*! Resources for working with EXIF Metadata

- <https://github.com/ianare/exif-samples>
*/

mod gps;
mod obj;

pub use gps::GPSLocation;
pub use obj::LpImageMetadata;

use crate::util::gettext::*;
use crate::util::{self, ToBufRead};

use crate::deps::*;
use gio::prelude::*;

#[derive(Default)]
pub enum ImageMetadata {
    Exif(exif::Exif),
    #[default]
    None,
}

impl std::fmt::Debug for ImageMetadata {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Self::Exif(exif) => {
                let list = exif.fields().map(|f| {
                    let mut value = f.display_value().to_string();
                    // Remove long values
                    if value.len() > 100 {
                        value = String::from("…");
                    }

                    (f.ifd_num.to_string(), f.tag.to_string(), value)
                });
                fmt.write_str("Exif")?;
                fmt.debug_list().entries(list).finish()
            }
            Self::None => fmt.write_str("None"),
        }
    }
}

impl ImageMetadata {
    pub fn load(file: &gio::File) -> Self {
        log::debug!("Loading metadata for {}", file.uri());
        if let Ok(mut bufreader) = file.to_buf_read() {
            let exifreader = exif::Reader::new();

            if let Ok(exif) = exifreader.read_from_container(&mut bufreader) {
                return Self::Exif(exif);
            }
        }

        Self::None
    }

    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    pub fn orientation(&self) -> Orientation {
        match self {
            Self::Exif(exif) => {
                if let Some(orientation) = exif
                    .get_field(exif::Tag::Orientation, exif::In::PRIMARY)
                    .and_then(|x| x.value.get_uint(0))
                {
                    Orientation::from(orientation)
                } else {
                    Orientation::default()
                }
            }
            Self::None => Orientation::default(),
        }
    }

    pub fn originally_created(&self) -> Option<String> {
        if let Self::Exif(exif) = self {
            if let Some(field) = exif
                .get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
                .or_else(|| exif.get_field(exif::Tag::DateTime, exif::In::PRIMARY))
            {
                if let exif::Value::Ascii(ascii_vec) = &field.value {
                    if let Some(ascii) = ascii_vec.first() {
                        if let Ok(dt) = exif::DateTime::from_ascii(ascii) {
                            if let Ok(datetime) = glib::DateTime::from_local(
                                dt.year.into(),
                                dt.month.into(),
                                dt.day.into(),
                                dt.hour.into(),
                                dt.minute.into(),
                                dt.second.into(),
                            ) {
                                return util::datetime_fmt(&datetime);
                            }
                        }
                    }
                }

                return Some(field.display_value().to_string());
            }
        }

        None
    }

    pub fn f_number(&self) -> Option<String> {
        if let Self::Exif(exif) = self {
            if let Some(field) = exif.get_field(exif::Tag::FNumber, exif::In::PRIMARY) {
                return Some(format!("ƒ\u{2215}{}", field.display_value()));
            }
        }

        None
    }

    pub fn exposure_time(&self) -> Option<String> {
        if let Self::Exif(exif) = self {
            let field = exif.get_field(exif::Tag::ExposureTime, exif::In::PRIMARY)?;
            if let exif::Value::Rational(rational) = &field.value {
                let exposure = format!("{:.0}", 1. / rational.first()?.to_f32());

                // Translators: Unit for exposure time in seconds
                return Some(gettext_f("1\u{2215}{}\u{202F}s", &[&exposure]));
            }
        }

        None
    }

    pub fn iso(&self) -> Option<String> {
        if let Self::Exif(exif) = self {
            if let Some(field) =
                exif.get_field(exif::Tag::PhotographicSensitivity, exif::In::PRIMARY)
            {
                return Some(field.display_value().to_string());
            }
        }

        None
    }

    pub fn focal_length(&self) -> Option<String> {
        if let Self::Exif(exif) = self {
            let field = exif.get_field(exif::Tag::FocalLength, exif::In::PRIMARY)?;
            if let exif::Value::Rational(rational) = &field.value {
                let length = format!("{:.0}", rational.first()?.to_f32());
                // Translators: Unit for focal length in millimeters
                return Some(gettext_f("{}\u{202F}mm", &[&length]));
            }
        }

        None
    }

    /// Combined maker and model info
    pub fn maker_model(&self) -> Option<String> {
        if let Some(mut model) = self.model() {
            if let Some(maker) = self.maker() {
                // This is to avoid doubling the maker name
                // Canon for example also puts "Canon" in the model as well
                // NIKON sometimes puts "NIKON" in model and "NIKON CORPORATION" in maker
                if model.split_whitespace().next() != maker.split_whitespace().next() {
                    model = format!("{maker} {model}");
                }
            }

            Some(model)
        } else {
            self.maker()
        }
    }

    pub fn model(&self) -> Option<String> {
        if let Self::Exif(exif) = self {
            if let Some(field) = exif.get_field(exif::Tag::Model, exif::In::PRIMARY) {
                if let exif::Value::Ascii(value) = &field.value {
                    if let Some(entry) = value.first() {
                        return Some(String::from_utf8_lossy(entry).to_string());
                    }
                }
            }
        }

        None
    }

    pub fn maker(&self) -> Option<String> {
        if let Self::Exif(exif) = self {
            if let Some(field) = exif.get_field(exif::Tag::Make, exif::In::PRIMARY) {
                if let exif::Value::Ascii(value) = &field.value {
                    if let Some(entry) = value.first() {
                        return Some(String::from_utf8_lossy(entry).to_string());
                    }
                }
            }
        }

        None
    }

    pub fn gps_location(&self) -> Option<GPSLocation> {
        if let Self::Exif(exif) = self {
            if let (Some(latitude), Some(latitude_ref), Some(longitude), Some(longitude_ref)) = (
                exif.get_field(exif::Tag::GPSLatitude, exif::In::PRIMARY),
                exif.get_field(exif::Tag::GPSLatitudeRef, exif::In::PRIMARY),
                exif.get_field(exif::Tag::GPSLongitude, exif::In::PRIMARY),
                exif.get_field(exif::Tag::GPSLongitudeRef, exif::In::PRIMARY),
            ) {
                if let (
                    exif::Value::Rational(latitude),
                    exif::Value::Ascii(latitude_ref),
                    exif::Value::Rational(longitude),
                    exif::Value::Ascii(longitude_ref),
                ) = (
                    &latitude.value,
                    &latitude_ref.value,
                    &longitude.value,
                    &longitude_ref.value,
                ) {
                    return GPSLocation::for_exif(latitude, latitude_ref, longitude, longitude_ref);
                }
            }
        }

        None
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Orientation {
    pub rotation: f64,
    pub mirrored: bool,
}

impl From<u32> for Orientation {
    fn from(number: u32) -> Self {
        match number {
            8 => Self {
                rotation: 90.,
                mirrored: false,
            },
            3 => Self {
                rotation: 180.,
                mirrored: false,
            },
            6 => Self {
                rotation: 270.,
                mirrored: false,
            },
            2 => Self {
                rotation: 0.,
                mirrored: true,
            },
            5 => Self {
                rotation: 90.,
                mirrored: true,
            },
            4 => Self {
                rotation: 180.,
                mirrored: true,
            },
            7 => Self {
                rotation: 270.,
                mirrored: true,
            },
            _ => Self::default(),
        }
    }
}
