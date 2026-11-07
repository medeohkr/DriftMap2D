//! simulation.rs bins engine particles directly into reusable heatmap storage

use super::EulerianGrid;
use proteus_core::{basemodel::Particles, tracers::TracerKind};

pub(crate) struct SimulationHeatmap {
    grid: EulerianGrid,
}

impl Default for SimulationHeatmap {
    fn default() -> Self {
        Self {
            grid: EulerianGrid::new(0.0, 0.0, 0.0, 0.0, 1.0),
        }
    }
}

impl SimulationHeatmap {
    fn rebuild(&mut self, particles: &Particles, cell_size: f64, smoothing: f64) {
        // use the same float32 bounds and double-precision longitude shift as the previous bridge
        let (mut lon_min, mut lon_max, mut lat_min, mut lat_max) =
            (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        let (mut shifted_min, mut shifted_max) = (f64::INFINITY, f64::NEG_INFINITY);
        for i in 0..particles.len {
            if particles.stranded[i] {
                continue;
            }
            let lon = particles.lons[i];
            let lat = particles.lats[i];
            lon_min = lon_min.min(lon);
            lon_max = lon_max.max(lon);
            lat_min = lat_min.min(lat);
            lat_max = lat_max.max(lat);
            let shifted = if lon < 0.0 {
                lon as f64 + 360.0
            } else {
                lon as f64
            };
            shifted_min = shifted_min.min(shifted);
            shifted_max = shifted_max.max(shifted);
        }
        let shift = lon_max as f64 - lon_min as f64 > 180.0;
        let (left, right) = if shift {
            (shifted_min, shifted_max)
        } else {
            (lon_min as f64, lon_max as f64)
        };
        let padding = cell_size * (smoothing + 1.0);
        self.grid.reset(
            left - padding,
            right + padding,
            lat_min as f64 - padding,
            lat_max as f64 + padding,
            cell_size,
        );
        for i in 0..particles.len {
            if particles.stranded[i] {
                continue;
            }
            let mass = match &particles.tracer {
                TracerKind::Generic(tracer) => tracer.data.mass_per_particle,
                TracerKind::Oil(tracer) => tracer.data.total_mass[i],
                TracerKind::Leeway(_) => 1.0,
            };
            let mut lon = particles.lons[i] as f64;
            if shift && lon < 0.0 {
                lon += 360.0;
            }
            self.grid.add_particle(lon, particles.lats[i] as f64, mass);
        }
        for _ in 0..smoothing as usize {
            self.grid.smooth();
        }
        if matches!(&particles.tracer, TracerKind::Leeway(_)) {
            self.grid.normalize_probability();
        }
    }

    pub(crate) fn geojson(
        &mut self,
        particles: &Particles,
        cell_size: f64,
        smoothing: f64,
        levels: &[f32],
    ) -> String {
        self.rebuild(particles, cell_size, smoothing);
        if matches!(&particles.tracer, TracerKind::Leeway(_)) {
            self.grid.to_probability_contour_geojson(levels)
        } else {
            self.grid.to_contour_geojson(levels)
        }
    }
}

#[cfg(test)]
mod tests;
