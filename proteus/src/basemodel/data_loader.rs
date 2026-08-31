use super::{
    bilerp, find_depth_indices, meters_per_degree_lat, meters_per_degree_lon, normalize_lon,
    ParticleView, Particles,
};
use gloo_net::http::Request;
use half::f16;
use std::collections::{HashMap, HashSet};
use thiserror::Error;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = "getPreloadedTile")]
    fn get_preloaded_tile(url: &str) -> Option<Vec<u8>>;
}

macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub struct TileKey {
    pub lon_idx: usize,
    pub lat_idx: usize,
    pub day: usize,
}

pub struct TileData {
    pub u: Vec<f32>,
    pub v: Vec<f32>,
    pub wind_u: Vec<f32>,
    pub wind_v: Vec<f32>,
    pub sst_k: Vec<f32>,
    pub depths: Vec<f32>,
    pub n_lon: usize,
    pub n_lat: usize,
    pub n_lon_wind: usize,
    pub n_lat_wind: usize,
    pub n_hours: usize,
    pub n_steps: usize,
}

pub struct DataLoader {
    min_lon: f32,
    min_lat: f32,
    step: f32,
    step_wind: f32,
    tile_size: f32,
    base_url: String,

    pub current_day: usize,
    pub current_hour: usize,
    pub cache: HashMap<TileKey, TileData>,
    pending: HashSet<TileKey>,
}

#[derive(Error, Debug)]
pub enum LoaderError {
    #[error("Network request failed: {0}")]
    Network(String),
    #[error("Failed to parse tile data: {0}")]
    Parse(String),
    #[error("Tile not found: {0}")]
    NotFound(String),
    #[error("HTTP error: {0}")]
    Http(u16),
}

impl DataLoader {
    pub fn new(base_url: &str, min_lon: f32, min_lat: f32) -> Self {
        Self {
            min_lon,
            min_lat,
            step: 1.0 / 12.0,
            step_wind: 1.0 / 4.0,
            tile_size: 10.0,
            base_url: base_url.to_string(),
            current_day: 0,
            current_hour: 0,
            cache: HashMap::new(),
            pending: HashSet::new(),
        }
    }

    pub fn update_tiles(&mut self, particles: &Particles) -> HashSet<TileKey> {
        let needed = self.fetch_tiles(particles);
        self.cache.retain(|k, _| needed.contains(k));
        needed
    }

    /// Load tiles for a given day. One request gets all 24 hours.
    pub async fn load_by_date(
        &mut self,
        date: usize,
        tiles: &HashSet<TileKey>,
    ) -> Result<(), LoaderError> {
        // log!("load_by_date: {} tiles: {:?}", tiles.len(), tiles.iter().map(|t| (t.lon_idx, t.lat_idx)).collect::<Vec<_>>());
        for tile in tiles {
            if self.cache.contains_key(tile) || self.pending.contains(tile) {
                continue;
            }

            self.pending.insert(tile.clone());
            let url = self.tile_url(date, tile);

            match self.load_tile(&url).await {
                Ok(data) => {
                    self.cache.insert(tile.clone(), data);
                }
                Err(e) => {
                    self.pending.remove(tile);
                    return Err(e);
                }
            }
            self.pending.remove(tile);
        }
        Ok(())
    }

