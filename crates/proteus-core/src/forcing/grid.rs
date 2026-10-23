// grid.rs maps geographic coordinates to tiles and grid cells

use super::{DataLoader, TileKey};
use crate::fetch::TileFetcher;

/// fractional coordinates reused by fields on the same grid
#[derive(Clone, Copy)]
pub(super) struct GridSample {
    pub(super) output_index: usize,
    pub(super) x: f32,
    pub(super) y: f32,
}

impl<F: TileFetcher> DataLoader<F> {
    /// Compute the geographic origin (min_lon, min_lat) of a tile.
    pub(super) fn tile_origin(&self, key: &TileKey) -> (f32, f32) {
        (
            self.min_lon + (key.lon_idx as f32) * self.tile_size,
            self.min_lat + (key.lat_idx as f32) * self.tile_size,
        )
    }

    pub fn get_tile_key(&self, lon: f32, lat: f32, day: usize) -> TileKey {
        let lon_idx = ((lon - self.min_lon) / self.tile_size).floor() as i32;
        let lat_idx = ((lat - self.min_lat) / self.tile_size).floor() as i32;

        TileKey {
            lon_idx: lon_idx.max(0).min(35) as usize,
            lat_idx: lat_idx.max(0).min(16) as usize,
            day,
        }
    }

    pub fn get_cell_index(
        &self,
        lon: f32,
        lat: f32,
        n_lon: usize,
        n_lat: usize,
        step: f32,
        tile_min_lon: f32,
        tile_min_lat: f32,
    ) -> (usize, usize) {
        let lon_cell = ((lon - tile_min_lon) / step).floor() as i32;
        let lat_cell = ((lat - tile_min_lat) / step).floor() as i32;

        let lon_cell = lon_cell.max(0).min(n_lon as i32 - 2) as usize;
        let lat_cell = lat_cell.max(0).min(n_lat as i32 - 2) as usize;

        (lon_cell, lat_cell)
    }
}
