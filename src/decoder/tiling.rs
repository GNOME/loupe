///! Tiled renderer
///!
///! It is not always feasible or desiareble to store the complete decoded
///! image in the VRAM. [`TilingStore`] allows to compose the parts of
///! the image currently viewed, out of smaller [`Tiles`].
///!
///! This is especially important for SVGs where every the image has to
///! be re-genearted for each zoom level. It can also be used to allow
///! showing large JPEGs etc where the complete decoded image would not
///! even fit in the VRAM.
use super::{DecoderUpdate, ImageDimensionDetails, UpdateSender};
use crate::deps::*;

use arc_swap::ArcSwap;
use gtk::prelude::*;

use std::collections::BTreeMap;
use std::sync::Arc;

const ZOOM_SIGNIFICANT_DIGETS: i32 = 6;
pub const TILE_SIZE: u16 = 4000;

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
    pub fn add_to_snapshot(&self, snapshot: &gtk::Snapshot, _options: &RenderOptions) {
        let zoom = self.zoom();
        let area = self.area().scale(1. / zoom, 1. / zoom);
        let area_with_bleed = self.area_with_bleed().scale(1. / zoom, 1. / zoom);

        // TODO: do not clip outer bounderies of the image
        snapshot.push_clip(&area);
        snapshot.append_color(&gdk::RGBA::new(0.118, 0.118, 0.118, 1.), &area);
        snapshot.append_texture(&self.texture, &area_with_bleed);
        snapshot.pop();
    }

    fn coordinates(&self) -> Coordinates {
        self.position
    }
}

#[derive(Clone, Debug)]
/// Store tiles for image
pub struct TilingStore {
    pub tiles: BTreeMap<ZoomLevel, BTreeMap<Coordinates, Tile>>,
    /// Tile size without bleed
    pub tile_size: u16,
    /// Complete image size
    pub original_dimensions: Option<Coordinates>,
    pub update_sender: Option<UpdateSender>,
}

impl Default for TilingStore {
    fn default() -> Self {
        Self::new(TILE_SIZE)
    }
}

#[derive(Debug, Clone)]
pub struct RenderOptions {
    pub scaling_filter: gsk::ScalingFilter,
}

impl TilingStore {
    pub fn new(tile_size: u16) -> Self {
        TilingStore {
            tiles: Default::default(),
            tile_size,
            original_dimensions: None,
            update_sender: None,
        }
    }

    /// Render tiles as image
    ///
    /// This is what actually draws the image
    pub fn add_to_snapshot(&self, snapshot: &gtk::Snapshot, zoom: f64, options: &RenderOptions) {
        let tiles: Vec<_> = self
            .iter_rendering_priority(zoom)
            .flat_map(|(_, tiles)| tiles)
            .collect();

        log::trace!("Rendering {} tiles", tiles.len());
        // reverse to put least fitting tiles in the background
        for (_, tile) in tiles.iter().rev() {
            tile.add_to_snapshot(snapshot, options);
        }
    }

    pub fn push(&mut self, tile: Tile) {
        let map = self.tiles.entry(tile.zoom_level).or_default();
        map.insert(tile.coordinates(), tile);
        if let Some(updater) = &self.update_sender {
            updater.send(DecoderUpdate::Redraw);
        }
    }

    pub fn set_original_dimensions(&mut self, dimensions: Coordinates) {
        self.set_original_dimensions_full(dimensions, Default::default());
    }

    pub fn set_original_dimensions_full(
        &mut self,
        dimensions: Coordinates,
        dimension_details: ImageDimensionDetails,
    ) {
        self.original_dimensions = Some(dimensions);
        if let Some(updater) = &self.update_sender {
            updater.send(DecoderUpdate::Dimensions(dimension_details));
        }
    }

    pub fn set_update_sender(&mut self, sender: UpdateSender) {
        self.update_sender = Some(sender);
    }

    pub fn contains(&self, zoom: f64, coordinates: Coordinates) -> bool {
        let Some(tile_plane) = self.tiles.get(&zoom_to_level(zoom)) else { return false };

        tile_plane.contains_key(&coordinates)
    }

    /// Tiles ordered by how good they are suited for the zoom level
    pub fn iter_rendering_priority(
        &self,
        zoom: f64,
    ) -> impl Iterator<Item = (&ZoomLevel, &BTreeMap<Coordinates, Tile>)> {
        let zoom_level = zoom_to_level(zoom);
        self.tiles
            // Tiles at zoom level and with better resolution
            .range(zoom_level..)
            // Afterwards try tiles with worse resolution
            .chain(self.tiles.range(..zoom_level).rev())
    }

