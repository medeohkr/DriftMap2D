// heatmap.rs builds smoothed concentration and probability contours

mod contours;

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

const EPSILON: f32 = 1.0e-12;
const OCCUPIED_THRESHOLD_FACTOR: f32 = 1.0e-6;

// ============================================================================
// Geometry
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point2D {
    pub x: f64, // longitude
    pub y: f64, // latitude
}

// ============================================================================
// Contours
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contour {
    pub threshold: f32,
    pub rings: Vec<Vec<Point2D>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbabilityContour {
    pub probability: f32,
    pub density_threshold: f32,
    pub rings: Vec<Vec<Point2D>>,
}

// ============================================================================
// Grid
// ============================================================================

pub struct EulerianGrid {
    lon_min: f64,
    lat_min: f64,
    cell_size: f64,
    nx: usize,
    ny: usize,

    // This stores either:
    // - concentration/weight for ordinary heatmaps; or
    // - probability mass for SAR mode.
    grid: Vec<f32>,

    smooth_kernel: [f32; 9],
}

impl EulerianGrid {
    pub fn new(lon_min: f64, lon_max: f64, lat_min: f64, lat_max: f64, cell_size: f64) -> Self {
        let valid_bounds =
            lon_min < lon_max && lat_min < lat_max && cell_size.is_finite() && cell_size > 0.0;

        if !valid_bounds {
            return Self {
                lon_min: 0.0,
                lat_min: 0.0,
                cell_size: 1.0,
                nx: 1,
                ny: 1,
                grid: vec![0.0],
                smooth_kernel: Self::default_kernel(),
            };
        }

        let nx = (((lon_max - lon_min) / cell_size).ceil() as usize).max(2);
        let ny = (((lat_max - lat_min) / cell_size).ceil() as usize).max(2);

        Self {
            lon_min,
            lat_min,
            cell_size,
            nx,
            ny,
            grid: vec![0.0; nx * ny],
            smooth_kernel: Self::default_kernel(),
        }
    }

    fn default_kernel() -> [f32; 9] {
        [
            1.0 / 16.0,
            2.0 / 16.0,
            1.0 / 16.0,
            2.0 / 16.0,
            4.0 / 16.0,
            2.0 / 16.0,
            1.0 / 16.0,
            2.0 / 16.0,
            1.0 / 16.0,
        ]
    }

    pub fn clear(&mut self) {
        self.grid.fill(0.0);
    }

    #[inline]
    fn grid_index(&self, ix: usize, iy: usize) -> usize {
        iy * self.nx + ix
    }

    fn coordinate_to_cell(&self, lon: f64, lat: f64) -> Option<(usize, usize)> {
        if !lon.is_finite() || !lat.is_finite() {
            return None;
        }

        let fx = (lon - self.lon_min) / self.cell_size;
        let fy = (lat - self.lat_min) / self.cell_size;

        if fx < 0.0 || fy < 0.0 {
            return None;
        }

        let ix = fx.floor() as usize;
        let iy = fy.floor() as usize;

        if ix >= self.nx || iy >= self.ny {
            return None;
        }

        Some((ix, iy))
    }

    pub fn add_particle(&mut self, lon: f64, lat: f64, weight: f32) {
        if !weight.is_finite() || weight <= 0.0 {
            return;
        }

        let Some((ix, iy)) = self.coordinate_to_cell(lon, lat) else {
            return;
        };

        let index = self.grid_index(ix, iy);
        self.grid[index] += weight;
    }

    pub fn add_particles(&mut self, lons: &[f64], lats: &[f64], weights: Option<&[f32]>) {
        let n = lons.len().min(lats.len());

        for i in 0..n {
            let weight = match weights {
                Some(values) => values.get(i).copied().unwrap_or(0.0),
                None => 1.0,
            };

            self.add_particle(lons[i], lats[i], weight);
        }
    }

    // ========================================================================
    // Probability operations
    // ========================================================================

    pub fn total_mass(&self) -> f32 {
        self.grid
            .iter()
            .copied()
            .filter(|value| value.is_finite() && *value > 0.0)
            .sum()
    }

    pub fn normalize_probability(&mut self) {
        let total = self.total_mass();

        if total <= EPSILON || !total.is_finite() {
            return;
        }

        for value in &mut self.grid {
            if value.is_finite() && *value > 0.0 {
                *value /= total;
            } else {
                *value = 0.0;
            }
        }
    }

    // For equal-sized cells, probability mass and density have the same
    // ordering. This is appropriate for the current regular degree grid.
    pub fn hdr_density_threshold(&self, target_probability: f32) -> Option<f32> {
        let mut values: Vec<f32> = self
            .grid
            .iter()
            .copied()
            .filter(|value| value.is_finite() && *value > 0.0)
            .collect();

        if values.is_empty() {
            return None;
        }

        values.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

        let total: f32 = values.iter().sum();

        if total <= EPSILON || !total.is_finite() {
            return None;
        }

        let target = target_probability.clamp(0.0, 1.0) * total;
        let mut cumulative = 0.0;

        for value in values.iter().copied() {
            cumulative += value;

            if cumulative + EPSILON >= target {
                return Some(value);
            }
        }

        values.last().copied()
    }

    pub fn occupied_threshold(&self) -> Option<f32> {
        let maximum = self
            .grid
            .iter()
            .copied()
            .filter(|value| value.is_finite() && *value > 0.0)
            .fold(0.0_f32, f32::max);

        if maximum <= EPSILON {
            None
        } else {
            Some(maximum * OCCUPIED_THRESHOLD_FACTOR)
        }
    }

    // ========================================================================
    // Smoothing
    // ========================================================================

    pub fn smooth(&mut self) {
        if self.grid.is_empty() || self.nx < 2 || self.ny < 2 {
            return;
        }

        let mut smoothed = vec![0.0; self.grid.len()];

        for iy in 0..self.ny {
            for ix in 0..self.nx {
                let mut sum = 0.0;

                for ky in -1..=1 {
                    for kx in -1..=1 {
                        let source_x = (ix as isize + kx).clamp(0, self.nx as isize - 1) as usize;

                        let source_y = (iy as isize + ky).clamp(0, self.ny as isize - 1) as usize;

                        let kernel_index = ((ky + 1) * 3 + (kx + 1)) as usize;

                        let source_index = self.grid_index(source_x, source_y);

                        sum += self.grid[source_index] * self.smooth_kernel[kernel_index];
                    }
                }

                smoothed[self.grid_index(ix, iy)] = sum;
            }
        }

        self.grid = smoothed;
    }

    // ========================================================================
    // Ordinary concentration contours
    // ========================================================================

    pub fn generate_contours(&self, thresholds: &[f32]) -> Vec<Contour> {
        thresholds
            .iter()
            .copied()
            .filter(|threshold| threshold.is_finite())
            .filter_map(|threshold| {
                let rings = self.marching_squares(threshold);

                if rings.is_empty() {
                    None
                } else {
                    Some(Contour { threshold, rings })
                }
            })
            .collect()
    }

    pub fn to_contour_geojson(&self, thresholds: &[f32]) -> String {
        let contours = self.generate_contours(thresholds);
        let mut features = Vec::new();

        for contour in contours {
            features.push(contours::feature(contour.rings, contour.threshold));
        }

        serde_json::json!({
            "type": "FeatureCollection",
            "features": features
        })
        .to_string()
    }

    // ========================================================================
    // SAR probability contours
    // ========================================================================

    pub fn generate_probability_contours(&self, probabilities: &[f32]) -> Vec<ProbabilityContour> {
        let mut contours = Vec::with_capacity(probabilities.len());

        for &probability in probabilities {
            let threshold = if probability >= 0.999_999 {
                self.occupied_threshold()
            } else {
                self.hdr_density_threshold(probability)
            };

            let Some(density_threshold) = threshold else {
                continue;
            };

            let rings = self.marching_squares(density_threshold);

            if !rings.is_empty() {
                contours.push(ProbabilityContour {
                    probability,
                    density_threshold,
                    rings,
                });
            }
        }

        contours
    }

    pub fn to_probability_contour_geojson(&self, probabilities: &[f32]) -> String {
        let mut sorted_probabilities = probabilities.to_vec();
        sorted_probabilities.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

        let contours = self.generate_probability_contours(&sorted_probabilities);

        let mut features = Vec::new();

        for contour in contours {
            features.push(contours::feature(contour.rings, contour.probability));
        }

        serde_json::json!({
            "type": "FeatureCollection",
            "features": features
        })
        .to_string()
    }

    // ========================================================================
    // Marching squares
    // ========================================================================

    fn marching_squares(&self, threshold: f32) -> Vec<Vec<Point2D>> {
        if self.nx < 2 || self.ny < 2 {
            return Vec::new();
        }
        let classified: Vec<u8> = self
            .grid
            .iter()
            .map(|&value| u8::from(value.is_finite() && value >= threshold))
            .collect();
        let mut boundaries = contours::Boundaries::default();
        for y in 0..self.ny - 1 {
            for x in 0..self.nx - 1 {
                let idx = self.grid_index(x, y);
                let [b0, b1, b2, b3] = [
                    classified[idx],
                    classified[idx + 1],
                    classified[idx + self.nx + 1],
                    classified[idx + self.nx],
                ];
                let config = b0 | (b1 << 1) | (b2 << 2) | (b3 << 3);
                if config == 0
                    || (config == 15 && x > 0 && y > 0 && x + 2 < self.nx && y + 2 < self.ny)
                {
                    continue;
                }
                let mb = self.crossing(x, y, true, threshold);
                let mr = self.crossing(x + 1, y, false, threshold);
                let mt = self.crossing(x, y + 1, true, threshold);
                let ml = self.crossing(x, y, false, threshold);
                // interior grid edges cancel by construction so only contour edges are retained
                match config {
                    1 => boundaries.edge(mb, ml),
                    2 => boundaries.edge(mr, mb),
                    3 => boundaries.edge(mr, ml),
                    4 => boundaries.edge(mt, mr),
                    5 => {
                        boundaries.edge(mb, mr);
                        boundaries.edge(mt, ml);
                    }
                    6 => boundaries.edge(mt, mb),
                    7 => boundaries.edge(mt, ml),
                    8 => boundaries.edge(ml, mt),
                    9 => boundaries.edge(mb, mt),
                    10 => {
                        boundaries.edge(mr, mt);
                        boundaries.edge(ml, mb);
                    }
                    11 => boundaries.edge(mr, mt),
                    12 => boundaries.edge(ml, mr),
                    13 => boundaries.edge(mb, mr),
                    14 => boundaries.edge(ml, mb),
                    _ => {}
                }
                if y == 0 {
                    boundaries.border(self.corner(x, y), self.corner(x + 1, y), mb, b0, b1);
                }
                if x + 2 == self.nx {
                    boundaries.border(self.corner(x + 1, y), self.corner(x + 1, y + 1), mr, b1, b2);
                }
                if y + 2 == self.ny {
                    boundaries.border(self.corner(x + 1, y + 1), self.corner(x, y + 1), mt, b2, b3);
                }
                if x == 0 {
                    boundaries.border(self.corner(x, y + 1), self.corner(x, y), ml, b3, b0);
                }
            }
        }
        boundaries
            .finish()
            .into_iter()
            .map(|ring| {
                ring.into_iter()
                    .map(|p| Point2D {
                        x: self.lon_min + p.x * self.cell_size,
                        y: self.lat_min + p.y * self.cell_size,
                    })
                    .collect()
            })
            .collect()
    }

    fn corner(&self, x: usize, y: usize) -> contours::Vertex {
        contours::Vertex {
            id: self.grid_index(x, y) * 3,
            point: Point2D {
                x: x as f64,
                y: y as f64,
            },
        }
    }

    /** shared edges and exact threshold corners use the same vertex identity */
    fn crossing(&self, x: usize, y: usize, horizontal: bool, threshold: f32) -> contours::Vertex {
        let [dx, dy] = if horizontal { [1, 0] } else { [0, 1] };
        let start = self.grid_index(x, y);
        let end = self.grid_index(x + dx, y + dy);
        let difference = self.grid[end] - self.grid[start];
        let t = if !difference.is_finite() || difference.abs() < 1.0e-10 {
            0.5
        } else {
            ((threshold - self.grid[start]) / difference).clamp(0.0, 1.0) as f64
        };
        if t == 0.0 {
            return self.corner(x, y);
        }
        if t == 1.0 {
            return self.corner(x + dx, y + dy);
        }
        contours::Vertex {
            id: start * 3 + if horizontal { 1 } else { 2 },
            point: Point2D {
                x: x as f64 + dx as f64 * t,
                y: y as f64 + dy as f64 * t,
            },
        }
    }
}

// ============================================================================
// WASM bindings
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

    pub fn add_particles(&mut self, lons: &[f64], lats: &[f64], weights: Option<Vec<f32>>) {
        self.grid.add_particles(lons, lats, weights.as_deref());
    }

    pub fn normalize_probability(&mut self) {
        self.grid.normalize_probability();
    }

    pub fn smooth(&mut self, n: usize) {
        for _ in 0..n {
            self.grid.smooth();
        }
    }

    pub fn to_contour_geojson(&self, thresholds: &[f32]) -> String {
        self.grid.to_contour_geojson(thresholds)
    }

    pub fn to_probability_contour_geojson(&self, probabilities: &[f32]) -> String {
        self.grid.to_probability_contour_geojson(probabilities)
    }

    pub fn probability_thresholds(&self, probabilities: &[f32]) -> Vec<f32> {
        probabilities
            .iter()
            .map(|&probability| {
                if probability >= 0.999_999 {
                    self.grid.occupied_threshold().unwrap_or(0.0)
                } else {
                    self.grid.hdr_density_threshold(probability).unwrap_or(0.0)
                }
            })
            .collect()
    }
}
#[cfg(test)]
mod tests;
