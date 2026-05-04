use serde::{Serialize, Deserialize};
use wasm_bindgen::prelude::*;

macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into())
    }
}

// ============================================================================
// POINT STRUCT
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Point2D {
    pub x: f64,  // longitude
    pub y: f64,  // latitude
}

// ============================================================================
// EULERIAN GRID
// ============================================================================

pub struct EulerianGrid {
    lon_min: f64,
    lon_max: f64,
    lat_min: f64,
    lat_max: f64,
    cell_size: f64,
    nx: usize,
    ny: usize,
    grid: Vec<f32>,
    smooth_kernel: [f32; 9],
}

impl EulerianGrid {
    pub fn new(lon_min: f64, lon_max: f64, lat_min: f64, lat_max: f64, cell_size: f64) -> Self {
        let nx = ((lon_max - lon_min) / cell_size).ceil() as usize;
        let ny = ((lat_max - lat_min) / cell_size).ceil() as usize;
        
        Self {
            lon_min,
            lon_max,
            lat_min,
            lat_max,
            cell_size,
            nx,
            ny,
            grid: vec![0.0; nx * ny],
            smooth_kernel: [
                1.0/16.0, 2.0/16.0, 1.0/16.0,
                2.0/16.0, 4.0/16.0, 2.0/16.0,
                1.0/16.0, 2.0/16.0, 1.0/16.0,
            ],
        }
    }
    
    pub fn clear(&mut self) {
        self.grid.fill(0.0);
    }
    
    pub fn add_particle(&mut self, lon: f64, lat: f64, concentration: f32) {
        let ix = ((lon - self.lon_min) / self.cell_size).floor() as usize;
        let iy = ((lat - self.lat_min) / self.cell_size).floor() as usize;
        
        if ix < self.nx && iy < self.ny {
            let idx = iy * self.nx + ix;
            self.grid[idx] += concentration;
        }
    }
    
    pub fn add_particles(&mut self, lons: &[f64], lats: &[f64], concentrations: Option<&[f32]>) {
        for i in 0..lons.len() {
            let conc = concentrations.map_or(1.0, |c| c[i]);
            self.add_particle(lons[i], lats[i], conc);
        }
    }
    
    pub fn smooth(&mut self) {
        let mut smoothed = vec![0.0; self.grid.len()];
        
        for iy in 1..self.ny - 1 {
            for ix in 1..self.nx - 1 {
                let mut sum = 0.0;
                for ky in -1..=1 {
                    for kx in -1..=1 {
                        let grid_idx = ((iy as isize + ky) as usize) * self.nx + ((ix as isize + kx) as usize);
                        let kernel_idx = ((ky + 1) * 3 + (kx + 1)) as usize;
                        sum += self.grid[grid_idx] * self.smooth_kernel[kernel_idx];
                    }
                }
                let smoothed_idx = iy * self.nx + ix;
                smoothed[smoothed_idx] = sum;
            }
        }
        
        // Use copy_from_slice for better performance
        for iy in 1..self.ny - 1 {
            let start = iy * self.nx + 1;
            let end = start + self.nx - 2;
            self.grid[start..end].copy_from_slice(&smoothed[start..end]);
        }
    }
    
    pub fn get_max_value(&self) -> f32 {
        self.grid.iter().fold(0.0f32, |max, &val| if val > max { val } else { max })
    }
    
    pub fn get_min_max(&self) -> (f32, f32) {
        let mut min = f32::MAX;
        let mut max = f32::MIN;
        for &val in &self.grid {
            if val > 0.0 {
                if val < min { min = val; }
                if val > max { max = val; }
            }
        }
        (min, max)
    }
    
    pub fn get_bounds(&self) -> (f64, f64, f64, f64) {
        (self.lon_min, self.lon_max, self.lat_min, self.lat_max)
    }
    
    pub fn get_dimensions(&self) -> (usize, usize) {
        (self.nx, self.ny)
    }
    
    pub fn get_grid(&self) -> &[f32] {
        &self.grid
    }
    
