// Copyright (c) 2022-2023 Sophie Herold
// Copyright (c) 2023 Lubosz Sarnecki
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

use crate::util::gettext::*;

#[derive(Debug, Clone, Copy)]
pub struct GPSLocation {
    pub latitude: GPSCoord,
    pub longitude: GPSCoord,
}

#[derive(Debug, Clone, Copy)]
pub struct GPSCoord {
    sing: bool,
    deg: f64,
    min: Option<f64>,
    sec: Option<f64>,
}

impl GPSCoord {
    fn to_f64(self) -> f64 {
        let sign = if self.sing { 1. } else { -1. };

        let min = self.min.unwrap_or_default();
        let sec = self.sec.unwrap_or_default();

        sign * (self.deg + min / 60. + sec / 60. / 60.)
    }

    fn display(&self, reference: &str) -> String {
        let deg = self.deg;

        if let (Some(min), Some(sec)) = (self.min, self.sec) {
            format!("{deg}° {min}′ {sec}″ {reference}")
        } else if let Some(min) = self.min {
            format!("{deg}° {min}′ {reference}")
        } else {
            format!("{deg}° {reference}")
        }
    }

    fn latitude_sign(reference: &[Vec<u8>]) -> Option<bool> {
        let reference = reference.first().and_then(|x| x.first())?;
        match reference.to_ascii_uppercase() {
            b'N' => Some(true),
            b'S' => Some(false),
            _ => None,
        }
    }

    fn longitude_sign(reference: &[Vec<u8>]) -> Option<bool> {
        let reference = reference.first().and_then(|x| x.first())?;
        match reference.to_ascii_uppercase() {
            b'E' => Some(true),
            b'W' => Some(false),
            _ => None,
        }
    }

    fn position_exif(position: &[exif::Rational]) -> Option<(f64, Option<f64>, Option<f64>)> {
        let (deg, mut min, mut sec) = (position.get(0)?, position.get(1), position.get(2));

        if let (Some(min_), Some(sec_)) = (min, sec) {
            if min_.denom > 1 && sec_.num == 0 {
                sec = None;
            }
        }

        if let Some(min_) = min {
            if deg.denom > 1 && min_.num == 0 {
                min = None;
            }
        }

        Some((
            deg.to_f64(),
            min.map(exif::Rational::to_f64),
            sec.map(exif::Rational::to_f64),
        ))
    }
}

impl GPSLocation {
    pub fn for_exif(
        latitude: &[exif::Rational],
        latitude_ref: &[Vec<u8>],
        longitude: &[exif::Rational],
        longitude_ref: &[Vec<u8>],
    ) -> Option<Self> {
        let (lat_deg, lat_min, lat_sec) = GPSCoord::position_exif(latitude)?;
        let lat_sign = GPSCoord::latitude_sign(latitude_ref)?;

        let (lon_deg, lon_min, lon_sec) = GPSCoord::position_exif(longitude)?;
        let lon_sign = GPSCoord::longitude_sign(longitude_ref)?;

        Some(Self {
            latitude: GPSCoord {
                sing: lat_sign,
                deg: lat_deg,
                min: lat_min,
                sec: lat_sec,
            },
            longitude: GPSCoord {
                sing: lon_sign,
                deg: lon_deg,
                min: lon_min,
                sec: lon_sec,
            },
        })
    }

    pub fn display(&self) -> String {
        if let Some(world) = libgweather::Location::world() {
            let location = world.find_nearest_city(self.latitude.to_f64(), self.longitude.to_f64());
            let location_exact = libgweather::Location::new_detached(
                "",
                None,
                self.latitude.to_f64(),
                self.longitude.to_f64(),
            );

            // do not use city if more than 100 km away
            if location.distance(&location_exact) < 100. {
                if let (Some(city), Some(country)) = (location.city_name(), location.country_name())
                {
                    return format!("{city}, {country}");
                }
            }
        }

        // fallback
        self.latitude_display() + "\n" + &self.longitude_display()
    }

    pub fn latitude_display(&self) -> String {
        let coord = self.latitude;
        let reference = if coord.sing {
            // Translators: short for "north" in GPS coordinate
            gettext("N")
        } else {
            // Translators: short for "south" in GPS coordinate
            gettext("S")
        };

        coord.display(&reference)
    }

    pub fn longitude_display(&self) -> String {
        let coord = self.longitude;
        let reference = if coord.sing {
            // Translators: short for "east" in GPS coordinate
            gettext("E")
        } else {
            // Translators: short for "west" in GPS coordinate
            gettext("W")
        };

        coord.display(&reference)
    }

    pub fn geo_uri(&self) -> String {
        let latitude = self.latitude.to_f64();
        let longitude = self.longitude.to_f64();
        // six decimal places gives as more than a meter accuracy
        format!("geo:{latitude:.6},{longitude:.6}")
    }
}