    /// Viewport within zoom coordinates
    ///
    /// TODO: This is the most buggy function ever
    pub fn cleanup(&mut self, zoom: f64, viewport: graphene::Rect) {
        let Some(original_dimensions) = self.original_dimensions else { log::error!("Too-early cleanup: Original dimension not known"); return; };

        let mut kept_tiles = Self::new(self.tile_size);

        // Always work with 100% zoom level for tiles
        let mut missing_tiles = vec![viewport.scale(1. / zoom as f32, 1. / zoom as f32)];

        // Keep tiles in top layer
        // TODO: Change the top layer if window size changes
        if let Some(top_layer) = self.tiles.iter().next() {
            for tile in top_layer.1.values() {
                kept_tiles.push(tile.clone());
            }
        }

        // Put all tiles in correct processing order
        let stored_tiles = self.iter_rendering_priority(zoom);

        for (zoom_level, tiles) in stored_tiles {
            let tile_zoom = level_to_zoom(*zoom_level);

            let mut next_missing_tiles = vec![];

            // Tiling for this zoom level and viewport
            let tiling = Tiling {
                tile_size: self.tile_size,
                original_dimensions,
                zoom: tile_zoom,
                bleed: 2,
            };

            for mut tile_instruction in tiling.relevant_tiles(&viewport) {
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
                        kept_tiles.push(tile.clone());
                    } else {
                        for missing in missing_tiles.iter() {
                            if let Some(overlap) = missing.intersection(&tile_instruction.area) {
                                next_missing_tiles.push(overlap);
                            }
                        }
                    }
                }
            }

            missing_tiles = next_missing_tiles;
        }

        kept_tiles.tiles.values().flat_map(|x| x.iter()).count();

        self.tiles = kept_tiles.tiles;
    }
}

#[derive(Debug, Clone, Copy)]
/// Abstract definition of a tiling
pub struct Tiling {
    pub tile_size: u16,
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

