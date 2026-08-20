use rand::prelude::*;
use rand_distr::{Normal, Distribution};
use crate::basemodel::{DataLoader, LandMaskLoader};
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
        landmask: &LandMaskLoader,
        lons: f32,
        lats: f32,
        new_lon: f32,
        new_lat: f32,
        depth: f32,
        day: u32,
        dt_days: f32,
        hour: u32,
    ) -> (f32, f32) {
        let dx = 0.01;
        let dy = 0.01;

        let probes = [
            (loader.normalize_lon(lon + dx), lat),
            (loader.normalize_lon(lon - dx), lat),
            (lon, (lat + dy).clamp(-80.0, 90.0)),
            (lon, (lat - dy).clamp(-80.0, 90.0)),
        ];

        // if any probe is on land, skip diffusion
        // if probes.iter().any(|&(plon, plat)| landmask.is_on_land(plon, plat)) {
        //     return (0.0, 0.0);
        // }

        let velocities = loader.get_velocities_batch(&probes, depth, day, hour);
        
        // use actual velocities instead
        if velocities.iter().any(|&(u, v)| u == 0.0|| v == 0.0) {
            return (0.0, 0.0);
        }

        let (updx, vpdx) = velocities[0];
        let (umdx, vmdx) = velocities[1];
        let (updy, vpdy) = velocities[2];
        let (umdy, vmdy) = velocities[3];

        let dudx = (updx - umdx) / (2.0 * dx);
        let dudy = (updy - umdy) / (2.0 * dy);
        let dvdx = (vpdx - vmdx) / (2.0 * dx);
        let dvdy = (vpdy - vmdy) / (2.0 * dy);

        let deg2_to_m2 = METERS_PER_DEGREE.powi(2) * lat.to_radians().cos();
        let cell_area_m2 = CELL_AREA_DEG2 * deg2_to_m2;

        let strain = (dudx.powi(2) + 0.5 * (dudy + dvdx).powi(2) + dvdy.powi(2)).sqrt();
        let k = self.cs * cell_area_m2 * strain;
        let dt_seconds = dt_days * 86400.0;
        let sigma = (2.0 * k * dt_seconds).sqrt();

        let dx_meters = self.normal.sample(&mut self.rng) * sigma;
        let dy_meters = self.normal.sample(&mut self.rng) * sigma;

        let meters_per_degree_lon = METERS_PER_DEGREE * lat.to_radians().cos();
        (dx_meters / meters_per_degree_lon, dy_meters / METERS_PER_DEGREE)
    }
}