    // Generate blocky GeoJSON (for fallback visualization)
    pub fn to_geojson(&self) -> String {
        let mut features = Vec::new();
        
        for iy in 0..self.ny {
            for ix in 0..self.nx {
                let val = self.grid[iy * self.nx + ix];
                if val == 0.0 { continue; }
                
                let lon = self.lon_min + ix as f64 * self.cell_size;
                let lat = self.lat_min + iy as f64 * self.cell_size;
                
                let coordinates = vec![
                    vec![lon, lat],
                    vec![lon + self.cell_size, lat],
                    vec![lon + self.cell_size, lat + self.cell_size],
                    vec![lon, lat + self.cell_size],
                    vec![lon, lat],
                ];
                
                let feature = serde_json::json!({
                    "type": "Feature",
                    "geometry": {
                        "type": "Polygon",
                        "coordinates": [coordinates]
                    },
                    "properties": {
                        "concentration": val
                    }
                });
                
                features.push(feature);
            }
        }
        
        let geojson = serde_json::json!({
            "type": "FeatureCollection",
            "features": features
        });
        
        geojson.to_string()
    }
    
    // Generate smooth contour GeoJSON
    pub fn to_contour_geojson(&self, thresholds: &[f32]) -> String {
        let contours = self.generate_contours(thresholds);
        let mut features = Vec::with_capacity(contours.len());
        
        for contour in &contours {
            for ring in &contour.rings {
                let coordinates: Vec<Vec<f64>> = ring.iter()
                    .map(|p| vec![p.x, p.y])
                    .collect();
                
                let feature = serde_json::json!({
                    "type": "Feature",
                    "geometry": {
                        "type": "Polygon",
                        "coordinates": [coordinates]
                    },
                    "properties": {
                        "concentration": contour.threshold
                    }
                });
                
                features.push(feature);
            }
        }
        
        let geojson = serde_json::json!({
            "type": "FeatureCollection",
            "features": features
        });
        
        geojson.to_string()
    }
    
    // Generate contours at multiple thresholds
    pub fn generate_contours(&self, thresholds: &[f32]) -> Vec<Contour> {
        let mut contours = Vec::with_capacity(thresholds.len());
        
        for &threshold in thresholds {
            let rings = self.marching_squares(threshold);
            if !rings.is_empty() {
                contours.push(Contour { threshold, rings });
            }
        }
        
        contours
    }
    
