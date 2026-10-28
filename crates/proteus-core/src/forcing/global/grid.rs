// grid.rs maps geographic coordinates to tiles and grid cells

use super::{GlobalForcing, TileKey};
use crate::fetch::TileFetcher;

impl<F: TileFetcher> GlobalForcing<F> {
    /// Compute the geographic origin (min_lon, min_lat) of a tile.
    pub(super) fn tile_origin(&self, key: &TileKey) -> (f32, f32) {
        (
            self.min_lon + (key.lon_idx as f32) * self.tile_size,
            self.min_lat + (key.lat_idx as f32) * self.tile_size,
        )
    }

    pub(super) fn get_tile_key(&self, lon: f32, lat: f32, day: usize) -> TileKey {
        let lon_idx = ((lon - self.min_lon) / self.tile_size).floor() as i32;
        let lat_idx = ((lat - self.min_lat) / self.tile_size).floor() as i32;

        TileKey {
            lon_idx: lon_idx.max(0).min(35) as usize,
            lat_idx: lat_idx.max(0).min(16) as usize,
            day,
        }
    }
}