    pub fn get_velocities_wind(
        &self,
        view: &ParticleView,
        day: usize,
        hour: usize,
    ) -> Vec<(f32, f32, f32, f32)> {
        let mut groups: HashMap<TileKey, Vec<(usize, f32, f32, f32)>> = HashMap::new();
        for (i, lon, lat, depth) in view.iter() {
            let lon = normalize_lon(lon);
            let key = self.get_tile_key(lon, lat, day);
            groups
                .entry(key)
                .or_insert_with(Vec::new)
                .push((i, lon, lat, depth));
        }

        let mut results = vec![(0.0, 0.0, 0.0, 0.0); view.indices.len()];

        for (key, group) in groups {
            if let Some(tile) = self.cache.get(&key) {
                let wind_step = hour / 6;

                let cells_per_hour = tile.n_lon * tile.n_lat;
                let hour_offset = hour * cells_per_hour;

                let cells_per_wind_step = tile.n_lon_wind * tile.n_lat_wind;
                let wind_step_offset = wind_step * cells_per_wind_step;

                let tile_min_lon = self.min_lon + (key.lon_idx as f32) * self.tile_size;
                let tile_min_lat = self.min_lat + (key.lat_idx as f32) * self.tile_size;

                for (idx, lon, lat, depth) in group {
                    let (lon_cell, lat_cell) = self.get_cell_index(
                        lon,
                        lat,
                        tile.n_lon,
                        tile.n_lat,
                        self.step,
                        tile_min_lon,
                        tile_min_lat,
                    );

                    let cell_lon_min = tile_min_lon + (lon_cell as f32) * self.step;
                    let cell_lat_min = tile_min_lat + (lat_cell as f32) * self.step;

                    let frac_lon = (lon - cell_lon_min) / self.step;
                    let frac_lat = (lat - cell_lat_min) / self.step;

                    let (depth_idx, _t) = find_depth_indices(&tile.depths, depth);
                    let stride = cells_per_hour;
                    let idx_bot =
                        hour_offset + depth_idx * stride + lat_cell * tile.n_lon + lon_cell;

                    let current_u_m = bilerp(&tile.u, frac_lon, frac_lat, idx_bot, tile.n_lon);
                    let current_v_m = bilerp(&tile.v, frac_lon, frac_lat, idx_bot, tile.n_lon);

                    let (wlon_cell, wlat_cell) = self.get_cell_index(
                        lon,
                        lat,
                        tile.n_lon_wind,
                        tile.n_lat_wind,
                        self.step_wind,
                        tile_min_lon,
                        tile_min_lat,
                    );

                    let wcell_lon_min = tile_min_lon + (wlon_cell as f32) * self.step_wind;
                    let wcell_lat_min = tile_min_lat + (wlat_cell as f32) * self.step_wind;

                    let wfrac_lon = (lon - wcell_lon_min) / self.step_wind;
                    let wfrac_lat = (lat - wcell_lat_min) / self.step_wind;
                    let widx = wind_step_offset + wlat_cell * tile.n_lon_wind + wlon_cell;

                    let wind_u_m =
                        bilerp(&tile.wind_u, wfrac_lon, wfrac_lat, widx, tile.n_lon_wind);
                    let wind_v_m =
                        bilerp(&tile.wind_v, wfrac_lon, wfrac_lat, widx, tile.n_lon_wind);

                    results[idx] = (
                        meters_per_degree_lon(current_u_m, lat),
                        meters_per_degree_lat(current_v_m),
                        wind_u_m,
                        wind_v_m,
                    );
                }
            }
        }

        results
    }
    pub fn get_velocities_wind_slice(
        &self,
        positions: &[(f32, f32, f32)],
        day: usize,
        hour: usize,
    ) -> Vec<(f32, f32, f32, f32)> {
        let mut groups: HashMap<TileKey, Vec<(usize, f32, f32, f32)>> = HashMap::new();
        for (i, &(lon, lat, depth)) in positions.iter().enumerate() {
            let lon = normalize_lon(lon);
            let key = self.get_tile_key(lon, lat, day);
            groups
                .entry(key)
                .or_insert_with(Vec::new)
                .push((i, lon, lat, depth));
        }

        let mut results = vec![(0.0, 0.0, 0.0, 0.0); positions.len()];

        for (key, group) in groups {
            if let Some(tile) = self.cache.get(&key) {
                let wind_step = hour / 6;

                let cells_per_hour = tile.n_lon * tile.n_lat;
                let hour_offset = hour * cells_per_hour;

                let cells_per_wind_step = tile.n_lon_wind * tile.n_lat_wind;
                let wind_step_offset = wind_step * cells_per_wind_step;

                let tile_min_lon = self.min_lon + (key.lon_idx as f32) * self.tile_size;
                let tile_min_lat = self.min_lat + (key.lat_idx as f32) * self.tile_size;

                for (idx, lon, lat, depth) in group {
                    let (lon_cell, lat_cell) = self.get_cell_index(
                        lon,
                        lat,
                        tile.n_lon,
                        tile.n_lat,
                        self.step,
                        tile_min_lon,
                        tile_min_lat,
                    );

                    let cell_lon_min = tile_min_lon + (lon_cell as f32) * self.step;
                    let cell_lat_min = tile_min_lat + (lat_cell as f32) * self.step;

                    let frac_lon = (lon - cell_lon_min) / self.step;
                    let frac_lat = (lat - cell_lat_min) / self.step;

                    let (depth_idx, _t) = find_depth_indices(&tile.depths, depth);
                    let stride = cells_per_hour;
                    let idx_bot =
                        hour_offset + depth_idx * stride + lat_cell * tile.n_lon + lon_cell;

                    let current_u_m = bilerp(&tile.u, frac_lon, frac_lat, idx_bot, tile.n_lon);
                    let current_v_m = bilerp(&tile.v, frac_lon, frac_lat, idx_bot, tile.n_lon);

                    let (wlon_cell, wlat_cell) = self.get_cell_index(
                        lon,
                        lat,
                        tile.n_lon_wind,
                        tile.n_lat_wind,
                        self.step_wind,
                        tile_min_lon,
                        tile_min_lat,
                    );

                    let wcell_lon_min = tile_min_lon + (wlon_cell as f32) * self.step_wind;
                    let wcell_lat_min = tile_min_lat + (wlat_cell as f32) * self.step_wind;

                    let wfrac_lon = (lon - wcell_lon_min) / self.step_wind;
                    let wfrac_lat = (lat - wcell_lat_min) / self.step_wind;
                    let widx = wind_step_offset + wlat_cell * tile.n_lon_wind + wlon_cell;

                    let wind_u_m =
                        bilerp(&tile.wind_u, wfrac_lon, wfrac_lat, widx, tile.n_lon_wind);
                    let wind_v_m =
                        bilerp(&tile.wind_v, wfrac_lon, wfrac_lat, widx, tile.n_lon_wind);

                    results[idx] = (
                        meters_per_degree_lon(current_u_m, lat),
                        meters_per_degree_lat(current_v_m),
                        wind_u_m,
                        wind_v_m,
                    );
                }
            }
        }

        results
    }

