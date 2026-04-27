use std::collections::{HashMap, HashSet};
use crate::particles::Particles;
use crate::interpolation::{find_depth_indices, lerp};
use half::f16;
use thiserror::Error;
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub struct TileKey {
    pub lon_idx: usize,
    pub lat_idx: usize,
    pub day: u32,
}

pub struct TileData {
    pub u: Vec<f32>,
    pub v: Vec<f32>,
    pub m: Vec<f32>,
    pub n_lon: usize,
    pub n_lat: usize,
}

pub struct GlorysLoader {
    // Configuration
    min_lon: f32,
    max_lon: f32,
    min_lat: f32,
    max_lat: f32,
    lon_step: f32,
    lat_step: f32,
    n_lon: usize,
    n_lat: usize,
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
    pub fn new(base_url: &str, min_lon: f32, max_lon: f32, min_lat: f32, max_lat: f32) -> Self {
        let lon_step = 1.0 / 12.0;
        let lat_step = 1.0 / 12.0;
        let n_lon = ((max_lon - min_lon) / lon_step).round() as usize;
        let n_lat = ((max_lat - min_lat) / lat_step).round() as usize;
        
        Self {
            min_lon,
            max_lon,
            min_lat,
            max_lat,
            lon_step,
            lat_step,
            n_lon,
            n_lat,
            tile_size: 10.0,
            base_url: base_url.to_string(),
            current_day: 0,
            cache: HashMap::new(),
            pending: HashSet::new(),
        }
    }
    
    pub fn update_tiles(&mut self, particles: &Particles) -> HashSet<TileKey> {
        let needed = self.fetch_tiles(particles);
        //self.cache.retain(|k, _| needed.contains(k));
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
    
    pub fn get_velocity(&self, lon: f32, lat: f32, day: u32) -> Option<(f32, f32)> {
        let key = self.get_tile_key(lon, lat, day);
        let tile_data = match self.cache.get(&key) {
            Some(data) => data,
            None => {

                return None;
            }
        };
        let (lon_cell, lat_cell) = self.get_cell_index(lon, lat, tile_data);
        let idx = lat_cell * tile_data.n_lon + lon_cell;
        
        let u0 = tile_data.u[idx];
        let v0 = tile_data.v[idx];
        let u1 = tile_data.u[idx + 1];
        let v1 = tile_data.v[idx + 1];
        let u2 = tile_data.u[idx + tile_data.n_lon];
        let v2 = tile_data.v[idx + tile_data.n_lon];
        let u3 = tile_data.u[idx + tile_data.n_lon + 1];
        let v3 = tile_data.v[idx + tile_data.n_lon + 1];
        
        let lon_min = self.min_lon + (key.lon_idx as f32) * self.tile_size;
        let lat_min = self.min_lat + (key.lat_idx as f32) * self.tile_size;
        
        let x_frac = (lon - lon_min) / self.lon_step;
        let y_frac = (lat - lat_min) / self.lat_step;
        
        let u_interp = lerp(
            lerp(u0, u1, x_frac),
            lerp(u2, u3, x_frac),
            y_frac,
        );
        let v_interp = lerp(
            lerp(v0, v1, x_frac),
            lerp(v2, v3, x_frac),
            y_frac,
        );
        
        Some((u_interp, v_interp))


    }
    
    fn fetch_tiles(&self, particles: &Particles) -> HashSet<TileKey> {
        let (xmin, xmax, ymin, ymax) = particles.bounding_box();
        let lon_min_idx = ((xmin - self.min_lon) / self.tile_size).floor() as usize;
        let lon_max_idx = ((xmax - self.min_lon) / self.tile_size).ceil() as usize;
        let lat_min_idx = ((ymin - self.min_lat) / self.tile_size).floor() as usize;
        let lat_max_idx = ((ymax - self.min_lat) / self.tile_size).ceil() as usize;
        
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
        
        let n_cells = n_lon * n_lat;
        let data_bytes = n_cells * 2;
        let offset = 8;

        
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
        
        let m_start = v_end;
        let m_end = m_start + data_bytes;
        if bytes.len() < m_end {
            return Err("File too short for m data".to_string());
        }
        
        let m_f16 = &bytes[m_start..m_end];
        let m: Vec<f32> = m_f16
            .chunks_exact(2)
            .map(|chunk| {
                let bits = u16::from_le_bytes([chunk[0], chunk[1]]);
                f16::from_bits(bits).to_f32()
            })
            .collect();
        

        Ok(TileData {
            u,
            v,
            m,
            n_lon,
            n_lat,
        })
    }
    
    async fn load_tile(&self, url: &str) -> Result<TileData, LoaderError> {
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
        
        let lon_cell = ((lon - tile_min_lon) / self.lon_step).round() as usize;
        let lat_cell = ((lat - tile_min_lat) / self.lat_step).round() as usize;
        
        let lon_cell = lon_cell.clamp(0, tile.n_lon - 1);
        let lat_cell = lat_cell.clamp(0, tile.n_lat - 1);
        
        (lon_cell, lat_cell)
    }
    // In glorysloader.rs
    pub fn set_current_day(&mut self, day: u32) {
        self.current_day = day;
    }
}
