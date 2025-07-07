// Copyright (c) 2022-2025 Sophie Herold
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

//! Metadata handling

mod file;

use std::path::PathBuf;
use std::str::FromStr;

use chrono::prelude::*;
pub use file::FileInfo;
use glib::TimeZone;
use glycin::{Frame, FrameDetails, ImageDetails, MemoryFormat};
use gufo::common::datetime::DateTime;
use gufo::common::orientation::Orientation;

use crate::deps::*;
use crate::util;
use crate::util::gettext::*;

#[derive(Default, Debug)]
pub struct Metadata {
    mime_type: Option<String>,
    metadata: gufo::Metadata,
    file_info: Option<FileInfo>,
    image_info: Option<glycin::ImageDetails>,
    frame_info: Option<FrameDetails>,
    memory_format: Option<MemoryFormat>,
}

impl Metadata {
    fn set_exif_bytes(&mut self, bytes: Vec<u8>) {
        let result = self.metadata.add_raw_exif(bytes);

        if let Err(err) = &result {
            log::warn!("Failed to decode EXIF bytes: {err}");
        }
    }

    fn set_xmp_bytes(&mut self, bytes: Vec<u8>) {
        let result = self.metadata.add_raw_xmp(bytes);

        if let Err(err) = &result {
            log::warn!("Failed to decode XMP bytes: {err}");
        }
    }

    pub fn set_image_info(&mut self, image_info: ImageDetails) {
        if let Some(exif_raw) = image_info
            .metadata_exif()
            .as_ref()
            .and_then(|x| x.get_full().ok())
        {
            self.set_exif_bytes(exif_raw);
        }
        if let Some(exif_xmp) = image_info
            .metadata_xmp()
            .as_ref()
            .and_then(|x| x.get_full().ok())
        {
            self.set_xmp_bytes(exif_xmp);
        }
        self.image_info = Some(image_info);
    }

    pub fn set_frame_metadata(&mut self, frame: &Frame) {
        self.frame_info = Some(frame.details().clone());
        self.memory_format = Some(frame.memory_format());
    }

    pub fn set_file_info(&mut self, file_info: FileInfo) {
        self.file_info = Some(file_info);
    }

    pub fn set_mime_type(&mut self, mime_type: String) {
        self.mime_type = Some(mime_type);
    }