    /// Returns relevant tiles
    ///
    /// This gives what tiles are needed for convering the `preload_area`.
    /// Decoders use the [`TileInstructions`] to know what they have to render.
    pub fn relevant_tiles(&self, preload_area: &graphene::Rect) -> Vec<TileInstructions> {
        let tile_size = self.tile_size as f32;

        let x0 = (preload_area.x() / tile_size).floor() * tile_size;
        let x1 = ((preload_area.x() + preload_area.width()) / tile_size).ceil() * tile_size;
        let y0 = (preload_area.y() / tile_size).floor() * tile_size;
        let y1 = ((preload_area.y() + preload_area.height()) / tile_size).ceil() * tile_size;

        let mut tiles = Vec::new();
        for x in (x0 as u32..x1 as u32).step_by(self.tile_size.into()) {
            for y in (y0 as u32..y1 as u32).step_by(self.tile_size.into()) {
                let width = tile_size
                    + f32::min(
                        0.,
                        self.scaled_dimensions().0 as f32 - (x as f32 + tile_size),
                    );
                let height = tile_size
                    + f32::min(
                        0.,
                        self.scaled_dimensions().1 as f32 - (y as f32 + tile_size),
                    );

                let area = graphene::Rect::new(x as f32, y as f32, width, height);
                let tile = TileInstructions {
                    tiling: *self,
                    area,
                    coordinates: (x, y),
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
    (zoom * 10_f64.powi(ZOOM_SIGNIFICANT_DIGETS)) as u32
}

pub fn level_to_zoom(zoom_level: u32) -> f64 {
    zoom_level as f64 * 10_f64.powi(-ZOOM_SIGNIFICANT_DIGETS)
}

pub fn zoom_normalize(zoom: f64) -> f64 {
    level_to_zoom(zoom_to_level(zoom))
}

trait RectExt {
    fn is_intersected(&self, b: &graphene::Rect) -> bool;
}

impl RectExt for graphene::Rect {
    fn is_intersected(&self, b: &graphene::Rect) -> bool {
        self.intersection(b).is_some()
    }
}

pub trait TilingStoreExt {
    fn push(&self, tile: Tile);
    fn set_original_dimensions(&self, size: Coordinates);
    fn set_original_dimensions_full(
        &self,
        size: Coordinates,
        dimension_details: ImageDimensionDetails,
    );
    fn set_update_sender(&self, sender: UpdateSender);
}

impl TilingStoreExt for ArcSwap<TilingStore> {
    fn push(&self, tile: Tile) {
        self.rcu(|tiling_store| {
            let mut new_store = (**tiling_store).clone();
            new_store.push(tile.clone());
            Arc::new(new_store)
        });
    }

    fn set_original_dimensions(&self, size: Coordinates) {
        self.rcu(|tiling_store| {
            let mut new_store = (**tiling_store).clone();
            new_store.set_original_dimensions(size);
            Arc::new(new_store)
        });
    }

    fn set_original_dimensions_full(
        &self,
        size: Coordinates,
        dimension_details: ImageDimensionDetails,
    ) {
        self.rcu(|tiling_store| {
            let mut new_store = (**tiling_store).clone();
            new_store.set_original_dimensions_full(size, dimension_details.clone());
            Arc::new(new_store)
        });
    }

    fn set_update_sender(&self, sender: UpdateSender) {
        self.rcu(|tiling_store| {
            let mut new_store = (**tiling_store).clone();
            new_store.set_update_sender(sender.clone());
            Arc::new(new_store)
        });
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_tiling_cleanup() {
        let mut store = TilingStore::new(2);

        let toplevel = Tile {
            position: (0, 0),
            zoom_level: zoom_to_level(1.),
            bleed: 0,
            texture: gdk::MemoryTexture::new(
                4,
                4,
                gdk::MemoryFormat::R8g8b8,
                &glib::Bytes::from_static(&[0; 4 * 4 * 3]),
                4 * 3,
            )
            .upcast(),
        };
        store.push(toplevel);

        let right_level = Tile {
            position: (0, 0),
            zoom_level: zoom_to_level(1.5),
            bleed: 0,
            texture: gdk::MemoryTexture::new(
                2,
                2,
                gdk::MemoryFormat::R8g8b8,
                &glib::Bytes::from_static(&[0; 2 * 2 * 3]),
                2 * 3,
            )
            .upcast(),
        };
        store.push(right_level);

        let low_level = Tile {
            position: (2, 0),
            zoom_level: zoom_to_level(1.6),
            bleed: 0,
            texture: gdk::MemoryTexture::new(
                2,
                2,
                gdk::MemoryFormat::R8g8b8,
                &glib::Bytes::from_static(&[0; 2 * 2 * 3]),
                2 * 3,
            )
            .upcast(),
        };
        store.push(low_level);

        let overlapped = Tile {
            position: (0, 0),
            zoom_level: zoom_to_level(1.7),
            bleed: 0,
            texture: gdk::MemoryTexture::new(
                2,
                2,
                gdk::MemoryFormat::R8g8b8,
                &glib::Bytes::from_static(&[0; 2 * 2 * 3]),
                2 * 3,
            )
            .upcast(),
        };
        store.push(overlapped);

        store.cleanup(1.5, graphene::Rect::new(0., 0., 4. * 1.5, 4. * 1.5));

        assert!(store.tiles.get(&1000000).unwrap().get(&(0, 0)).is_some());
        assert!(store.tiles.get(&1500000).unwrap().get(&(0, 0)).is_some());
        assert!(store.tiles.get(&1600000).unwrap().get(&(2, 0)).is_some());
        // TODO: This should pass, but it does not
        //assert!(store.tiles.get(&1700000).is_none());
    }

    #[test]
    fn test_tiling_cleanup_simple() {
        let mut store = TilingStore::new(2);

        let toplevel = Tile {
            position: (0, 0),
            zoom_level: zoom_to_level(1.),
            bleed: 0,
            texture: gdk::MemoryTexture::new(
                4,
                4,
                gdk::MemoryFormat::R8g8b8,
                &glib::Bytes::from_static(&[0; 4 * 4 * 3]),
                4 * 3,
            )
            .upcast(),
        };
        store.push(toplevel);

        let detailled = Tile {
            position: (2, 2),
            zoom_level: zoom_to_level(2.),
            bleed: 0,
            texture: gdk::MemoryTexture::new(
                4,
                4,
                gdk::MemoryFormat::R8g8b8,
                &glib::Bytes::from_static(&[0; 4 * 4 * 3]),
                4 * 3,
            )
            .upcast(),
        };
        store.push(detailled);

        store.cleanup(1.5, graphene::Rect::new(0., 0., 20., 20.));

        assert!(store.tiles.get(&1000000).unwrap().get(&(0, 0)).is_some());
        assert!(store.tiles.get(&2000000).unwrap().get(&(2, 2)).is_some());
    }
}
