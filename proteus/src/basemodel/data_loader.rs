use std::collections::{HashMap, HashSet};
use crate::basemodel::{Particles, find_depth_indices, lerp};
use crate::tracers::Tracer;
use half::f16;
use thiserror::Error;
use gloo_net::http::Request;
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
    pub day: u32,
}

pub struct TileData {
    pub u: Vec<f32>,
    pub v: Vec<f32>, 
    pub u_wind: Vec<f32>,
    pub v_wind: Vec<f32>,
    pub sst: Vec<f32>,
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
    lon_step: f32,
    lat_step: f32,
    lon_step_wind: f32,
    lat_step_wind: f32,
    tile_size: f32,
    base_url: String,
    
    pub current_day: u32,
    pub current_hour: u32,
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
            lon_step: 1.0 / 12.0,
            lat_step: 1.0 / 12.0,
            lon_step_wind: 1.0 / 4.0,
            lat_step_wind: 1.0 / 4.0,
            tile_size: 10.0,
            base_url: base_url.to_string(),
            current_day: 0,
            current_hour: 0,
            cache: HashMap::new(),
            pending: HashSet::new(),
        }
    }
    
    pub fn update_tiles<T: Tracer>(&mut self, particles: &Particles<T>) -> HashSet<TileKey> {
        let needed = self.fetch_tiles(particles);
        self.cache.retain(|k, _| needed.contains(k));
        needed
    }
    
    /// Load tiles for a given day. One request gets all 24 hours.
    pub async fn load_by_date(&mut self, date: u32, tiles: &HashSet<TileKey>) -> Result<(), LoaderError> {
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
    
    pub fn get_velocities_wind_sst(
        &self,
        positions: &[(f32, f32, f32)],
        day: u32,
        hour: u32,
    ) -> Vec<((f32, f32), (f32, f32), f32)> {
        let mut groups: HashMap<TileKey, Vec<(usize, (f32, f32, f32))>> = HashMap::new();
        for (i, &(lon, lat, depth)) in positions.iter().enumerate() {
            let lon = self.normalize_lon(lon);
            let key = self.get_tile_key(lon, lat, day);
            groups.entry(key).or_insert_with(Vec::new).push((i, (lon, lat, depth)));
        }

        let mut results = vec![((0.0, 0.0), (0.0, 0.0), 0.0_f32); positions.len()];

        for (key, group) in groups {
            if let Some(tile) = self.cache.get(&key) {
                let has_wind = tile.n_steps > 0 && !tile.u_wind.is_empty();
                let has_sst = !tile.sst.is_empty();

                let h = (hour as usize).min(tile.n_hours.saturating_sub(1));
                let wind_step = if has_wind {
                    ((hour / 6) as usize).min(tile.n_steps.saturating_sub(1))
                } else {
                    0
                };

                let cells_per_hour = tile.n_lon * tile.n_lat;
                let hour_offset = h * cells_per_hour;

                let cells_per_step = if has_wind { tile.n_lon_wind * tile.n_lat_wind } else { 1 };
                let step_offset = if has_wind { wind_step * cells_per_step } else { 0 };

                let tile_min_lon = self.min_lon + (key.lon_idx as f32) * self.tile_size;
                let tile_min_lat = self.min_lat + (key.lat_idx as f32) * self.tile_size;

                for (idx, (lon, lat, depth)) in group {
                    // ---- Current velocity (bilinear at 1/12°) ----
                    let (lon_cell, lat_cell) = self.get_cell_index(lon, lat, tile.n_lon, tile.n_lat, self.lon_step, self.lat_step, tile_min_lon, tile_min_lat);
                    let cell_lon_min = tile_min_lon + (lon_cell as f32) * self.lon_step;
                    let cell_lat_min = tile_min_lat + (lat_cell as f32) * self.lat_step;
                    let x_frac = ((lon - cell_lon_min) / self.lon_step).clamp(0.0, 1.0);
                    let y_frac = ((lat - cell_lat_min) / self.lat_step).clamp(0.0, 1.0);

                    let (depth_idx, _t) = find_depth_indices(&tile.depths, depth);
                    let stride = cells_per_hour;
                    let idx_bot = hour_offset + depth_idx * stride + lat_cell * tile.n_lon + lon_cell;

                    let cu0 = tile.u[idx_bot];
                    let cu1 = tile.u[idx_bot + 1];
                    let cu2 = tile.u[idx_bot + tile.n_lon];
                    let cu3 = tile.u[idx_bot + tile.n_lon + 1];
                    let cv0 = tile.v[idx_bot];
                    let cv1 = tile.v[idx_bot + 1];
                    let cv2 = tile.v[idx_bot + tile.n_lon];
                    let cv3 = tile.v[idx_bot + tile.n_lon + 1];

                    let u_current = lerp(lerp(cu0, cu1, x_frac), lerp(cu2, cu3, x_frac), y_frac);
                    let v_current = lerp(lerp(cv0, cv1, x_frac), lerp(cv2, cv3, x_frac), y_frac);

                    let meters_per_degree_lat = 111_120.0;
                    let meters_per_degree_lon = 111_120.0 * lat.to_radians().cos();

                    let current = (
                        u_current / meters_per_degree_lon,
                        v_current / meters_per_degree_lat,
                    );

                    // ---- Wind (raw 10m components, no drift angle applied) ----
                    let wind_raw = if has_wind {
                        let (wlon_cell, wlat_cell) = self.get_cell_index(lon, lat, tile.n_lon_wind, tile.n_lat_wind, self.lon_step_wind, self.lat_step_wind, tile_min_lon, tile_min_lat);
                        let wcell_lon_min = tile_min_lon + (wlon_cell as f32) * self.lon_step_wind;
                        let wcell_lat_min = tile_min_lat + (wlat_cell as f32) * self.lat_step_wind;
                        let wx_frac = ((lon - wcell_lon_min) / self.lon_step_wind).clamp(0.0, 1.0);
                        let wy_frac = ((lat - wcell_lat_min) / self.lat_step_wind).clamp(0.0, 1.0);

                        let w_idx = step_offset + wlat_cell * tile.n_lon_wind + wlon_cell;
                        // Before the line that panics, check bounds
                        let idx = step_offset + wlat_cell * tile.n_lon_wind + wlon_cell;
                        if idx + 1 >= tile.u_wind.len() || idx + tile.n_lon_wind >= tile.u_wind.len() {
                            log!("WIND BOUNDS: idx={}, n_lon_wind={}, step_offset={}, wlat_cell={}, wlon_cell={}, len={}", 
                                idx, tile.n_lon_wind, step_offset, wlat_cell, wlon_cell, tile.u_wind.len());
                        }
                        let wu0 = tile.u_wind[w_idx];
                        let wv0 = tile.v_wind[w_idx];
                        let wu1 = tile.u_wind[w_idx + 1];
                        let wv1 = tile.v_wind[w_idx + 1];
                        let wu2 = tile.u_wind[w_idx + tile.n_lon_wind];
                        let wv2 = tile.v_wind[w_idx + tile.n_lon_wind];
                        let wu3 = tile.u_wind[w_idx + tile.n_lon_wind + 1];
                        let wv3 = tile.v_wind[w_idx + tile.n_lon_wind + 1];

                        let u_wind = lerp(lerp(wu0, wu1, wx_frac), lerp(wu2, wu3, wx_frac), wy_frac);
                        let v_wind = lerp(lerp(wv0, wv1, wx_frac), lerp(wv2, wv3, wx_frac), wy_frac);

                        (u_wind, v_wind)
                    } else {
                        (0.0, 0.0)
                    };

                    // ---- SST (bilinear at 0.25°) ----
                    let sst = if has_sst && has_wind {
                        let (wlon_cell, wlat_cell) = self.get_cell_index(lon, lat, tile.n_lon_wind, tile.n_lat_wind, self.lon_step_wind, self.lat_step_wind, tile_min_lon, tile_min_lat);
                        let wcell_lon_min = tile_min_lon + (wlon_cell as f32) * self.lon_step_wind;
                        let wcell_lat_min = tile_min_lat + (wlat_cell as f32) * self.lat_step_wind;
                        let wx_frac = ((lon - wcell_lon_min) / self.lon_step_wind).clamp(0.0, 1.0);
                        let wy_frac = ((lat - wcell_lat_min) / self.lat_step_wind).clamp(0.0, 1.0);

                        let s_idx = step_offset + wlat_cell * tile.n_lon_wind + wlon_cell;

                        let s0 = tile.sst[s_idx];
                        let s1 = tile.sst[s_idx + 1];
                        let s2 = tile.sst[s_idx + tile.n_lon_wind];
                        let s3 = tile.sst[s_idx + tile.n_lon_wind + 1];

                        lerp(lerp(s0, s1, wx_frac), lerp(s2, s3, wx_frac), wy_frac)
                    } else {
                        20.0 // fallback to reference temperature if no SST data
                    };
                    // log!("{sst}");
                    results[idx] = (current, wind_raw, sst);
                }
            }
        }

        results
    }
    /// Get just current velocities (no wind/SST) for multiple positions
    pub fn get_velocities_batch(
        &self,
        positions: &[(f32, f32)],
        depth: f32,
        day: u32,
        hour: u32,
    ) -> Vec<(f32, f32)> {
        let positions_3d: Vec<(f32, f32, f32)> = positions.iter()
            .map(|&(lon, lat)| (lon, lat, depth))
            .collect();
        
        let env_data = self.get_velocities_wind_sst(&positions_3d, day, hour);
        
        env_data.into_iter()
            .map(|((cu, cv), _, _)| (cu, cv))
            .collect()
    }
    fn fetch_tiles<T: Tracer>(&self, particles: &Particles<T>) -> HashSet<TileKey> {
        let mut tiles = HashSet::new();
        
        for i in 0..particles.len {
            if !particles.active[i] || particles.stranded[i] {
                continue;
            }
            
            let lon = particles.x[i];
            let lat = particles.y[i];
            
            // Normalize before calculating tile index
            let lon = self.normalize_lon(lon);
            
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
    
    fn tile_url(&self, date: u32, tile: &TileKey) -> String {
        let year = date / 10000;
        let month = (date / 100) % 100;
        let day = date % 100;
        format!(
            "{}/{:04}/{:02}/{:02}/{:03}_{:03}.bin",
            self.base_url, year, month, day,
            tile.lon_idx, tile.lat_idx,
        )
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
                bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3],
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
                    u_f16.chunks_exact(2).map(|c| f16::from_bits(u16::from_le_bytes([c[0], c[1]])).to_f32())
                );
                
                let v_f16 = &bytes[offset..offset + n_cells * 2];
                offset += n_cells * 2;
                v.extend(
                    v_f16.chunks_exact(2).map(|c| f16::from_bits(u16::from_le_bytes([c[0], c[1]])).to_f32())
                );
            }
        }
        
        // Check if wind data is present (need at least 12 bytes for wind header)
        if offset + 12 > bytes.len() {
            // No wind data — return with empty wind/SST vectors
            return Ok(TileData {
                u, v,
                u_wind: Vec::new(),
                v_wind: Vec::new(),
                sst: Vec::new(),
                n_lon, n_lat,
                n_lon_wind: 0, n_lat_wind: 0,
                depths, n_hours, n_steps: 0,
            });
        }
        
        // Parse wind header
        let n_lon_wind = u32::from_le_bytes([bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]]) as usize;
        let n_lat_wind = u32::from_le_bytes([bytes[offset + 4], bytes[offset + 5], bytes[offset + 6], bytes[offset + 7]]) as usize;
        let n_steps = u32::from_le_bytes([bytes[offset + 8], bytes[offset + 9], bytes[offset + 10], bytes[offset + 11]]) as usize;

        offset += 12;
        
        let n_cells_wind = n_lon_wind * n_lat_wind;
        let wind_bytes_needed = n_steps * n_cells_wind * 2 * 3; // u + v + sst, 2 bytes each
        
        // Check if we have enough bytes for wind data
        if offset + wind_bytes_needed > bytes.len() {
            // Incomplete wind data — return without wind
            return Ok(TileData {
                u, v,
                u_wind: Vec::new(),
                v_wind: Vec::new(),
                sst: Vec::new(),
                n_lon, n_lat,
                n_lon_wind: 0, n_lat_wind: 0,
                depths, n_hours, n_steps: 0,
            });
        }
        
        let mut u_wind = Vec::with_capacity(n_steps * n_cells_wind);
        let mut v_wind = Vec::with_capacity(n_steps * n_cells_wind);
        let mut sst = Vec::with_capacity(n_steps * n_cells_wind);
        
        for _ in 0..n_steps {
            let u_wind_f16 = &bytes[offset..offset + n_cells_wind * 2];
            offset += n_cells_wind * 2;
            u_wind.extend(
                u_wind_f16.chunks_exact(2).map(|c| f16::from_bits(u16::from_le_bytes([c[0], c[1]])).to_f32())
            );
            
            let v_wind_f16 = &bytes[offset..offset + n_cells_wind * 2];
            offset += n_cells_wind * 2;
            v_wind.extend(
                v_wind_f16.chunks_exact(2).map(|c| f16::from_bits(u16::from_le_bytes([c[0], c[1]])).to_f32())
            );
            
            let sst_f16 = &bytes[offset..offset + n_cells_wind * 2];
            offset += n_cells_wind * 2;
            sst.extend(
                sst_f16.chunks_exact(2).map(|c| f16::from_bits(u16::from_le_bytes([c[0], c[1]])).to_f32())
            );
        }
        
        Ok(TileData {
            u, v,
            u_wind, v_wind, sst,
            n_lon, n_lat,
            n_lon_wind, n_lat_wind,
            depths, n_hours, n_steps,
        })
    }
    async fn load_tile(&self, url: &str) -> Result<TileData, LoaderError> {
        // Try preloader cache first
        if let Some(bytes) = get_preloaded_tile(url) {
            return Self::parse_tile_data(&bytes).map_err(LoaderError::Parse);
        }
        
        // Fall back to network if preloader missed it
        let response = Request::get(url)
            .send()
            .await
            .map_err(|e| LoaderError::Network(e.to_string()))?;
        
        if !response.ok() {
            return Err(LoaderError::Http(response.status()));
        }
        
        let bytes = response.binary()
            .await
            .map_err(|e| LoaderError::Network(e.to_string()))?;
        
        Self::parse_tile_data(&bytes).map_err(LoaderError::Parse)
    }
    
    pub fn get_tile_key(&self, lon: f32, lat: f32, day: u32) -> TileKey {
        let lon_idx = ((lon - self.min_lon) / self.tile_size).floor() as i32;
        let lat_idx = ((lat - self.min_lat) / self.tile_size).floor() as i32;
        
        let key = TileKey {
            lon_idx: lon_idx.max(0).min(35) as usize,
            lat_idx: lat_idx.max(0).min(16) as usize,
            day,
        };
        
        key
    }
    
    pub fn get_cell_index(&self, lon: f32, lat: f32, n_lon: usize, n_lat: usize, lon_step: f32, lat_step: f32, tile_min_lon: f32, tile_min_lat: f32) -> (usize, usize) {
        let lon_cell = ((lon - tile_min_lon) / lon_step).floor() as i32;
        let lat_cell = ((lat - tile_min_lat) / lat_step).floor() as i32;
        
        let lon_cell = lon_cell.max(0).min(n_lon as i32 - 2) as usize;
        let lat_cell = lat_cell.max(0).min(n_lat as i32 - 2) as usize;
        
        (lon_cell, lat_cell)
    }
    
    pub fn set_current_day(&mut self, day: u32, hour: u32) {
        self.current_day = day;
        self.current_hour = hour;
    }
    pub fn normalize_lon(&self, lon: f32) -> f32 {
        let mut lon = lon;
        while lon < -180.0 { lon += 360.0; }
        while lon >= 180.0 { lon -= 360.0; }
        lon
    }
}