    pub fn get_velocities(
        &self,
        positions: &[(f32, f32, f32)],
        day: usize,
        hour: usize,
    ) -> Vec<(f32, f32)> {
        let mut groups: HashMap<TileKey, Vec<(usize, f32, f32, f32)>> = HashMap::new();
        for (i, &(lon, lat, depth)) in positions.iter().enumerate() {
            let lon = normalize_lon(lon);
            let key = self.get_tile_key(lon, lat, day);
            groups
                .entry(key)
                .or_insert_with(Vec::new)
                .push((i, lon, lat, depth));
        }

        let mut results = vec![(0.0, 0.0); positions.len()];

        for (key, group) in groups {
            if let Some(tile) = self.cache.get(&key) {
                let cells_per_hour = tile.n_lon * tile.n_lat;
                let hour_offset = hour * cells_per_hour;

                let tile_min_lon = self.min_lon + (key.lon_idx as f32) * self.tile_size;
                let tile_min_lat = self.min_lat + (key.lat_idx as f32) * self.tile_size;

                for (idx, lon, lat, depth) in group {
                    let (lon_cell, lat_cell) = self.get_cell_index(
                        lon,
                        lat,
                        tile.n_lon,
                        tile.n_lat,
                        self.step,
                        tile_min_lon,
                        tile_min_lat,
                    );

                    let cell_lon_min = tile_min_lon + (lon_cell as f32) * self.step;
                    let cell_lat_min = tile_min_lat + (lat_cell as f32) * self.step;

                    let frac_lon = (lon - cell_lon_min) / self.step;
                    let frac_lat = (lat - cell_lat_min) / self.step;

                    let (depth_idx, _t) = find_depth_indices(&tile.depths, depth);
                    let stride = cells_per_hour;
                    let idx_bot =
                        hour_offset + depth_idx * stride + lat_cell * tile.n_lon + lon_cell;

                    let current_u_m = bilerp(&tile.u, frac_lon, frac_lat, idx_bot, tile.n_lon);
                    let current_v_m = bilerp(&tile.v, frac_lon, frac_lat, idx_bot, tile.n_lon);

                    results[idx] = (
                        meters_per_degree_lon(current_u_m, lat),
                        meters_per_degree_lat(current_v_m),
                    )
                }
            }
        }

        results
    }