    fn marching_squares(&self, threshold: f32) -> Vec<Vec<Point2D>> {
        // Pre-allocate with estimated capacity (roughly 25% of cells will have contours)
        let estimated_capacity = (self.nx * self.ny) / 4;
        let mut polygons = Vec::with_capacity(estimated_capacity);
        
        // Pre-compute binary classifications to avoid repeated comparisons
        let classified: Vec<u8> = self.grid.iter()
            .map(|&v| if v >= threshold { 1u8 } else { 0u8 })
            .collect();
        
        let cell_size = self.cell_size;
        let lon_min = self.lon_min;
        let lat_min = self.lat_min;
        
        for y in 0..self.ny - 1 {
            let row_offset = y * self.nx;
            let next_row_offset = (y + 1) * self.nx;
            let lat = lat_min + y as f64 * cell_size;
            let next_lat = lat + cell_size;
            
            for x in 0..self.nx - 1 {
                let idx = row_offset + x;
                
                // Fast config computation using pre-classified values
                let b0 = classified[idx];
                let b1 = classified[idx + 1];
                let b2 = classified[next_row_offset + x + 1];
                let b3 = classified[next_row_offset + x];
                
                let config = b0 | (b1 << 1) | (b2 << 2) | (b3 << 3);
                
                // Skip empty cells early
                if config == 0 {
                    continue;
                }
                
                let lon = lon_min + x as f64 * cell_size;
                let next_lon = lon + cell_size;
                
                // Compute corner points
                let p0 = Point2D { x: lon, y: lat };
                let p1 = Point2D { x: next_lon, y: lat };
                let p2 = Point2D { x: next_lon, y: next_lat };
                let p3 = Point2D { x: lon, y: next_lat };
                
                // Handle full cell case early (no interpolation needed)
                if config == 15 {
                    polygons.push(vec![p0, p1, p2, p3]);
                    continue;
                }
                
                // Only fetch actual grid values we need for interpolation
                let c0 = self.grid[idx];
                let c1 = self.grid[idx + 1];
                let c2 = self.grid[next_row_offset + x + 1];
                let c3 = self.grid[next_row_offset + x];
                
                // Interpolate edge crossings
                let mb = Self::interpolate(&p0, &p1, c0, c1, threshold);
                let mr = Self::interpolate(&p1, &p2, c1, c2, threshold);
                let mt = Self::interpolate(&p3, &p2, c3, c2, threshold);
                let ml = Self::interpolate(&p0, &p3, c0, c3, threshold);
                
                // Generate polygon for this cell configuration
                let polygon = match config {
                    1 => vec![p0, mb, ml],
                    2 => vec![p1, mr, mb],
                    4 => vec![p2, mt, mr],
                    8 => vec![p3, ml, mt],
                    
                    3 => vec![p0, p1, mr, ml],
                    6 => vec![p1, p2, mt, mb],
                    12 => vec![p2, p3, ml, mr],
                    9 => vec![p0, p3, mt, mb],
                    
                    5 => vec![p0, mb, mr, p2, mt, ml],
                    10 => vec![mb, p1, mr, mt, p3, ml],
                    
                    7 => vec![p0, p1, p2, mt, ml],
                    11 => vec![p0, p1, mr, mt, p3],
                    13 => vec![p0, mb, mr, p2, p3],
                    14 => vec![mb, p1, p2, p3, ml],
                    
                    _ => vec![],
                };
                
                if !polygon.is_empty() {
                    polygons.push(polygon);
                }
            }
        }
        
        polygons
    }
    
    /// Linear interpolation along a cell edge
    #[inline]
    fn interpolate(p1: &Point2D, p2: &Point2D, v1: f32, v2: f32, threshold: f32) -> Point2D {
        let diff = v2 - v1;
        if diff.abs() < 1e-10 {
            return Point2D {
                x: (p1.x + p2.x) * 0.5,
                y: (p1.y + p2.y) * 0.5,
            };
        }
        
        let t = ((threshold - v1) / diff) as f64;
        let t = if t < 0.0 { 0.0 } else if t > 1.0 { 1.0 } else { t };
        
        Point2D {
            x: p1.x + t * (p2.x - p1.x),
            y: p1.y + t * (p2.y - p1.y),
        }
    }
}

// ============================================================================
// CONTOUR STRUCT
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contour {
    pub threshold: f32,
    pub rings: Vec<Vec<Point2D>>,
}

// ============================================================================
// WASM BINDINGS
// ============================================================================

#[wasm_bindgen]
pub struct HeatmapGenerator {
    grid: EulerianGrid,
}

#[wasm_bindgen]
impl HeatmapGenerator {
    #[wasm_bindgen(constructor)]
    pub fn new(lon_min: f64, lon_max: f64, lat_min: f64, lat_max: f64, cell_size: f64) -> Self {
        Self {
            grid: EulerianGrid::new(lon_min, lon_max, lat_min, lat_max, cell_size),
        }
    }
    
    pub fn clear(&mut self) {
        self.grid.clear();
    }
    
    pub fn add_particles(&mut self, lons: &[f64], lats: &[f64], concentrations: Option<Vec<f32>>) {
        self.grid.add_particles(lons, lats, concentrations.as_deref());
    }
    
    pub fn smooth(&mut self) {
        self.grid.smooth();
    }
    
    pub fn get_max_value(&self) -> f32 {
        self.grid.get_max_value()
    }
    
    pub fn to_geojson(&self) -> String {
        self.grid.to_geojson()
    }
    
    pub fn to_contour_geojson(&self, thresholds: &[f32]) -> String {
        self.grid.to_contour_geojson(thresholds)
    }
}