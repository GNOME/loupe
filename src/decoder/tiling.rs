// Copyright (c) 2023-2025 Sophie Herold
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

//! Tiled renderer
//!
//! It is not always feasible or desiareble to store the complete decoded
//! image in the VRAM. [`TiledImage`] allows to compose the parts of
//! the image currently viewed, out of smaller [`Tile`]s.
//!
//! This is especially important for SVGs where every the image has to
//! be re-genearted for each zoom level. It can also be used to allow
//! showing large JPEGs etc where the complete decoded image would not
//! even fit in the VRAM.
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use gtk::prelude::*;

use super::{DecoderUpdate, UpdateSender};
use crate::deps::*;

const ZOOM_SIGNIFICANT_DIGITS: i32 = 6;
pub const MIN_TILE_SIZE: u16 = 500;

pub type ZoomLevel = u32;
pub type Coordinate = u32;
pub type Coordinates = (u32, u32);

#[derive(Clone, Debug)]
/// Part of the image
pub struct Tile {
    pub position: Coordinates,
    pub zoom_level: ZoomLevel,
    /// Border that gets cut away to avoid scaling artifacts at the sides
    pub bleed: u8,
    pub texture: gdk::Texture,
}

/// Temporary function until HighDPI is sorted out
///
/// TODO: Does this has to follow scaling factor?
#[must_use]
pub fn round(r: &graphene::Rect) -> graphene::Rect {
    graphene::Rect::new(
        r.x().round(),
        r.y().round(),
        r.width().round(),
        r.height().round(),
    )
}

impl Tile {
    pub fn area(&self) -> graphene::Rect {
        let bleed = self.bleed as f32;
        // The texture is rendered slightly larger such that bleed can be cut away
        self.area_with_bleed().inset_r(bleed, bleed)
    }

    pub fn area_with_bleed(&self) -> graphene::Rect {
        let width = self.texture.width() as f32;
        let height = self.texture.height() as f32;
        let bleed = self.bleed as f32;
        graphene::Rect::new(
            self.position.0 as f32 - bleed,
            self.position.1 as f32 - bleed,
            width,
            height,
        )
    }

    pub fn zoom(&self) -> f32 {
        level_to_zoom(self.zoom_level) as f32
    }

    /// Add tile to snapshot
    ///
    /// This is where parts of images actually land on the screen
    pub fn add_to_snapshot(
        &self,
        snapshot: &gtk::Snapshot,
        output_zoom: f64,
        options: &RenderOptions,
    ) {
        // Multiply by scale factor to get the image scaled to `zoom` in physical pixels
        let zoom = output_zoom as f32 * options.scaling as f32 / self.zoom();
        let area = round(&self.area().scale(zoom, zoom));
        let area_with_bleed = round(&self.area_with_bleed().scale(zoom, zoom));

        if area_with_bleed.width() < 1. || area_with_bleed.height() < 1. {
            log::warn!("Trying to draw image with dimensions smaller than 1");
            return;
        }

        // TODO: do not clip outer boundaries of the image
        snapshot.push_clip(&area);
        // TODO: do not clip outer boundaries of the image
        if let Some(background_color) = &options.background_color {
            snapshot.append_color(background_color, &area_with_bleed);
        }
        snapshot.append_scaled_texture(&self.texture, options.scaling_filter, &area_with_bleed);
        snapshot.pop();

        if std::env::var_os("LOUPE_TILING_DEBUG").is_some_and(|x| x.len() > 0) {
            snapshot.append_inset_shadow(
                &gsk::RoundedRect::from_rect(area, 0.),
                &gdk::RGBA::new(1., 0., 0., 0.7),
                0.,
                0.,
                10.,
                0.,
            );
        }
    }

    fn coordinates(&self) -> Coordinates {
        self.position
    }
}

/// Buffers multiple images for animations
///
/// If the image is not animated, this only contains one image
#[derive(Clone, Debug, Default)]
pub struct FrameBuffer {
    /// The first entry is always the currently shown frame
    pub images: VecDeque<TiledImage>,
    pub update_sender: Option<UpdateSender>,
}

impl FrameBuffer {
    /// Returns mutable reference to the current image
    ///
    /// This is a convenience functions for the case of non-animated images.
    /// If no image exists yet, a new one is inserted an returned.
    pub fn current(&mut self) -> &mut TiledImage {
        if self.images.is_empty() {
            self.images.push_back(TiledImage::default())
        }
        self.images.front_mut().unwrap()
    }