    pub fn get_wind_sst(
        &self,
        view: &ParticleView,
        day: usize,
        hour: usize,
    ) -> Vec<(f32, f32, f32)> {
        let mut groups: HashMap<TileKey, Vec<(usize, f32, f32)>> = HashMap::new();
        for (i, lon, lat, _) in view.iter() {
            let lon = normalize_lon(lon);
            let key = self.get_tile_key(lon, lat, day);
            groups
                .entry(key)
                .or_insert_with(Vec::new)
                .push((i, lon, lat));
        }

        let mut results = vec![(0.0, 0.0, 0.0); view.indices.len()];
        for (key, group) in groups {
            if let Some(tile) = self.cache.get(&key) {
                let wind_step = hour / 6;

                let cells_per_wind_step = tile.n_lon_wind * tile.n_lat_wind;
                let wind_step_offset = wind_step * cells_per_wind_step;

                let tile_min_lon = self.min_lon + (key.lon_idx as f32) * self.tile_size;
                let tile_min_lat = self.min_lat + (key.lat_idx as f32) * self.tile_size;

                for (idx, lon, lat) in group {
                    let (wlon_cell, wlat_cell) = self.get_cell_index(
                        lon,
                        lat,
                        tile.n_lon_wind,
                        tile.n_lat_wind,
                        self.step_wind,
                        tile_min_lon,
                        tile_min_lat,
                    );

                    let wcell_lon_min = tile_min_lon + (wlon_cell as f32) * self.step_wind;
                    let wcell_lat_min = tile_min_lat + (wlat_cell as f32) * self.step_wind;

                    let wfrac_lon = (lon - wcell_lon_min) / self.step_wind;
                    let wfrac_lat = (lat - wcell_lat_min) / self.step_wind;
                    let widx = wind_step_offset + wlat_cell * tile.n_lon_wind + wlon_cell;

                    let wind_u_m =
                        bilerp(&tile.wind_u, wfrac_lon, wfrac_lat, widx, tile.n_lon_wind);
                    let wind_v_m =
                        bilerp(&tile.wind_v, wfrac_lon, wfrac_lat, widx, tile.n_lon_wind);
                    let sst_k = bilerp(&tile.sst_k, wfrac_lon, wfrac_lat, widx, tile.n_lon_wind);

                    results[idx] = (wind_u_m, wind_v_m, sst_k);
                }
            }
        }

        results
    }

    fn fetch_tiles(&self, particles: &Particles) -> HashSet<TileKey> {
        let mut tiles = HashSet::new();

        for i in 0..particles.len {
            if particles.stranded[i] {
                continue;
            }

            let lon = normalize_lon(particles.lons[i]);
            let lat = particles.lats[i];

            let lon_idx = ((lon - self.min_lon) / self.tile_size).floor() as i32;
            let lat_idx = ((lat - self.min_lat) / self.tile_size).floor() as i32;

            if lon_idx >= 0 && lon_idx < 36 && lat_idx >= 0 && lat_idx < 17 {
                tiles.insert(TileKey {
                    lon_idx: lon_idx as usize,
                    lat_idx: lat_idx as usize,
                    day: self.current_day,
                });
            }
        }

        tiles
    }

    fn tile_url(&self, date: usize, tile: &TileKey) -> String {
        let year = date / 10000;
        let month = (date / 100) % 100;
        let day = date % 100;
        format!(
            "{}/{:04}/{:02}/{:02}/{:03}_{:03}.bin",
            self.base_url, year, month, day, tile.lon_idx, tile.lat_idx,
        )
    }

    async fn load_tile(&self, url: &str) -> Result<TileData, LoaderError> {
        // Try preloader cache first
        if let Some(bytes) = get_preloaded_tile(url) {
            return parse_tile_data(&bytes).map_err(LoaderError::Parse);
        }

        // Fall back to network if preloader missed it
        let response = Request::get(url)
            .send()
            .await
            .map_err(|e| LoaderError::Network(e.to_string()))?;

        if !response.ok() {
            return Err(LoaderError::Http(response.status()));
        }

        let bytes = response
            .binary()
            .await
            .map_err(|e| LoaderError::Network(e.to_string()))?;

        parse_tile_data(&bytes).map_err(LoaderError::Parse)
    }

