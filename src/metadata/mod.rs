// Copyright (c) 2022-2024 Sophie Herold
// Copyright (c) 2023 FineFindus
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

mod file;
mod gps;

pub use file::FileInfo;
use glycin_utils::{FrameDetails, ImageInfoDetails};
pub use gps::GPSLocation;

use crate::deps::*;
use crate::util;
use crate::util::gettext::*;

#[derive(Default)]
pub struct Metadata {
    mime_type: Option<String>,
    exif: Option<exif::Exif>,
    file_info: Option<FileInfo>,
    // TODO: Replace with glycin in newer glycin version
    image_info: Option<glycin_utils::ImageInfoDetails>,
    frame_info: Option<FrameDetails>,
}

impl std::fmt::Debug for Metadata {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        if let Some(exif) = &self.exif {
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
        } else {
            fmt.write_str("Empty")
        }
    }
}

impl Metadata {
    fn set_exif_bytes(&mut self, bytes: Vec<u8>) {
        let reader = exif::Reader::new();
        let exif = reader.read_raw(bytes);

        if let Err(err) = &exif {
            log::warn!("Failed to decode EXIF bytes: {err}");
        }

        self.exif = exif.ok();
    }

    pub fn set_image_info(&mut self, image_info: ImageInfoDetails) {
        if let Some(exif_raw) = image_info.exif.as_ref() {
            self.set_exif_bytes(exif_raw.clone());
        }
        self.image_info = Some(image_info);
    }

    pub fn set_frame_info(&mut self, frame_info: FrameDetails) {
        self.frame_info = Some(frame_info);
    }

    pub fn set_file_info(&mut self, file_info: FileInfo) {
        self.file_info = Some(file_info);
    }

    pub fn set_mime_type(&mut self, mime_type: String) {
        self.mime_type = Some(mime_type);
    }

    pub fn merge(&mut self, other: Self) {
        if self.exif.is_none() {
            self.exif = other.exif;
        }
        if self.file_info.is_none() {
            self.file_info = other.file_info;
        }
        if self.image_info.is_none() {
            self.image_info = other.image_info;
        }
        if self.mime_type.is_none() {
            self.mime_type = other.mime_type;
        }
        if self.frame_info.is_none() {
            self.frame_info = other.frame_info;
        }
    }

    pub fn mime_type(&self) -> Option<String> {
        self.file_info
            .as_ref()
            .and_then(|x| x.mime_type.as_ref())
            .map(|x| x.to_string())
    }

    pub fn format_name(&self) -> Option<String> {
        self.image_info
            .as_ref()
            .and_then(|x| x.format_name.clone())
            .or_else(|| self.mime_type())
    }

    pub fn alpha_channel(&self) -> Option<bool> {
        self.frame_info.as_ref().and_then(|x| x.alpha_channel)
    }

    pub fn grayscale(&self) -> Option<bool> {
        self.frame_info.as_ref().and_then(|x| x.grayscale)
    }

    pub fn bit_depth(&self) -> Option<u8> {
        self.frame_info.as_ref().and_then(|x| x.bit_depth)
    }

    pub fn is_svg(&self) -> bool {
        matches!(
            self.mime_type.as_deref(),
            Some("image/svg+xml") | Some("image/svg+xml-compressed")
        )
    }

    pub fn is_potentially_transparent(&self) -> bool {
        // TODO: Implement again
        true
    }

    pub fn transformations_applied(&self) -> bool {
        self.image_info
            .as_ref()
            .map(|x| x.transformations_applied)
            .unwrap_or(false)
    }

    pub fn orientation(&self) -> Orientation {
        if self.transformations_applied() {
            // HEIF library already does it's transformations on its own
            Orientation::default()
        } else if let Some(exif) = &self.exif {
            if let Some(orientation) = exif
                .get_field(exif::Tag::Orientation, exif::In::PRIMARY)
                .and_then(|x| x.value.get_uint(0))
            {
                Orientation::from(orientation)
            } else {
                Orientation::default()
            }
        } else {
            Orientation::default()
        }
    }

    pub fn dimensions_inch(&self) -> Option<(f64, f64)> {
        self.image_info.as_ref().and_then(|x| x.dimensions_inch)
    }

    pub fn dimensions_text(&self) -> Option<String> {
        self.image_info
            .as_ref()
            .and_then(|x| x.dimensions_text.clone())
    }

    pub fn file_name(&self) -> Option<String> {
        self.file_info.as_ref().map(|x| x.display_name.to_string())
    }

    pub fn file_size(&self) -> Option<String> {
        self.file_info
            .as_ref()
            .and_then(|x| x.file_size)
            .map(|x| glib::format_size(x).to_string())
    }

    pub fn file_created(&self) -> Option<String> {
        self.file_info
            .as_ref()
            .and_then(|x| x.created.as_ref())
            .and_then(util::datetime_fmt)
    }

    pub fn file_modified(&self) -> Option<String> {
        self.file_info
            .as_ref()
            .and_then(|x| x.modified.as_ref())
            .and_then(util::datetime_fmt)
    }

    pub fn originally_created(&self) -> Option<String> {
        if let Some(exif) = &self.exif {
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
        if let Some(field) = self
            .exif
            .as_ref()
            .and_then(|exif| exif.get_field(exif::Tag::FNumber, exif::In::PRIMARY))
        {
            return Some(format!("ƒ\u{2215}{}", field.display_value()));
        }

        None
    }

    pub fn exposure_time(&self) -> Option<String> {
        if let Some(exif) = &self.exif {
            let field = exif.get_field(exif::Tag::ExposureTime, exif::In::PRIMARY)?;
            if let exif::Value::Rational(rational) = &field.value {
                let exposure = format!("{:.0}", 1. / rational.first()?.to_f32());

                // Translators: Unit for exposure time in seconds
                return Some(gettext_f("1\u{2215}{}\u{202F}s", [exposure]));
            }
        }

        None
    }

    pub fn iso(&self) -> Option<String> {
        if let Some(exif) = &self.exif {
            if let Some(field) =
                exif.get_field(exif::Tag::PhotographicSensitivity, exif::In::PRIMARY)
            {
                return Some(field.display_value().to_string());
            }
        }

        None
    }

    pub fn focal_length(&self) -> Option<String> {
        if let Some(exif) = &self.exif {
            let field = exif.get_field(exif::Tag::FocalLength, exif::In::PRIMARY)?;
            if let exif::Value::Rational(rational) = &field.value {
                let length = format!("{:.0}", rational.first()?.to_f32());
                // Translators: Unit for focal length in millimeters
                return Some(gettext_f("{}\u{202F}mm", [length]));
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
        if let Some(exif) = &self.exif {
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
        if let Some(exif) = &self.exif {
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
        if let Some(exif) = &self.exif {
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