    pub fn original_dimensions(&self) -> Option<Coordinates> {
        self.images.front()?.original_dimensions
    }

    pub fn set_update_sender(&mut self, sender: UpdateSender) {
        self.update_sender = Some(sender);
    }

    /// Render the current image into this snapshot
    pub fn add_to_snapshot(&self, snapshot: &gtk::Snapshot, zoom: f64, options: &RenderOptions) {
        if let Some(tiling) = self.images.front() {
            tiling.add_to_snapshot(snapshot, zoom, options);
        } else {
            log::error!("Trying to snapshot empty tiling queue");
        }
    }

    pub fn cleanup(&mut self, zoom: f64, viewport: graphene::Rect) {
        self.current().cleanup(zoom, viewport);
    }

    pub fn contains(&self, zoom: f64, coordinates: Coordinates) -> bool {
        self.images
            .front()
            .is_some_and(|x| x.contains(zoom, coordinates))
    }

    /// Returns true if there are no textures
    pub fn is_empty(&self) -> bool {
        self.images
            .front()
            .is_some_and(|x| x.tile_layers.is_empty())
    }
}

#[derive(Clone, Debug, Default)]
/// Store tiles for image
///
/// This is the common representation for image textures.
/// I many cases there might only be one tile.
pub struct TiledImage {
    pub tile_layers: BTreeMap<ZoomLevel, TileLayer>,
    /// Complete image size
    pub original_dimensions: Option<Coordinates>,
    /// Delay until to show this frame if animated
    pub delay: std::time::Duration,
}

#[derive(Clone, Debug)]
pub struct TileLayer {
    tiles: BTreeMap<Coordinates, Tile>,
    covering: Covering,
}

#[derive(Debug, Clone)]
pub struct RenderOptions {
    pub scaling_filter: gsk::ScalingFilter,
    pub scaling: f64,
    pub background_color: Option<gdk::RGBA>,
}

impl TiledImage {
    /// Render tiles as image
    ///
    /// This is what actually draws the image
    pub fn add_to_snapshot(&self, snapshot: &gtk::Snapshot, zoom: f64, options: &RenderOptions) {
        let tiles: Vec<_> = self
            .iter_tiles_rendering_priority(zoom)
            .flat_map(|(_, tiles)| tiles)
            .collect();

        log::trace!("Rendering {} tiles", tiles.len());

        // Scale from application pixels back to physical pixels
        snapshot.scale(1. / options.scaling as f32, 1. / options.scaling as f32);
        // reverse to put least fitting tiles in the background
        for (_, tile) in tiles.iter().rev() {
            tile.add_to_snapshot(snapshot, zoom, options);
        }
        // reset scale
        snapshot.scale(options.scaling as f32, options.scaling as f32);
    }

    pub fn push(&mut self, tile: Tile) {
        let layer = self
            .tile_layers
            .entry(tile.zoom_level)
            .or_insert_with(|| TileLayer {
                tiles: Default::default(),
                covering: Covering::Simple,
            });
        layer.tiles.insert(tile.coordinates(), tile);
    }

    pub fn push_tile(&mut self, tiling: Tiling, position: Coordinates, texture: gdk::Texture) {
        let tile = Tile {
            position,
            texture,
            zoom_level: zoom_to_level(tiling.zoom),
            bleed: tiling.bleed,
        };

        let layer = self
            .tile_layers
            .entry(tile.zoom_level)
            .or_insert_with(|| TileLayer {
                tiles: Default::default(),
                covering: Covering::Tiling(tiling),
            });

        layer.tiles.insert(tile.coordinates(), tile);
    }

    pub fn get_layer_tiling_or_default(&mut self, zoom: f64, viewport: graphene::Rect) -> Tiling {
        let zoom_level = zoom_to_level(zoom);
        if let Some(TileLayer {
            covering: Covering::Tiling(tiling),
            ..
        }) = self.tile_layers.get(&zoom_level)
        {
            return *tiling;
        };

        let tiling = Tiling {
            origin: (viewport.x() as u32, viewport.y() as u32),
            tile_width: u16::max(MIN_TILE_SIZE, viewport.width() as u16),
            tile_height: u16::max(MIN_TILE_SIZE, viewport.height() as u16),
            zoom,
            bleed: 0,
            original_dimensions: self.original_dimensions.unwrap_or_default(),
        };

        self.tile_layers.insert(
            zoom_level,
            TileLayer {
                tiles: Default::default(),
                covering: Covering::Tiling(tiling),
            },
        );

        tiling
    }

