// Copyright (c) 2023-2024 Sophie Herold
// Copyright (c) 2024 Maximiliano Sandoval
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

use super::*;

impl imp::LpImage {
    /// Returns the background color that should be used with this image
    ///
    /// Returns the default color if no one has been guessed yet
    pub fn background_color(&self) -> gdk::RGBA {
        if let Some(fixed) = *self.fixed_background_color.borrow() {
            return fixed;
        }

        (*self.background_color.borrow()).unwrap_or_else(Self::default_background_color)
    }

    pub(super) fn set_background_color(&self, color: Option<gdk::RGBA>) {
        self.background_color.replace(color);
    }

    pub fn default_background_color() -> gdk::RGBA {
        if adw::StyleManager::default().is_dark() {
            BACKGROUND_COLOR_DEFAULT
        } else {
            BACKGROUND_COLOR_DEFAULT_LIGHT_MODE
        }
    }

    pub fn alternate_background_color() -> gdk::RGBA {
        if adw::StyleManager::default().is_dark() {
            BACKGROUND_COLOR_ALTERNATE
        } else {
            BACKGROUND_COLOR_ALTERNATE_LIGHT_MODE
        }
    }

    /// Returns a background color that should give suitable contrast with
    /// transparent images
    ///
    /// For non-transparent images this always returns
    /// `BACKGROUND_COLOR_DEFAULT`
    pub async fn background_color_guess(&self) -> Option<gdk::RGBA> {
        let obj = self.obj();

        log::debug!("Determining background color");

        if self.fixed_background_color.borrow().is_some() {
            return None;
        }

        // Shortcut for formats that don't support transparency
        if !obj.metadata().is_potentially_transparent() {
            log::trace!("This format does not support transparency");
            return Some(Self::default_background_color());
        }

        let (width, height) = self.untransformed_dimensions();
        let max_size = i32::max(width, height);

        // Only use max 200px size scaled image for analysis
        let zoom = f64::min(1., 201. / max_size as f64);

        let snapshot = gtk::Snapshot::new();
        let render_options = tiling::RenderOptions {
            scaling_filter: gsk::ScalingFilter::Nearest,
            background_color: None,
            scaling: 1.,
        };
        self.frame_buffer
            .load()
            .add_to_snapshot(&snapshot, zoom, &render_options);

        let node = snapshot.to_node()?;

        let renderer = obj.root()?.renderer()?;

        // Render the small version of the image and download to RAM
        let texture = renderer.render_texture(node, None);
        let mut downloader: gdk::TextureDownloader = gdk::TextureDownloader::new(&texture);
        downloader.set_format(gdk::MemoryFormat::R8g8b8a8);
        let (bytes, stride) = downloader.download_bytes();

        // Get here because only available in main thread
        let alternate_color = Self::alternate_background_color();
        let default_color = Self::default_background_color();

        gio::spawn_blocking(move || {
            let mut has_transparency = false;
            let mut bytes_iter = bytes.iter();
            // Number of transparent pixels
            let mut completely_transparent = 0;
            // Number of non-transparent pixels with bad contrast
            let mut bad_contrast = 0;
            'img: loop {
                for _ in 0..texture.width() {
                    let Some(r) = bytes_iter.next() else {
                        break 'img;
                    };
                    let Some(g) = bytes_iter.next() else {
                        break 'img;
                    };
                    let Some(b) = bytes_iter.next() else {
                        break 'img;
                    };
                    let Some(a) = bytes_iter.next() else {
                        break 'img;
                    };

                    if *a < 255 {
                        has_transparency = true;
                    }

                    if *a == 0 {
                        completely_transparent += 1;
                    } else {
                        let fg = gdk::RGBA::new(
                            *r as f32 / 255.,
                            *g as f32 / 255.,
                            *b as f32 / 255.,
                            *a as f32 / 255.,
                        );
                        let contrast = crate::util::contrast_ratio(&default_color, &fg);

                        if contrast < BACKGROUND_GUESS_LOW_CONTRAST_RATIO {
                            bad_contrast += 1;
                        }
                    }
                }

                let advance_by = stride - 4 * texture.width() as usize;

                if advance_by > 0 {
                    bytes_iter.nth(advance_by - 1);
                }
            }

            if !has_transparency {
                log::trace!("This image does not have transparency");
                return Some(default_color);
            }

            let n_pixels = texture.width() * texture.height();

            let part_bad_contrast = if completely_transparent < n_pixels {
                bad_contrast as f64 / (n_pixels as f64 - completely_transparent as f64)
            } else {
                1.
            };

            log::trace!("Total: {n_pixels}, transparent: {completely_transparent}, bad contrast: {bad_contrast}");
            log::trace!("Amount bad contrast: {part_bad_contrast}");

            if part_bad_contrast > BACKGROUND_GUESS_LOW_CONTRAST_TRHESHOLD {
                Some(alternate_color)
            } else {
                Some(default_color)
            }
        })
        .await
        .ok()?
    }
}

impl LpImage {
    pub fn set_fixed_background_color(&self, color: Option<gdk::RGBA>) {
        self.imp().fixed_background_color.replace(color);
    }
}