    pub fn merge(&mut self, other: Self) {
        if self.metadata.is_empty() {
            self.metadata = other.metadata;
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
        if self.memory_format.is_none() {
            self.memory_format = other.memory_format;
        }
    }

    pub fn mime_type(&self) -> Option<String> {
        self.mime_type.clone()
    }

    /// Unreliable mime type
    ///
    /// Before the image is successfully loaded, this mime type might be wrong
    /// since it is not yet based on the more complex mime type detection used
    /// by glyin.
    pub fn unreliable_mime_type(&self) -> Option<String> {
        self.mime_type().or_else(|| {
            self.file_info
                .as_ref()
                .and_then(|x| x.mime_type.as_ref())
                .map(|x| x.to_string())
        })
    }

    pub fn format_name(&self) -> Option<String> {
        self.image_info
            .as_ref()
            .and_then(|x| x.info_format_name().map(|x| x.to_string()))
            .or_else(|| self.unreliable_mime_type())
    }

    pub fn alpha_channel(&self) -> Option<bool> {
        self.frame_info
            .as_ref()
            .and_then(|x| x.info_alpha_channel())
    }

    pub fn grayscale(&self) -> Option<bool> {
        self.frame_info.as_ref().and_then(|x| x.info_grayscale())
    }

    pub fn bit_depth(&self) -> Option<u8> {
        self.frame_info.as_ref().and_then(|x| x.info_bit_depth())
    }

    pub fn is_svg(&self) -> bool {
        matches!(
            self.mime_type.as_deref(),
            Some("image/svg+xml") | Some("image/svg+xml-compressed")
        )
    }

    pub fn is_potentially_transparent(&self) -> bool {
        self.memory_format.is_some_and(|x| x.has_alpha())
    }

    pub fn transformations_applied(&self) -> bool {
        self.image_info
            .as_ref()
            .map(|x| x.transformation_ignore_exif())
            .unwrap_or(false)
    }

    pub fn orientation(&self) -> Orientation {
        if self.transformations_applied() {
            // HEIF library already does it's transformations on its own
            Orientation::Id
        } else {
            self.metadata.orientation().unwrap_or(Orientation::Id)
        }
    }

    pub fn dimensions_inch(&self) -> Option<(f64, f64)> {
        self.image_info.as_ref().and_then(|x| x.dimensions_inch())
    }

    pub fn dimensions_text(&self) -> Option<String> {
        self.image_info
            .as_ref()
            .and_then(|x| x.info_dimensions_text().map(|x| x.to_string()))
    }

    pub fn file_name(&self) -> Option<String> {
        self.file_info.as_ref().map(|x| x.display_name.to_string())
    }

    /// Original host path inside flatpaks obtained via xattr
    pub fn host_path(&self) -> Option<PathBuf> {
        self.file_info
            .as_ref()
            .and_then(|x| x.host_path.as_ref())
            .and_then(|x| PathBuf::from_str(x.as_str()).ok())
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
        if let Some(date_time) = &self.metadata.date_time_original() {
            let glib_date_time = match date_time {
                DateTime::Naive(naive) => {
                    glib::DateTime::from_unix_local(naive.and_utc().timestamp()).ok()
                }
                DateTime::FixedOffset(fixed) => {
                    let naive = fixed.naive_utc();
                    glib::DateTime::new(
                        &TimeZone::from_offset(fixed.timezone().local_minus_utc() * 60),
                        naive.year(),
                        naive.month() as i32,
                        naive.day() as i32,
                        naive.hour() as i32,
                        naive.minute() as i32,
                        naive.second() as f64,
                    )
                    .ok()
                }
            };

            if let Some(glib_date_time) = glib_date_time {
                util::datetime_fmt(&glib_date_time)
            } else {
                Some(date_time.to_string())
            }
        } else {
            None
        }
    }

    pub fn user_comment(&self) -> Option<String> {
        self.metadata.user_comment()
    }

    pub fn f_number(&self) -> Option<String> {
        if let Some(f_number) = self.metadata.f_number() {
            return Some(
                // Translators: Display of the f-number <https://en.wikipedia.org/wiki/F-number>. {} will be replaced with the number
                gettext_f(r"\u{192}\u{2215}{}", [f_number.to_string()]),
            );
        }

        None
    }

    pub fn exposure_time(&self) -> Option<String> {
        if let Some((num, denom)) = self.metadata.exposure_time() {
            let speed = num as f64 / denom as f64;
            if speed <= 0.5 {
                let exposure = format!("{:.0}", 1. / speed);
                // Translators: Fractional exposure time (photography) in seconds
                Some(gettext_f(r"1\u{2215}{}\u{202F}s", [exposure]))
            } else {
                let exposure = if speed < 5. {
                    format!("{:.1}", speed)
                } else {
                    format!("{:.0}", speed)
                };
                // Translators: Exposure time (photography) in seconds
                Some(gettext_f(r"{}\u{202F}s", [exposure]))
            }
        } else {
            None
        }
    }

    pub fn iso(&self) -> Option<String> {
        self.metadata.iso_speed_rating().map(|iso| iso.to_string())
    }

    pub fn focal_length(&self) -> Option<String> {
        self.metadata
            .focal_length()
            .map(|focal_length| gettext_f(r"{}\u{202F}mm", [focal_length.to_string()]))
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
        self.metadata.model()
    }

    pub fn maker(&self) -> Option<String> {
        self.metadata.make()
    }

    pub fn gps_location(&self) -> Option<gufo::common::geography::Location> {
        self.metadata.gps_location()
    }

    pub fn gps_location_display(&self) -> Option<String> {
        if let Some(location) = self.gps_location() {
            let (lat_ref, (lat_deg, lat_min, lat_sec)) = location.lat_ref_deg_min_sec();

            let lat_ref = match lat_ref {
                gufo::common::geography::LatRef::North => {
                    // Translators: short for "north" in GPS coordinate
                    gettext("N")
                }
                gufo::common::geography::LatRef::South => {
                    // Translators: short for "south" in GPS coordinate
                    gettext("S")
                }
            };

            let (lon_ref, (lon_deg, lon_min, lon_sec)) = location.lon_ref_deg_min_sec();

            let lon_ref = match lon_ref {
                gufo::common::geography::LonRef::East => {
                    // Translators: short for "east" in GPS coordinate
                    gettext("E")
                }
                gufo::common::geography::LonRef::West => {
                    // Translators: short for "west" in GPS coordinate
                    gettext("W")
                }
            };

            let lat = format!("{lat_deg}° {lat_min}′ {lat_sec:05.2}″ {lat_ref}");
            let lon = format!("{lon_deg}° {lon_min}′ {lon_sec:05.2}″ {lon_ref}");
            Some(format!("{lat}\n{lon}"))
        } else {
            None
        }
    }

    /// Outputs location as "City, County" if possible
    ///
    /// Falls back to coordinates
    pub fn gps_location_place(&self) -> Option<String> {
        if let Some(location) = self.gps_location() {
            if let Some(world) = libgweather::Location::world() {
                let lat = location.lat.0;
                let lon = location.lon.0;
                let nearest_city = world.find_nearest_city(lat, lon);
                let location_exact = libgweather::Location::new_detached("", None, lat, lon);

                // do not use city if more than 15 km away
                if nearest_city.distance(&location_exact) < 15. {
                    if let (Some(city), Some(country)) =
                        (nearest_city.city_name(), nearest_city.country_name())
                    {
                        return Some(format!("{city}, {country}"));
                    }
                }
            }

            self.gps_location_display()
        } else {
            None
        }
    }
}
