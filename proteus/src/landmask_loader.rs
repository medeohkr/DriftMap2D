use std::collections::HashMap;
use half::f16;
use gloo_net::http::Request;

pub struct LandMaskLoader {
    min_lon: f32,
    min_lat: f32,
    tile_size: f32,
    lon_step: f32,
    lat_step: f32,
    base_url: String,
    cache: HashMap<(usize, usize), Vec<f32>>,
}

impl LandMaskLoader {
    pub fn new(base_url: &str, min_lon: f32, min_lat: f32) -> Self {
        Self {
            min_lon,
            min_lat,
            tile_size: 10.0,
            lon_step: 1.0 / 12.0,
            lat_step: 1.0 / 12.0,
            base_url: base_url.to_string(),
            cache: HashMap::new(),
        }
    }

    pub async fn load_tile(&mut self, lon_idx: usize, lat_idx: usize) -> Result<(), String> {
        if self.cache.contains_key(&(lon_idx, lat_idx)) {
            return Ok(());
        }

        let url = format!(
            "{}/{:03}_{:03}.bin",
            self.base_url, lon_idx, lat_idx
        );

        let response = Request::get(&url)
            .send()
            .await
            .map_err(|e| format!("Network error: {}", e))?;

        if !response.ok() {
            return Err(format!("HTTP {}", response.status()));
        }

        let bytes = response.binary()
            .await
            .map_err(|e| format!("Binary error: {}", e))?;

        // Parse same format as current tiles
        let n_lon = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        let n_lat = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
        
        let n_cells = n_lon * n_lat;
        let land_frac: Vec<f32> = bytes[16..16 + n_cells * 2]
            .chunks_exact(2)
            .map(|c| f16::from_bits(u16::from_le_bytes([c[0], c[1]])).to_f32())
            .collect();

        self.cache.insert((lon_idx, lat_idx), land_frac);
        Ok(())
    }

    pub fn is_on_land(&self, lon: f32, lat: f32) -> bool {
        let lon_idx = ((lon - self.min_lon) / self.tile_size).floor() as usize;
        let lat_idx = ((lat - self.min_lat) / self.tile_size).floor() as usize;

        if let Some(land_frac) = self.cache.get(&(lon_idx, lat_idx)) {
            let tile_min_lon = self.min_lon + lon_idx as f32 * self.tile_size;
            let tile_min_lat = self.min_lat + lat_idx as f32 * self.tile_size;
            let lon_cell = ((lon - tile_min_lon) / self.lon_step) as usize;
            let lat_cell = ((lat - tile_min_lat) / self.lat_step) as usize;
            
            // Tile dimensions from your current format
            let n_lon = 120;
            let n_lat = 120;
            
            if lon_cell < n_lon && lat_cell < n_lat {
                let idx = lat_cell * n_lon + lon_cell;
                if idx < land_frac.len() {
                    return land_frac[idx] > 0.5;
                }
            }
        }
        false
    }
}