    pub fn contains(&self, zoom: f64, coordinates: Coordinates) -> bool {
        let Some(tile_layer) = self.tile_layers.get(&zoom_to_level(zoom)) else {
            return false;
        };

        tile_layer.tiles.contains_key(&coordinates)
    }

    /// Tiles ordered by how good they are suited for the zoom level
    pub fn iter_tiles_rendering_priority(
        &self,
        zoom: f64,
    ) -> impl Iterator<Item = (&ZoomLevel, &BTreeMap<Coordinates, Tile>)> {
        self.iter_rendering_priority(zoom)
            .map(|(level, layer)| (level, &layer.tiles))
    }

    pub fn iter_rendering_priority(
        &self,
        zoom: f64,
    ) -> impl Iterator<Item = (&ZoomLevel, &TileLayer)> {
        let zoom_level = zoom_to_level(zoom);
        self.tile_layers
            // Tiles at zoom level and with better resolution
            .range(zoom_level..)
            // Afterwards try tiles with worse resolution
            .chain(self.tile_layers.range(..zoom_level).rev())
    }

    /// Viewport within zoom coordinates
    ///
    /// preserve_area: Slightly larger than the viewport, area for which we want
    /// tiles TODO: This is the most buggy function ever
    pub fn cleanup(&mut self, zoom: f64, preserve_area: graphene::Rect) {
        let zoom = zoom_normalize(zoom);
        let mut kept_tiles = Self::default();

        // Always work with 100% zoom level for tiles
        let preserve_area_100 = preserve_area.scale(1. / zoom as f32, 1. / zoom as f32);
        let mut missing_tiles = vec![preserve_area_100];

        // Keep tiles in total-zoom-out layer
        // TODO: Change the top layer if window size changes
        if let Some((_, top_layer)) = self.tile_layers.iter().next() {
            for tile in top_layer.tiles.values() {
                if let Covering::Tiling(tiling) = top_layer.covering {
                    kept_tiles.push_tile(tiling, tile.position, tile.texture.clone());
                } else {
                    return;
                }
            }
        }

        // Put all tiles in correct processing order
        let stored_tiles = self.iter_rendering_priority(zoom);

        for (zoom_level, tile_layer) in stored_tiles {
            // This should always be tiled since "Simple" would be in total-zoom-out layer
            if let Covering::Tiling(tiling) = tile_layer.covering {
                let tiles = &tile_layer.tiles;
                let tile_zoom = level_to_zoom(*zoom_level);

                let mut next_missing_tiles = vec![];

                // Tiling for this zoom level and viewport

                let preserve_area_tiling =
                    preserve_area_100.scale(tiling.zoom as f32, tiling.zoom as f32);
                for mut tile_instruction in tiling.relevant_tiles(&preserve_area_tiling) {
                    // scale to 100% reference
                    tile_instruction.area = tile_instruction
                        .area
                        .scale(1. / tile_zoom as f32, 1. / tile_zoom as f32);

                    // Determine if tile of this zoom level has overlap with a missing tile
                    let mut intersections = missing_tiles
                        .iter()
                        .filter(|missing| tile_instruction.area.is_intersected(missing))
                        .peekable();

                    if intersections.peek().is_some() {
                        if let Some(tile) = tiles.get(&tile_instruction.coordinates) {
                            // we have this tile and can provide it
                            kept_tiles.push_tile(tiling, tile.position, tile.texture.clone());
                        } else {
                            for missing in missing_tiles.iter() {
                                if let Some(overlap) = missing.intersection(&tile_instruction.area)
                                {
                                    next_missing_tiles.push(overlap);
                                }
                            }
                        }
                    }
                }

                missing_tiles = next_missing_tiles;
            } else {
                log::error!("Images should never have multiple 'Simple' layers: {tile_layer:?}");
            }
        }

        log::trace!(
            "Cleanup kept {} tiles",
            kept_tiles
                .tile_layers
                .values()
                .flat_map(|x| x.tiles.iter())
                .count()
        );

        self.tile_layers = kept_tiles.tile_layers;
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Covering {
    Simple,
    Tiling(Tiling),
}

#[derive(Debug, Clone, Copy)]
/// Abstract definition of a tiling
pub struct Tiling {
    pub origin: Coordinates,
    pub tile_width: u16,
    pub tile_height: u16,
    pub original_dimensions: Coordinates,
    pub zoom: f64,
    pub bleed: u8,
}

impl Tiling {
    pub fn scaled_dimensions(&self) -> Coordinates {
        let (w, h) = self.original_dimensions;
        (
            (w as f64 * self.zoom) as Coordinate,
            (h as f64 * self.zoom) as Coordinate,
        )
    }

    pub fn origin_shift_x(&self) -> f32 {
        let width = self.origin.0 as f32;
        let rem = width.rem_euclid(self.tile_width as f32);
        if rem > 0. {
            rem - self.tile_width as f32
        } else {
            0.
        }
    }

    pub fn origin_shift_y(&self) -> f32 {
        let height = self.origin.1 as f32;
        let rem = height.rem_euclid(self.tile_height as f32);
        if rem > 0. {
            rem - self.tile_height as f32
        } else {
            0.
        }
    }

    /// Returns relevant tiles
    ///
    /// This gives what tiles are needed for covering the `preload_area`.
    /// Decoders use the [`TileInstructions`] to know what they have to render.
    pub fn relevant_tiles(&self, preload_area: &graphene::Rect) -> Vec<TileInstructions> {
        let original_width = self.scaled_dimensions().0 as f32;
        let original_height = self.scaled_dimensions().1 as f32;

        let tile_width = self.tile_width as f32;
        let tile_height = self.tile_height as f32;

        let x0 = (preload_area.x() / tile_width).floor() * tile_width + self.origin_shift_x();
        let x1 = ((preload_area.x() + preload_area.width()) / tile_width).ceil() * tile_width
            + self.origin_shift_x();
        let y0 = (preload_area.y() / tile_height).floor() * tile_height + self.origin_shift_y();
        let y1 = ((preload_area.y() + preload_area.height()) / tile_height).ceil() * tile_height
            + self.origin_shift_y();

        let mut tiles = Vec::new();
        for x in (x0 as i32..=x1 as i32).step_by(self.tile_width.into()) {
            for y in (y0 as i32..=y1 as i32).step_by(self.tile_height.into()) {
                let area = graphene::Rect::new(x as f32, y as f32, tile_width, tile_height);
                let Some(restricted_area) = area.restrict(original_width, original_height) else {
                    continue;
                };

                let tile = TileInstructions {
                    tiling: *self,
                    area: restricted_area,
                    coordinates: (restricted_area.x() as u32, restricted_area.y() as u32),
                };
                tiles.push(tile);
            }
        }

        tiles.sort_by_key(|tile| tile.area.center().distance(&preload_area.center()).0 as u32);

        tiles
    }
}

#[derive(Debug, Copy, Clone)]
/// Instruction for decoder to generate this tile
pub struct TileInstructions {
    pub tiling: Tiling,

    pub area: graphene::Rect,
    pub coordinates: Coordinates,
}

impl TileInstructions {
    pub fn area_with_bleed(&self) -> graphene::Rect {
        let bleed = self.tiling.bleed as f32;
        self.area.inset_r(-bleed, -bleed)
    }
}

/// Integer zoom levels are used because they are hashable
/// and ignore tiny float calculation errors.
pub fn zoom_to_level(zoom: f64) -> u32 {
    (zoom * 10_f64.powi(ZOOM_SIGNIFICANT_DIGITS)) as u32
}

pub fn level_to_zoom(zoom_level: u32) -> f64 {
    zoom_level as f64 * 10_f64.powi(-ZOOM_SIGNIFICANT_DIGITS)
}

pub fn zoom_normalize(zoom: f64) -> f64 {
    level_to_zoom(zoom_to_level(zoom))
}

trait RectExt {
    fn is_intersected(&self, b: &graphene::Rect) -> bool;
    fn restrict(&self, max_width: f32, max_height: f32) -> Option<graphene::Rect>;
}

impl RectExt for graphene::Rect {
    fn is_intersected(&self, b: &graphene::Rect) -> bool {
        self.intersection(b).is_some()
    }

    fn restrict(&self, max_width: f32, max_height: f32) -> Option<graphene::Rect> {
        let mut x = self.x();
        let mut y = self.y();
        let mut width = self.width();
        let mut height = self.height();

        if x < 0. {
            width += x;
            x = 0.;
        }
        if y < 0. {
            height += y;
            y = 0.;
        }

        if x + width > max_width {
            width = max_width - x;
        }

        if y + height > max_height {
            height = max_height - y;
        }

        if width <= 0. || height <= 0. {
            return None;
        }

        Some(Self::new(x, y, width, height))
    }
}

#[derive(Debug, Default)]
pub struct SharedFrameBuffer {
    buffer: ArcSwap<FrameBuffer>,
}

impl std::ops::Deref for SharedFrameBuffer {
    type Target = ArcSwap<FrameBuffer>;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl SharedFrameBuffer {
    pub fn push(&self, tile: Tile) {
        self.buffer.rcu(|tiling_store| {
            let mut new_store = (**tiling_store).clone();
            new_store.current().push(tile.clone());
            Arc::new(new_store)
        });
        if let Some(updater) = &self.buffer.load().update_sender {
            updater.send(DecoderUpdate::Redraw);
        }
    }

    pub fn push_tile(&self, tiling: Tiling, position: Coordinates, texture: gdk::Texture) {
        self.buffer.rcu(|tiling_store| {
            let mut new_store = (**tiling_store).clone();
            new_store
                .current()
                .push_tile(tiling, position, texture.clone());
            Arc::new(new_store)
        });
        if let Some(updater) = &self.buffer.load().update_sender {
            updater.send(DecoderUpdate::Redraw);
        }
    }

    pub fn push_frame(&self, tile: Tile, dimensions: Coordinates, delay: Duration) {
        let mut store = TiledImage::default();
        store.push(tile);
        store.original_dimensions = Some(dimensions);
        store.delay = delay;

        self.buffer.rcu(|tiling_store| {
            let mut new_store = (**tiling_store).clone();
            new_store.images.push_back(store.clone());
            Arc::new(new_store)
        });
    }

    /// Reset to default and return old store
    ///
    /// Used when reloading images
    pub fn reset(&self) -> Self {
        let old_buffer = self.buffer.swap(Default::default());

        Self {
            buffer: ArcSwap::new(old_buffer),
        }
    }

    pub fn get_layer_tiling_or_default(&self, zoom: f64, viewport: graphene::Rect) -> Tiling {
        let mut tiling = None;
        self.buffer.rcu(|tiling_store| {
            let mut new_store = (**tiling_store).clone();
            tiling = Some(
                new_store
                    .current()
                    .get_layer_tiling_or_default(zoom, viewport),
            );
            Arc::new(new_store)
        });
        tiling.unwrap()
    }

    /// Return true if the next frame should be shown and removes the outdated
    /// frame
    pub fn frame_timeout(&self, elapsed: Duration) -> bool {
        let images = &self.buffer.load().images;

        // Only move to next image if there is one buffered
        if images.len() > 1
            && images
                .front()
                .is_some_and(|next_frame| elapsed >= next_frame.delay)
        {
            self.next_frame();
            return true;
        }

        false
    }

    pub fn next_frame(&self) {
        self.buffer.rcu(|tiling_store| {
            let mut new_store = (**tiling_store).clone();
            new_store.images.pop_front();
            Arc::new(new_store)
        });
    }

    /// Returns the number of currently buffered frames
    pub fn n_frames(&self) -> usize {
        self.buffer.load().images.len()
    }

    pub fn set_original_dimensions(&self, size: Coordinates) {
        self.buffer.rcu(|tiling_store| {
            let mut new_store = (**tiling_store).clone();
            new_store.current().original_dimensions = Some(size);
            new_store.original_dimensions();
            Arc::new(new_store)
        });

        if let Some(updater) = &self.buffer.load().update_sender {
            updater.send(DecoderUpdate::Dimensions);
        }
    }

    pub fn set_update_sender(&self, sender: UpdateSender) {
        self.buffer.rcu(|tiling_store| {
            let mut new_store = (**tiling_store).clone();
            new_store.set_update_sender(sender.clone());
            Arc::new(new_store)
        });
    }
}
