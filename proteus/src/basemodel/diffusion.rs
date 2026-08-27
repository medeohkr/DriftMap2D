use rand::prelude::*;
use rand_distr::{Normal, Distribution};
use super::{DataLoader, ParticleView, normalize_lon};
macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}
const METERS_PER_DEGREE: f32 = 111_120.0;
const CELL_AREA_DEG2: f32 = 1.0 / 144.0;

pub struct Diffusion {
    cs: f32,
    normal: Normal<f32>,
    rng: ThreadRng,
}

impl Diffusion {
    pub fn new(cs: f32) -> Self {
        let normal = Normal::new(0.0, 1.0).unwrap();
        let rng = rand::thread_rng();
        Self { cs, normal, rng }
    }

    pub fn smagorinsky_step(
        &mut self,
        loader: &DataLoader,
        old_view: &ParticleView,
        positions: &[(f32, f32, f32)],
        day: usize,
        dt_days: f32,
        hour: usize,
    ) -> Vec<(f32, f32)> {
        let dx = 0.01;
        let dy = 0.01;

        let n = positions.len();
        let mut probes = Vec::with_capacity(n);
        let mut final_positions = Vec::with_capacity(n);


        for (i, &(lon, lat, depth)) in positions.iter().enumerate() {
            let mid_lon = (old_view.lon(i) + lon) / 2.0;
            let mid_lat = (old_view.lat(i) + lat) / 2.0;

            probes.push((normalize_lon(mid_lon - dx), mid_lat, depth));
            probes.push((normalize_lon(mid_lon + dx), mid_lat, depth));
            probes.push((mid_lon, (mid_lat - dy).clamp(-80.0, 90.0), depth));
            probes.push((mid_lon, (mid_lat + dy).clamp(-80.0, 90.0), depth));
        }

        let velocities = loader.get_velocities(&probes, day, hour);

        for (i, &(lon, lat, _)) in positions.iter().enumerate() {
            let base = i * 4;

            let vel_lon_minus = velocities[base];
            let vel_lon_plus = velocities[base + 1];
            let vel_lat_minus = velocities[base + 2];
            let vel_lat_plus = velocities[base + 3];

            if  vel_lon_minus.0 == 0.0 ||
                vel_lon_minus.1 == 0.0 ||
                vel_lon_plus.0 == 0.0 ||
                vel_lon_plus.1 == 0.0 ||
                vel_lat_minus.0 == 0.0 ||
                vel_lat_minus.1 == 0.0 ||
                vel_lat_plus.0 == 0.0 ||
                vel_lat_plus.1 == 0.0
             {
                final_positions.push((lon, lat));
                continue;
            }

            let dudx = (vel_lon_plus.0 - vel_lon_minus.0) / (2.0 * dx);
            let dudy = (vel_lat_plus.0 - vel_lat_minus.0) / (2.0 * dy);
            let dvdx = (vel_lon_plus.1 - vel_lon_minus.1) / (2.0 * dx);
            let dvdy = (vel_lat_plus.1 - vel_lat_minus.1) / (2.0 * dy);

            let strain = (dudx.powi(2) + 0.5 * (dudy + dvdx).powi(2) + dvdy.powi(2)).sqrt();

            let deg2_to_m2 = METERS_PER_DEGREE.powi(2) * lat.to_radians().cos();
            let cell_area_m2 = CELL_AREA_DEG2 * deg2_to_m2;
            let k = self.cs * cell_area_m2 * strain;
            let dt_seconds = dt_days * 86400.0;
            let sigma = (2.0 * k * dt_seconds).sqrt();

            let dx_meters = self.normal.sample(&mut self.rng) * sigma;
            let dy_meters = self.normal.sample(&mut self.rng) * sigma;

            let meters_per_degree_lon = METERS_PER_DEGREE * lat.to_radians().cos();

            final_positions.push((
                lon + dx_meters / meters_per_degree_lon,
                lat + dy_meters / METERS_PER_DEGREE,
            ))
        }

        final_positions
    }
}