    pub fn get_tile_key(&self, lon: f32, lat: f32, day: usize) -> TileKey {
        let lon_idx = ((lon - self.min_lon) / self.tile_size).floor() as i32;
        let lat_idx = ((lat - self.min_lat) / self.tile_size).floor() as i32;

        let key = TileKey {
            lon_idx: lon_idx.max(0).min(35) as usize,
            lat_idx: lat_idx.max(0).min(16) as usize,
            day,
        };

        key
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

    pub fn set_current_day(&mut self, day: usize, hour: usize) {
        self.current_day = day;
        self.current_hour = hour;
    }
}

pub fn parse_tile_data(bytes: &[u8]) -> Result<TileData, String> {
    if bytes.len() < 12 {
        return Err("File too short for header".to_string());
    }

    let n_lon = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    let n_lat = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    let n_depths = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;

    let mut depths = Vec::with_capacity(n_depths);
    let mut offset = 12;
    for _ in 0..n_depths {
        let depth_val = f32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]);
        depths.push(depth_val);
        offset += 4;
    }

    let n_cells = n_lon * n_lat;
    let n_hours = 24;

    let mut u = Vec::with_capacity(n_hours * n_depths * n_cells);
    let mut v = Vec::with_capacity(n_hours * n_depths * n_cells);

    for _ in 0..n_hours {
        for _ in 0..n_depths {
            let u_f16 = &bytes[offset..offset + n_cells * 2];
            offset += n_cells * 2;
            u.extend(
                u_f16
                    .chunks_exact(2)
                    .map(|c| f16::from_bits(u16::from_le_bytes([c[0], c[1]])).to_f32()),
            );

            let v_f16 = &bytes[offset..offset + n_cells * 2];
            offset += n_cells * 2;
            v.extend(
                v_f16
                    .chunks_exact(2)
                    .map(|c| f16::from_bits(u16::from_le_bytes([c[0], c[1]])).to_f32()),
            );
        }
    }

    // Check if wind data is present (need at least 12 bytes for wind header)
    if offset + 12 > bytes.len() {
        // No wind data — return with empty wind/SST vectors
        return Ok(TileData {
            u,
            v,
            wind_u: Vec::new(),
            wind_v: Vec::new(),
            sst_k: Vec::new(),
            n_lon,
            n_lat,
            n_lon_wind: 0,
            n_lat_wind: 0,
            depths,
            n_hours,
            n_steps: 0,
        });
    }

    // Parse wind header
    let n_lon_wind = u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ]) as usize;
    let n_lat_wind = u32::from_le_bytes([
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ]) as usize;
    let n_steps = u32::from_le_bytes([
        bytes[offset + 8],
        bytes[offset + 9],
        bytes[offset + 10],
        bytes[offset + 11],
    ]) as usize;

    offset += 12;

    let n_cells_wind = n_lon_wind * n_lat_wind;
    let wind_bytes_needed = n_steps * n_cells_wind * 2 * 3; // u + v + sst, 2 bytes each

    // Check if we have enough bytes for wind data
    if offset + wind_bytes_needed > bytes.len() {
        // Incomplete wind data — return without wind
        return Ok(TileData {
            u,
            v,
            wind_u: Vec::new(),
            wind_v: Vec::new(),
            sst_k: Vec::new(),
            n_lon,
            n_lat,
            n_lon_wind: 0,
            n_lat_wind: 0,
            depths,
            n_hours,
            n_steps: 0,
        });
    }

    let mut wind_u = Vec::with_capacity(n_steps * n_cells_wind);
    let mut wind_v = Vec::with_capacity(n_steps * n_cells_wind);
    let mut sst_k = Vec::with_capacity(n_steps * n_cells_wind);

    for _ in 0..n_steps {
        let wind_u_f16 = &bytes[offset..offset + n_cells_wind * 2];
        offset += n_cells_wind * 2;
        wind_u.extend(
            wind_u_f16
                .chunks_exact(2)
                .map(|c| f16::from_bits(u16::from_le_bytes([c[0], c[1]])).to_f32()),
        );

        let wind_v_f16 = &bytes[offset..offset + n_cells_wind * 2];
        offset += n_cells_wind * 2;
        wind_v.extend(
            wind_v_f16
                .chunks_exact(2)
                .map(|c| f16::from_bits(u16::from_le_bytes([c[0], c[1]])).to_f32()),
        );

        let sst_f16 = &bytes[offset..offset + n_cells_wind * 2];
        offset += n_cells_wind * 2;
        sst_k.extend(
            sst_f16
                .chunks_exact(2)
                .map(|c| f16::from_bits(u16::from_le_bytes([c[0], c[1]])).to_f32()),
        );
    }

    Ok(TileData {
        u,
        v,
        wind_u,
        wind_v,
        sst_k,
        n_lon,
        n_lat,
        n_lon_wind,
        n_lat_wind,
        depths,
        n_hours,
        n_steps,
    })
}
