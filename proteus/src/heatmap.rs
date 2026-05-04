// heatmap.rs
use serde::{Serialize, Deserialize};
use wasm_bindgen::prelude::*;

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
        
        for iy in 1..self.ny - 1 {
            for ix in 1..self.nx - 1 {
                let idx = iy * self.nx + ix;
                self.grid[idx] = smoothed[idx];
            }
        }
    }
    
    pub fn get_max_value(&self) -> f32 {
        self.grid.iter().fold(0.0, |max, &val| max.max(val))
    }
    
    pub fn get_min_max(&self) -> (f32, f32) {
        let mut min = f32::MAX;
        let mut max = f32::MIN;
        for &val in &self.grid {
            if val > 0.0 {
                min = min.min(val);
                max = max.max(val);
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
        let mut features = Vec::new();
        
        for contour in &contours {
            for ring in &contour.rings {
                let coordinates: Vec<[f64; 2]> = ring.iter()
                    .map(|p| [p.x, p.y])
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
        let mut contours = Vec::new();
        
        for &threshold in thresholds {
            let rings = self.marching_squares(threshold);
            if !rings.is_empty() {
                contours.push(Contour { threshold, rings });
            }
        }
        
        contours
    }
    
    // Marching squares algorithm for a single threshold
    fn marching_squares(&self, threshold: f32) -> Vec<Vec<Point2D>> {
        let mut rings = Vec::new();
        
        for y in 0..self.ny - 1 {
            for x in 0..self.nx - 1 {
                let idx = y * self.nx + x;
                
                let v00 = self.grid[idx];
                let v10 = self.grid[idx + 1];
                let v01 = self.grid[(y + 1) * self.nx + x];
                let v11 = self.grid[(y + 1) * self.nx + x + 1];
                
                let c00 = if v00 >= threshold { 1 } else { 0 };
                let c10 = if v10 >= threshold { 1 } else { 0 };
                let c01 = if v01 >= threshold { 1 } else { 0 };
                let c11 = if v11 >= threshold { 1 } else { 0 };
                
                let config = c00 | (c10 << 1) | (c11 << 2) | (c01 << 3);
                
                if config == 0 || config == 15 {
                    continue;
                }
                
                let lon = self.lon_min + x as f64 * self.cell_size;
                let lat = self.lat_min + y as f64 * self.cell_size;
                
                let interpolate = |a: f32, b: f32, p1: &Point2D, p2: &Point2D| -> Point2D {
                    if (a - b).abs() < 1e-6 {
                        return p1.clone();
                    }
                    let t = (threshold - a) as f64 / (b - a) as f64;
                    Point2D {
                        x: p1.x + (p2.x - p1.x) * t,
                        y: p1.y + (p2.y - p1.y) * t,
                    }
                };
                
                let mut points = Vec::new();
                
                // Bottom edge
                if c00 != c10 {
                    points.push(interpolate(v00, v10,
                        &Point2D { x: lon, y: lat },
                        &Point2D { x: lon + self.cell_size, y: lat }));
                }
                
                // Right edge
                if c10 != c11 {
                    points.push(interpolate(v10, v11,
                        &Point2D { x: lon + self.cell_size, y: lat },
                        &Point2D { x: lon + self.cell_size, y: lat + self.cell_size }));
                }
                
                // Top edge
                if c11 != c01 {
                    points.push(interpolate(v11, v01,
                        &Point2D { x: lon + self.cell_size, y: lat + self.cell_size },
                        &Point2D { x: lon, y: lat + self.cell_size }));
                }
                
                // Left edge
                if c01 != c00 {
                    points.push(interpolate(v01, v00,
                        &Point2D { x: lon, y: lat + self.cell_size },
                        &Point2D { x: lon, y: lat }));
                }
                
                if points.len() >= 3 {
                    let center_x = lon + self.cell_size / 2.0;
                    let center_y = lat + self.cell_size / 2.0;
                    points.sort_by(|a, b| {
                        let angle_a = (a.y - center_y).atan2(a.x - center_x);
                        let angle_b = (b.y - center_y).atan2(b.x - center_x);
                        angle_a.partial_cmp(&angle_b).unwrap()
                    });
                    rings.push(points);
                }
            }
        }
        
        self.merge_contours(rings)
    }
    
    // Merge adjacent contour segments
    fn merge_contours(&self, mut segments: Vec<Vec<Point2D>>) -> Vec<Vec<Point2D>> {
        if segments.len() <= 1 {
            return segments;
        }
        
        let mut merged = Vec::new();
        let mut used = vec![false; segments.len()];
        
        for i in 0..segments.len() {
            if used[i] {
                continue;
            }
            
            let mut current = segments[i].clone();
            used[i] = true;
            
            let mut changed = true;
            while changed {
                changed = false;
                for j in 0..segments.len() {
                    if used[j] {
                        continue;
                    }
                    
                    let seg = &segments[j];
                    if seg.is_empty() {
                        used[j] = true;
                        continue;
                    }
                    
                    let epsilon = 1e-6;
                    let first_current = current.first().unwrap();
                    let last_current = current.last().unwrap();
                    let first_seg = seg.first().unwrap();
                    let last_seg = seg.last().unwrap();
                    
                    let dist_last_first = ((last_current.x - first_seg.x).powi(2) + (last_current.y - first_seg.y).powi(2)).sqrt();
                    let dist_last_last = ((last_current.x - last_seg.x).powi(2) + (last_current.y - last_seg.y).powi(2)).sqrt();
                    let dist_first_first = ((first_current.x - first_seg.x).powi(2) + (first_current.y - first_seg.y).powi(2)).sqrt();
                    let dist_first_last = ((first_current.x - last_seg.x).powi(2) + (first_current.y - last_seg.y).powi(2)).sqrt();
                    
                    if dist_last_first < epsilon {
                        current.extend(seg.iter().skip(1).cloned());
                        used[j] = true;
                        changed = true;
                    } else if dist_last_last < epsilon {
                        let mut rev = seg.clone();
                        rev.reverse();
                        current.extend(rev.iter().skip(1).cloned());
                        used[j] = true;
                        changed = true;
                    } else if dist_first_first < epsilon {
                        let mut new = seg.clone();
                        new.extend(current.iter().skip(1).cloned());
                        current = new;
                        used[j] = true;
                        changed = true;
                    } else if dist_first_last < epsilon {
                        let mut rev = seg.clone();
                        rev.reverse();
                        rev.extend(current.iter().skip(1).cloned());
                        current = rev;
                        used[j] = true;
                        changed = true;
                    }
                }
            }
            
            merged.push(current);
        }
        
        merged
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