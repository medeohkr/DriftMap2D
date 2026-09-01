use gloo_net::http::Request;
use roaring::RoaringBitmap;
use std::collections::{HashMap, HashSet};
use wasm_bindgen::prelude::*;
use super::Particles;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = "getPreloadedTile")]
    fn get_preloaded_tile(url: &str) -> Option<Vec<u8>>;
}

pub struct LandMaskLoader {
    min_lon: f32,
    min_lat: f32,
    max_lat: f32,
    tile_size: f32,
    resolution_deg: f32,
    cells_per_tile: u32,
    base_url: String,
    cache: HashMap<(usize, usize), RoaringBitmap>,
    loaded_tiles: HashSet<(usize, usize)>,
}

impl LandMaskLoader {
    pub fn new(base_url: &str, min_lon: f32, min_lat: f32, max_lat: f32) -> Self {
        let tile_size = 10.0;
        let resolution_deg = 1.0 / 240.0;
        let cells_per_tile = 2400; // Hardcoded

        Self {
            min_lon,
            min_lat,
            max_lat,
            tile_size,
            resolution_deg,
            cells_per_tile,
            base_url: base_url.to_string(),
            cache: HashMap::new(),
            loaded_tiles: HashSet::new()
        }
    }

    pub fn update_tiles(&mut self, particles: &Particles) -> HashSet<(usize, usize)> {
        let mut needed = HashSet::new();
        for i in 0..particles.len {
            if particles.stranded[i] { continue; }
            let lon_idx = ((particles.lons[i] + 180.0) / 10.0).floor() as i32;
            let lat_idx = ((particles.lats[i] + 90.0) / 10.0).floor() as i32;
            if lon_idx >= 0 && lon_idx < 36 && lat_idx >= 0 && lat_idx < 18 {
                needed.insert((lon_idx as usize, lat_idx as usize));
            }
        }

        self.loaded_tiles.retain(|t| needed.contains(t));

        needed.difference(&self.loaded_tiles).cloned().collect()
    }

    pub async fn load_tile(&mut self, lon_idx: usize, lat_idx: usize) -> Result<(), String> {
        let url = format!(
            "{}/landmask_{:03}_{:03}.bin",
            self.base_url, lon_idx, lat_idx
        );

        let bytes = if let Some(preloaded) = get_preloaded_tile(&url) {
            preloaded
        } else {
            let response = Request::get(&url)
                .send()
                .await
                .map_err(|e| format!("Network error: {}", e))?;

            if !response.ok() {
                return Err(format!("HTTP {}", response.status()));
            }

            response
                .binary()
                .await
                .map_err(|e| format!("Binary error: {}", e))?
        };

        if bytes.len() < 8 {
            return Err("File too short".to_string());
        }

        let bitmap = RoaringBitmap::deserialize_from(&bytes[8..])
            .map_err(|e| format!("Failed to deserialize bitmap: {:?}", e))?;

        self.cache.insert((lon_idx, lat_idx), bitmap);
        self.loaded_tiles.insert((lon_idx, lat_idx));
        Ok(())
    }

    pub fn is_on_land(&self, lon: f32, lat: f32) -> bool {
        // Check bounds
        if lat < self.min_lat || lat > self.max_lat {
            return false;
        }

        let lon_idx_raw = (lon - self.min_lon) / self.tile_size;
        let lat_idx_raw = (lat - self.min_lat) / self.tile_size;
        let lon_idx = lon_idx_raw.floor() as i32;
        let lat_idx = lat_idx_raw.floor() as i32;

        if lon_idx < 0 || lat_idx < 0 {
            return false;
        }

        let lon_idx = lon_idx as usize;
        let lat_idx = lat_idx as usize;

        if let Some(bitmap) = self.cache.get(&(lon_idx, lat_idx)) {
            let tile_min_lon = self.min_lon + (lon_idx as f32) * self.tile_size;
            let tile_min_lat = self.min_lat + (lat_idx as f32) * self.tile_size;

            let ix = ((lon - tile_min_lon) / self.resolution_deg).floor() as u32;

            let iy = ((lat - tile_min_lat) / self.resolution_deg).floor() as u32;

            if ix >= self.cells_per_tile || iy >= self.cells_per_tile {
                return false;
            }

            let idx = iy * self.cells_per_tile + ix;
            return bitmap.contains(idx);
        }

        false
    }
}
