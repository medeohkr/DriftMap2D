use std::collections::{HashMap, HashSet};
use crate::particles::Particles;
use crate::interpolation::{find_depth_indices, lerp};
use half::f16;
use thiserror::Error;
use gloo_net::http::Request;

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
    pub depths: Vec<f32>,
    pub n_lon: usize,
    pub n_lat: usize,
}

pub struct GlorysLoader {
    // Configuration
    min_lon: f32,
    min_lat: f32,
    lon_step: f32,
    lat_step: f32,
    tile_size: f32,
    base_url: String,
    
    // State
    current_day: u32,
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

impl GlorysLoader {
    pub fn new(base_url: &str, min_lon: f32, min_lat: f32) -> Self {
        let lon_step = 1.0 / 12.0;
        let lat_step = 1.0 / 12.0;
        
        Self {
            min_lon,
            min_lat,
            lon_step,
            lat_step,
            tile_size: 10.0,
            base_url: base_url.to_string(),
            current_day: 0,
            cache: HashMap::new(),
            pending: HashSet::new(),
        }
    }
    
    pub fn update_tiles(&mut self, particles: &Particles) -> HashSet<TileKey> {
        let needed = self.fetch_tiles(particles);
        self.cache.retain(|k, _| needed.contains(k));
        needed
    }
    
    /// Async load tiles for a given date
    pub async fn load_by_date(&mut self, date: u32, tiles: &HashSet<TileKey>) -> Result<(), LoaderError> {
        for tile in tiles {
            
            if self.cache.contains_key(tile) {
                continue;
            }
            
            if self.pending.contains(tile) {
                continue;
            }
            
            self.pending.insert(tile.clone());
            // web_sys::console::log_1(&format!(
            //     "CACHE INSERT: ({}, {}) day={}. Cache size now: {}", 
            //     tile.lon_idx, tile.lat_idx, tile.day, self.cache.len()
            // ).into());
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
    
    pub fn get_velocity(&self, lon: f32, lat: f32, depth_m: f32, day: u32) -> Option<(f32, f32)> {
        let key = self.get_tile_key(lon, lat, day);

        let tile_data = match self.cache.get(&key) {
            Some(data) => data,
            None => return None,
        };
        
        let (lon_cell, lat_cell) = self.get_cell_index(lon, lat, tile_data);
        let (depth_idx, t) = find_depth_indices(&tile_data.depths, depth_m);

        let stride = tile_data.n_lon * tile_data.n_lat;
        let idx_bot = depth_idx * stride + lat_cell * tile_data.n_lon + lon_cell;
        
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
        let (uz0, vz0, uz1, vz1, uz2, vz2, uz3, vz3) = 
            if depth_idx + 1 < tile_data.depths.len() {
                let idx_top = (depth_idx + 1) * stride + lat_cell * tile_data.n_lon + lon_cell;
                (
                    lerp(u0, tile_data.u[idx_top], t),
                    lerp(v0, tile_data.v[idx_top], t),
                    lerp(u1, tile_data.u[idx_top + 1], t),
                    lerp(v1, tile_data.v[idx_top + 1], t),
                    lerp(u2, tile_data.u[idx_top + tile_data.n_lon], t),
                    lerp(v2, tile_data.v[idx_top + tile_data.n_lon], t),
                    lerp(u3, tile_data.u[idx_top + tile_data.n_lon + 1], t),
                    lerp(v3, tile_data.v[idx_top + tile_data.n_lon + 1], t),
                )
            } else {
                (u0, v0, u1, v1, u2, v2, u3, v3)
            };
        
        // CORRECTED: Get the exact lon/lat of the bottom-left cell
        let tile_min_lon = self.min_lon + (key.lon_idx as f32) * self.tile_size;
        let tile_min_lat = self.min_lat + (key.lat_idx as f32) * self.tile_size;
        
        let cell_lon_min = tile_min_lon + (lon_cell as f32) * self.lon_step;
        let cell_lat_min = tile_min_lat + (lat_cell as f32) * self.lat_step;
        
        // Fractions are now between 0 and 1
        let x_frac = (lon - cell_lon_min) / self.lon_step;
        let y_frac = (lat - cell_lat_min) / self.lat_step;
        
        // Bilinear interpolation
        let u_interp = lerp(
            lerp(uz0, uz1, x_frac),
            lerp(uz2, uz3, x_frac),
            y_frac,
        );
        let v_interp = lerp(
            lerp(vz0, vz1, x_frac),
            lerp(vz2, vz3, x_frac),
            y_frac,
        );
        let meters_per_degree_lat = 111000.0;  // Approximately constant
        let meters_per_degree_lon = 111000.0 * lat.to_radians().cos();
        
        let u_deg_per_s = u_interp / meters_per_degree_lon;
        let v_deg_per_s = v_interp / meters_per_degree_lat;
        
        Some((u_deg_per_s, v_deg_per_s))
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
    
    fn tile_url(&self, date: u32, tile: &TileKey) -> String {
        let year = date / 10000;
        let month = (date / 100) % 100;
        let day = date % 100;
        format!(
            "{}/{:04}/{:02}/{:02}/{:03}_{:03}.bin",
            self.base_url,
            year,
            month,
            day,
            tile.lon_idx,
            tile.lat_idx,
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
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ]);
            depths.push(depth_val);
            offset += 4;
        }
        
        let n_cells = n_lon * n_lat;
        let data_bytes = n_cells * 2;
        
        let u_start = offset;
        let u_end = u_start + data_bytes;
        if bytes.len() < u_end {
            return Err("File too short for u data".to_string());
        }
        
        let u_f16 = &bytes[u_start..u_end];
        let u: Vec<f32> = u_f16
            .chunks_exact(2)
            .map(|chunk| {
                let bits = u16::from_le_bytes([chunk[0], chunk[1]]);
                f16::from_bits(bits).to_f32()
            })
            .collect();
        
        let v_start = u_end;
        let v_end = v_start + data_bytes;
        if bytes.len() < v_end {
            return Err("File too short for v data".to_string());
        }
        
        let v_f16 = &bytes[v_start..v_end];
        let v: Vec<f32> = v_f16
            .chunks_exact(2)
            .map(|chunk| {
                let bits = u16::from_le_bytes([chunk[0], chunk[1]]);
                f16::from_bits(bits).to_f32()
            })
            .collect();
        
        Ok(TileData {
            u,
            v,
            n_lon,
            n_lat,
            depths,
        })
    }
    
    async fn load_tile(&self, url: &str) -> Result<TileData, LoaderError> {
        // web_sys::console::log_1(&format!("Trying to load: {}", url).into());
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
    
    pub fn get_cell_index(&self, lon: f32, lat: f32, tile: &TileData) -> (usize, usize) {
        let tile_min_lon = self.min_lon + ((lon - self.min_lon) / self.tile_size).floor() * self.tile_size;
        let tile_min_lat = self.min_lat + ((lat - self.min_lat) / self.tile_size).floor() * self.tile_size;
        
        let lon_cell = ((lon - tile_min_lon) / self.lon_step).floor() as usize;
        let lat_cell = ((lat - tile_min_lat) / self.lat_step).floor() as usize;
        
        let lon_cell = lon_cell.clamp(0, tile.n_lon - 2);
        let lat_cell = lat_cell.clamp(0, tile.n_lat - 2);
        
        (lon_cell, lat_cell)
    }
    // In glorysloader.rs
    pub fn set_current_day(&mut self, day: u32) {
        self.current_day = day;
    }
}
