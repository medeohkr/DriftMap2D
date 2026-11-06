use super::{normalize_lon, ParticleView};
use crate::forcing::Forcing;
use rand::prelude::*;
use rand_distr::{Distribution, Normal};

const METERS_PER_DEGREE: f32 = 111_120.0;
pub enum DiffusionScheme {
    Constant(f32),
    Smagorinsky(f32)
}
pub struct Diffusion {
    diffusion_scheme: DiffusionScheme,
    normal: Normal<f32>,
    rng: ThreadRng,
}

impl Diffusion {
    pub fn new(scheme: &str, diffusion_coeffs: Vec<f32>) -> Self {
        let normal = Normal::new(0.0, 1.0).unwrap();
        let rng = rand::thread_rng();
        let diffusion_scheme = match scheme {
            "constant" => DiffusionScheme::Constant(diffusion_coeffs[0]),
            "smagorinsky" => DiffusionScheme::Smagorinsky(diffusion_coeffs[1]),
            _ => DiffusionScheme::Constant(diffusion_coeffs[0]),
        };

        Self { 
            diffusion_scheme,
            normal,
            rng
        }
    }

    pub fn diffusion_step(
        &mut self,
        forcing: &impl Forcing,
        old_view: &ParticleView,
        positions: Vec<(f32, f32)>,
        day: usize,
        dt_days: f32,
        hour: f32,
    ) -> Vec<(f32, f32)> {
        if matches!(self.diffusion_scheme, DiffusionScheme::Constant(0.0) | DiffusionScheme::Smagorinsky(0.0)) {
            return positions;
        }
        match self.diffusion_scheme {
            DiffusionScheme::Constant(k) => self.constant_diffusion_step(&positions, dt_days, k),
            DiffusionScheme::Smagorinsky(cs) => self.smagorinsky_step(forcing, old_view, &positions, day, dt_days, hour, cs)
        }
    }
    pub fn constant_diffusion_step(
        &mut self,
        positions: &[(f32, f32)],
        dt_days: f32,
        k: f32
    ) -> Vec<(f32, f32)> {
        let dt_seconds = dt_days * 86400.0;
        let sigma = (2.0 * k * dt_seconds).sqrt();
        let mut final_positions = Vec::with_capacity(positions.len());
            
        for (_, &(lon, lat)) in positions.iter().enumerate() {
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

    pub fn smagorinsky_step(
        &mut self,
        forcing: &impl Forcing,
        old_view: &ParticleView,
        positions: &[(f32, f32)],
        day: usize,
        dt_days: f32,
        hour: f32,
        cs: f32
    ) -> Vec<(f32, f32)> {
        let dx = 0.01;
        let dy = 0.01;

        let n = positions.len();
        let mut probes = Vec::with_capacity(n);
        let mut final_positions = Vec::with_capacity(n);

        for (i, &(lon, lat)) in positions.iter().enumerate() {
            let mid_lon = (old_view.lon(i) + lon) / 2.0;
            let mid_lat = (old_view.lat(i) + lat) / 2.0;

            probes.push((normalize_lon(mid_lon - dx), mid_lat));
            probes.push((normalize_lon(mid_lon + dx), mid_lat));
            probes.push((mid_lon, (mid_lat - dy).clamp(-80.0, 90.0)));
            probes.push((mid_lon, (mid_lat + dy).clamp(-80.0, 90.0)));
        }

        let velocities = forcing.get_velocities(&probes, day, hour);

        for (i, &(lon, lat)) in positions.iter().enumerate() {
            let base = i * 4;

            let vel_lon_minus = velocities[base];
            let vel_lon_plus = velocities[base + 1];
            let vel_lat_minus = velocities[base + 2];
            let vel_lat_plus = velocities[base + 3];

            let dudx = (vel_lon_plus.0 - vel_lon_minus.0) / (2.0 * dx);
            let dudy = (vel_lat_plus.0 - vel_lat_minus.0) / (2.0 * dy);
            let dvdx = (vel_lon_plus.1 - vel_lon_minus.1) / (2.0 * dx);
            let dvdy = (vel_lat_plus.1 - vel_lat_minus.1) / (2.0 * dy);

            let strain = (dudx.powi(2) + 0.5 * (dudy + dvdx).powi(2) + dvdy.powi(2)).sqrt();

            let cell_area_m2 = forcing.cell_area_m2(lat);
            let k = cs * cell_area_m2 * strain;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{basemodel::Particles, forcing::{ForcingFields, LoaderError}, tracers::{GenericTracer, TracerKind}};

    struct UnusedForcing;

    impl Forcing for UnusedForcing {
        async fn prepare_forcing(&mut self, _: Vec<f32>, _: usize, _: f32, _: f32) -> Result<(), LoaderError> {
            panic!("zero diffusion must not prepare forcing")
        }
        fn validate_forcing_interval(&self, _: usize, _: f32, _: f32) -> Result<(), LoaderError> {
            panic!("zero diffusion must not validate forcing")
        }
        fn sample_fields(&self, _: impl ExactSizeIterator<Item = (f32, f32)>, _: usize, _: f32, _: ForcingFields) -> Vec<[f32; 5]> {
            panic!("zero diffusion must not sample forcing")
        }
        fn cell_area_m2(&self, _: f32) -> f32 {
            panic!("zero diffusion must not query cell area")
        }
    }

    #[test]
    fn zero_diffusion_reuses_the_advection_buffer_without_forcing_queries() {
        let tracer = GenericTracer::new(r#"{"wind_factor":0,"wind_deflection":0}"#, 1.0);
        let particles = Particles::new(0, TracerKind::Generic(tracer));
        for scheme in ["constant", "smagorinsky"] {
            let mut diffusion = Diffusion::new(scheme, vec![0.0, 0.0]);
            let positions = vec![(3.0, 59.0), (4.0, 58.0)];
            let original_pointer = positions.as_ptr();
            let result = diffusion.diffusion_step(&UnusedForcing, &particles.view(), positions, 20261105, 1.0 / 96.0, 12.0);
            assert_eq!(result, [(3.0, 59.0), (4.0, 58.0)]);
            assert_eq!(result.as_ptr(), original_pointer);
        }
    }

    #[test]
    fn positive_constant_diffusion_still_moves_particles() {
        let tracer = GenericTracer::new(r#"{"wind_factor":0,"wind_deflection":0}"#, 1.0);
        let particles = Particles::new(0, TracerKind::Generic(tracer));
        let mut diffusion = Diffusion::new("constant", vec![10.0, 0.0]);
        let result = diffusion.diffusion_step(&UnusedForcing, &particles.view(), vec![(0.0, 0.0); 100], 20261105, 1.0 / 96.0, 12.0);
        assert!(result.iter().any(|&(lon, lat)| lon != 0.0 || lat != 0.0));
    }
}
