//! preload.rs reuses required global tiles for background requests

use super::*;

impl<F: TileFetcher> GlobalForcing<F> {
    pub(super) fn update_preload(&mut self, current: &HashSet<TileKey>, date: usize) {
        self.preload.date = date;
        self.preload.spatial.clear();
        for key in current {
            self.preload.spatial.insert((key.lon_idx, key.lat_idx));
            // include the neighboring world edge conservatively for wrapped releases
            if key.lon_idx == 0 {
                self.preload.spatial.insert((35, key.lat_idx));
            }
            if key.lon_idx == 35 {
                self.preload.spatial.insert((0, key.lat_idx));
            }
        }
    }

    pub(in crate::forcing) fn plan_preload(&mut self, positions: &[f32], date: usize) {
        let current = self.tiles_for_positions(positions, date);
        self.update_preload(&current, date);
    }

    pub(in crate::forcing) fn preload_urls(&self) -> Vec<String> {
        self.preload
            .urls(&self.base_url, |_| true, |key| self.cache.contains_key(key))
    }
}
