use std::collections::{HashMap, HashSet};
use crate::particles::Particles;
use crate::interpolation::{find_depth_indices, lerp};
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
    cache: HashMap<TileKey, TileData>,
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
    
    pub fn update_tiles(&mut self, particles: &Particles) -> HashSet<TileKey> {
        let needed = self.fetch_tiles(particles);
        self.cache.retain(|k, _| needed.contains(k));
        needed
    }
    
    /// Load tiles for a given day. One request gets all 24 hours.
    pub async fn load_by_date(&mut self, date: u32, tiles: &HashSet<TileKey>) -> Result<(), LoaderError> {
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
    
    /// Get velocity at a specific hour. Hour is an INDEX (0-23) into the loaded daily tile.
    pub fn get_velocity(&self, lon: f32, lat: f32, depth_m: f32, day: u32, hour: u32) -> Option<(f32, f32)> {
        let key = self.get_tile_key(lon, lat, day);
        let tile_data = self.cache.get(&key)?;
        
        // Clamp hour to what's available
        let h = (hour as usize).min(tile_data.n_hours.saturating_sub(1));
        
        let (lon_cell, lat_cell) = self.get_cell_index(lon, lat, tile_data, self.lon_step, self.lat_step);
        let (depth_idx, t) = find_depth_indices(&tile_data.depths, depth_m);

        let cells_per_hour = tile_data.n_lon * tile_data.n_lat;
        let hour_offset = h * cells_per_hour;  // Jump to this hour's data
        let stride = cells_per_hour;
        
        let idx_bot = hour_offset + depth_idx * stride + lat_cell * tile_data.n_lon + lon_cell;
        
        // Bottom layer corners
        let u0 = tile_data.u[idx_bot];
        let v0 = tile_data.v[idx_bot];
        let u1 = tile_data.u[idx_bot + 1];
        let v1 = tile_data.v[idx_bot + 1];
        let u2 = tile_data.u[idx_bot + tile_data.n_lon];
        let v2 = tile_data.v[idx_bot + tile_data.n_lon];
        let u3 = tile_data.u[idx_bot + tile_data.n_lon + 1];
        let v3 = tile_data.v[idx_bot + tile_data.n_lon + 1];

        // Vertical interpolation
        // let (uz0, vz0, uz1, vz1, uz2, vz2, uz3, vz3) = 
        //     if depth_idx + 1 < tile_data.depths.len() {
        //         let idx_top = hour_offset + (depth_idx + 1) * stride + lat_cell * tile_data.n_lon + lon_cell;
        //         (
        //             lerp(u0, tile_data.u[idx_top], t),
        //             lerp(v0, tile_data.v[idx_top], t),
        //             lerp(u1, tile_data.u[idx_top + 1], t),
        //             lerp(v1, tile_data.v[idx_top + 1], t),
        //             lerp(u2, tile_data.u[idx_top + tile_data.n_lon], t),
        //             lerp(v2, tile_data.v[idx_top + tile_data.n_lon], t),
        //             lerp(u3, tile_data.u[idx_top + tile_data.n_lon + 1], t),
        //             lerp(v3, tile_data.v[idx_top + tile_data.n_lon + 1], t),
        //         )
        //     } else {
        //         (u0, v0, u1, v1, u2, v2, u3, v3)
        //     };
        
        // Bilinear interpolation
        let tile_min_lon = self.min_lon + (key.lon_idx as f32) * self.tile_size;
        let tile_min_lat = self.min_lat + (key.lat_idx as f32) * self.tile_size;
        let cell_lon_min = tile_min_lon + (lon_cell as f32) * self.lon_step;
        let cell_lat_min = tile_min_lat + (lat_cell as f32) * self.lat_step;
        let x_frac = (lon - cell_lon_min) / self.lon_step;
        let y_frac = (lat - cell_lat_min) / self.lat_step;
        
        let u_interp = lerp(lerp(u0, u1, x_frac), lerp(u2, u3, x_frac), y_frac);
        let v_interp = lerp(lerp(v0, v1, x_frac), lerp(v2, v3, x_frac), y_frac);
        
        let meters_per_degree_lat = 111_120.0;
        let meters_per_degree_lon = 111_120.0 * lat.to_radians().cos();
        
        Some((
            u_interp / meters_per_degree_lon,
            v_interp / meters_per_degree_lat,
        ))
    }
    pub fn get_wind(&self, lon: f32, lat: f32, day: u32, hour: u32) -> Option<(f32, f32)> {
        let key = self.get_tile_key(lon, lat, day);
        let tile_data = self.cache.get(&key)?;
        
        // Return None if tile has no wind data
        if tile_data.n_steps == 0 || tile_data.u_wind.is_empty() {
            return None;
        }
        let wind_step = ((hour / 6) as usize).min(tile_data.n_steps.saturating_sub(1));
        
        let (lon_cell, lat_cell) = self.get_cell_index(
            lon, lat, tile_data, self.lon_step_wind, self.lat_step_wind
        );
        
        let cells_per_step = tile_data.n_lon_wind * tile_data.n_lat_wind;
        let step_offset = wind_step * cells_per_step;
        
        let idx = step_offset + lat_cell * tile_data.n_lon_wind + lon_cell;
        
        let u0 = tile_data.u_wind[idx];
        let v0 = tile_data.v_wind[idx];
        let u1 = tile_data.u_wind[idx + 1];
        let v1 = tile_data.v_wind[idx + 1];
        let u2 = tile_data.u_wind[idx + tile_data.n_lon_wind];
        let v2 = tile_data.v_wind[idx + tile_data.n_lon_wind];
        let u3 = tile_data.u_wind[idx + tile_data.n_lon_wind + 1];
        let v3 = tile_data.v_wind[idx + tile_data.n_lon_wind + 1];
        
        let tile_min_lon = self.min_lon + (key.lon_idx as f32) * self.tile_size;
        let tile_min_lat = self.min_lat + (key.lat_idx as f32) * self.tile_size;
        let cell_lon_min = tile_min_lon + (lon_cell as f32) * self.lon_step_wind;
        let cell_lat_min = tile_min_lat + (lat_cell as f32) * self.lat_step_wind;
        let x_frac = (lon - cell_lon_min) / self.lon_step_wind;
        let y_frac = (lat - cell_lat_min) / self.lat_step_wind;
        
        let u_interp = lerp(lerp(u0, u1, x_frac), lerp(u2, u3, x_frac), y_frac);
        let v_interp = lerp(lerp(v0, v1, x_frac), lerp(v2, v3, x_frac), y_frac);
        
        let wind_speed = (u_interp * u_interp + v_interp * v_interp).sqrt().max(0.1);
        let theta_deg = 25.0 * (-wind_speed.powi(3) / 1184.75).exp();
        let theta = if lat >= 0.0 { theta_deg.to_radians() } else { -theta_deg.to_radians() };
        let cos_t = theta.cos();
        let sin_t = theta.sin();
        
        let u_drift = 0.03 * (u_interp * cos_t - v_interp * sin_t);
        let v_drift = 0.03 * (u_interp * sin_t + v_interp * cos_t);
        
        // Convert m/s to deg/s
        let meters_per_degree_lat = 111_120.0;
        let meters_per_degree_lon = 111_120.0 * lat.to_radians().cos();
        
        Some((
            u_drift / meters_per_degree_lon,
            v_drift / meters_per_degree_lat,
        ))
    }
    pub fn get_velocities_wind_batch_grouped(
        &self,
        positions: &[(f32, f32, f32)],
        day: u32,
        hour: u32,
    ) -> Vec<((f32, f32), (f32, f32))> {
        let mut groups: HashMap<TileKey, Vec<(usize, (f32, f32, f32))>> = HashMap::new();
        
        for (i, &(lon, lat, depth)) in positions.iter().enumerate() {
            let key = self.get_tile_key(lon, lat, day);
            groups.entry(key).or_insert_with(Vec::new).push((i, (lon, lat, depth)));
        }
        
        let mut results = vec![((0.0, 0.0), (0.0, 0.0)); positions.len()];
        
        for (key, group) in groups {
            if let Some(tile) = self.cache.get(&key) {
                let has_wind = tile.n_steps > 0 && !tile.u_wind.is_empty();
                
                let h = (hour as usize).min(tile.n_hours.saturating_sub(1));
                let wind_step = if has_wind {
                    ((hour / 6) as usize).min(tile.n_steps.saturating_sub(1))
                } else {
                    0
                };
                
                // Current data
                let cells_per_hour = tile.n_lon * tile.n_lat;
                let hour_offset = h * cells_per_hour;
                
                // Wind data
                let cells_per_step = if has_wind { tile.n_lon_wind * tile.n_lat_wind } else { 1 };
                let step_offset = if has_wind { wind_step * cells_per_step } else { 0 };
                
                let tile_min_lon = self.min_lon + (key.lon_idx as f32) * self.tile_size;
                let tile_min_lat = self.min_lat + (key.lat_idx as f32) * self.tile_size;
                
                for (idx, (lon, lat, depth)) in group {
                    // Current velocity (bilinear at 1/12°)
                    let (lon_cell, lat_cell) = self.get_cell_index(lon, lat, tile, self.lon_step, self.lat_step);
                    let cell_lon_min = tile_min_lon + (lon_cell as f32) * self.lon_step;
                    let cell_lat_min = tile_min_lat + (lat_cell as f32) * self.lat_step;
                    let x_frac = ((lon - cell_lon_min) / self.lon_step).clamp(0.0, 1.0);
                    let y_frac = ((lat - cell_lat_min) / self.lat_step).clamp(0.0, 1.0);
                    
                    let (depth_idx, t) = find_depth_indices(&tile.depths, depth);
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
                    
                    // Wind drift (bilinear at 0.25°) — only if wind data available
                    let wind = if has_wind {
                        let (wlon_cell, wlat_cell) = self.get_cell_index(lon, lat, tile, self.lon_step_wind, self.lat_step_wind);
                        let wcell_lon_min = tile_min_lon + (wlon_cell as f32) * self.lon_step_wind;
                        let wcell_lat_min = tile_min_lat + (wlat_cell as f32) * self.lat_step_wind;
                        let wx_frac = ((lon - wcell_lon_min) / self.lon_step_wind).clamp(0.0, 1.0);
                        let wy_frac = ((lat - wcell_lat_min) / self.lat_step_wind).clamp(0.0, 1.0);
                        
                        let w_idx = step_offset + wlat_cell * tile.n_lon_wind + wlon_cell;
                        
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
                        
                        let wind_speed = (u_wind * u_wind + v_wind * v_wind).sqrt().max(0.1);
                        let theta_deg = 25.0 * (-wind_speed.powi(3) / 1184.75).exp();
                        let theta = if lat >= 0.0 { theta_deg.to_radians() } else { -theta_deg.to_radians() };
                        let cos_t = theta.cos();
                        let sin_t = theta.sin();
                        
                        let u_drift = 0.03 * (u_wind * cos_t - v_wind * sin_t);
                        let v_drift = 0.03 * (u_wind * sin_t + v_wind * cos_t);
                        
                        (u_drift / meters_per_degree_lon, v_drift / meters_per_degree_lat)
                    } else {
                        (0.0, 0.0)
                    };
                    
                    results[idx] = (current, wind);
                }
            }
        }
        
        results
    }

    fn fetch_tiles(&self, particles: &Particles) -> HashSet<TileKey> {
        let (xmin, xmax, ymin, ymax) = particles.bounding_box();
        if xmin == f32::MAX {
            return HashSet::new();
        }
        
        let lon_min_idx = ((xmin - self.min_lon) / self.tile_size).floor() as usize;
        let lon_max_idx = ((xmax - self.min_lon) / self.tile_size).floor() as usize;
        let lat_min_idx = ((ymin - self.min_lat) / self.tile_size).floor() as usize;
        let lat_max_idx = ((ymax - self.min_lat) / self.tile_size).floor() as usize;
        
        let mut tiles = HashSet::new();
        for lon_idx in lon_min_idx..=lon_max_idx {
            for lat_idx in lat_min_idx..=lat_max_idx {
                tiles.insert(TileKey {
                    lon_idx,
                    lat_idx,
                    day: self.current_day,
                });
            }
        }
        tiles
    }
    
    // UPDATED: No hour in path
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
        let lon_idx = ((lon - self.min_lon) / self.tile_size).floor() as usize;
        let lat_idx = ((lat - self.min_lat) / self.tile_size).floor() as usize;
        TileKey { lon_idx, lat_idx, day }
    }
    
    pub fn get_cell_index(&self, lon: f32, lat: f32, tile: &TileData, lon_step: f32, lat_step: f32) -> (usize, usize) {
        let tile_min_lon = self.min_lon + ((lon - self.min_lon) / self.tile_size).floor() * self.tile_size;
        let tile_min_lat = self.min_lat + ((lat - self.min_lat) / self.tile_size).floor() * self.tile_size;
        
        let lon_cell = ((lon - tile_min_lon) / lon_step).floor() as usize;
        let lat_cell = ((lat - tile_min_lat) / lat_step).floor() as usize;
        
        (lon_cell.clamp(0, tile.n_lon - 2), lat_cell.clamp(0, tile.n_lat - 2))
    }
    
    pub fn set_current_day(&mut self, day: u32, hour: u32) {
        self.current_day = day;
        self.current_hour = hour;
    }
}