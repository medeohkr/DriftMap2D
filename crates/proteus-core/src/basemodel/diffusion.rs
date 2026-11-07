// diffusion.rs applies stochastic displacements with reusable probe storage

use super::{normalize_lon, ParticleView};
use crate::forcing::{Forcing, ForcingFields};
use rand::prelude::*;
use rand_distr::{Distribution, Normal};

const METERS_PER_DEGREE: f32 = 111_120.0;
pub enum DiffusionScheme {
    Constant(f32),
    Smagorinsky(f32),
}
pub struct Diffusion {
    diffusion_scheme: DiffusionScheme,
    normal: Normal<f32>,
    rng: ThreadRng,
    probes: Vec<(f32, f32)>,
    samples: Vec<[f32; 5]>,
}

impl Diffusion {
    pub fn new(scheme: &str, diffusion_coeffs: Vec<f32>) -> Self {
        let diffusion_scheme = match scheme {
            "smagorinsky" => DiffusionScheme::Smagorinsky(diffusion_coeffs[1]),
            _ => DiffusionScheme::Constant(diffusion_coeffs[0]),
        };
        Self {
            diffusion_scheme,
            normal: Normal::new(0.0, 1.0).unwrap(),
            rng: rand::thread_rng(),
            probes: Vec::new(),
            samples: Vec::new(),
        }
    }

    pub fn diffusion_step(
        &mut self,
        forcing: &impl Forcing,
        old_view: &ParticleView,
        mut positions: Vec<(f32, f32)>,
        day: usize,
        dt_days: f32,
        hour: f32,
    ) -> Vec<(f32, f32)> {
        self.diffusion_step_in_place(forcing, old_view, &mut positions, day, dt_days, hour);
        positions
    }

    pub fn diffusion_step_in_place(
        &mut self,
        forcing: &impl Forcing,
        old_view: &ParticleView,
        positions: &mut [(f32, f32)],
        day: usize,
        dt_days: f32,
        hour: f32,
    ) {
        match self.diffusion_scheme {
            DiffusionScheme::Constant(0.0) | DiffusionScheme::Smagorinsky(0.0) => {}
            DiffusionScheme::Constant(k) => self.constant_in_place(positions, dt_days, k),
            DiffusionScheme::Smagorinsky(cs) => {
                self.smagorinsky_in_place(forcing, old_view, positions, day, dt_days, hour, cs)
            }
        }
    }

    pub fn constant_diffusion_step(
        &mut self,
        positions: &[(f32, f32)],
        dt_days: f32,
        k: f32,
    ) -> Vec<(f32, f32)> {
        let mut result = positions.to_vec();
        self.constant_in_place(&mut result, dt_days, k);
        result
    }

    fn constant_in_place(&mut self, positions: &mut [(f32, f32)], dt_days: f32, k: f32) {
        let sigma = (2.0 * k * (dt_days * 86400.0)).sqrt();
        for (lon, lat) in positions {
            let dx = self.normal.sample(&mut self.rng) * sigma;
            let dy = self.normal.sample(&mut self.rng) * sigma;
            let scale = METERS_PER_DEGREE * lat.to_radians().cos();
            *lon += dx / scale;
            *lat += dy / METERS_PER_DEGREE;
        }
    }

    pub fn smagorinsky_step(
        &mut self,
        forcing: &impl Forcing,
        old_view: &ParticleView,
        positions: &[(f32, f32)],
        day: usize,
        dt_days: f32,
        hour: f32,
        cs: f32,
    ) -> Vec<(f32, f32)> {
        let mut result = positions.to_vec();
        self.smagorinsky_in_place(forcing, old_view, &mut result, day, dt_days, hour, cs);
        result
    }

    fn smagorinsky_in_place(
        &mut self,
        forcing: &impl Forcing,
        old_view: &ParticleView,
        positions: &mut [(f32, f32)],
        day: usize,
        dt_days: f32,
        hour: f32,
        cs: f32,
    ) {
        let dx = 0.01;
        let dy = 0.01;
        self.probes.clear();
        for (i, &(lon, lat)) in positions.iter().enumerate() {
            let mid_lon = (old_view.lon(i) + lon) / 2.0;
            let mid_lat = (old_view.lat(i) + lat) / 2.0;
            self.probes.extend_from_slice(&[
                (normalize_lon(mid_lon - dx), mid_lat),
                (normalize_lon(mid_lon + dx), mid_lat),
                (mid_lon, (mid_lat - dy).clamp(-80.0, 90.0)),
                (mid_lon, (mid_lat + dy).clamp(-80.0, 90.0)),
            ]);
        }
        forcing.sample_fields_into(
            self.probes.iter().copied(),
            day,
            hour,
            ForcingFields::Currents,
            &mut self.samples,
        );
        for (i, (lon, lat)) in positions.iter_mut().enumerate() {
            let b = i * 4;
            let dudx = (self.samples[b + 1][0] - self.samples[b][0]) / (2.0 * dx);
            let dudy = (self.samples[b + 3][0] - self.samples[b + 2][0]) / (2.0 * dy);
            let dvdx = (self.samples[b + 1][1] - self.samples[b][1]) / (2.0 * dx);
            let dvdy = (self.samples[b + 3][1] - self.samples[b + 2][1]) / (2.0 * dy);
            let strain = (dudx.powi(2) + 0.5 * (dudy + dvdx).powi(2) + dvdy.powi(2)).sqrt();
            let k = cs * forcing.cell_area_m2(*lat) * strain;
            let sigma = (2.0 * k * (dt_days * 86400.0)).sqrt();
            let dx = self.normal.sample(&mut self.rng) * sigma;
            let dy = self.normal.sample(&mut self.rng) * sigma;
            let scale = METERS_PER_DEGREE * lat.to_radians().cos();
            *lon += dx / scale;
            *lat += dy / METERS_PER_DEGREE;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        basemodel::Particles,
        forcing::{ForcingFields, LoaderError},
        tracers::{GenericTracer, TracerKind},
    };

    struct UnusedForcing;

    impl Forcing for UnusedForcing {
        async fn prepare_forcing(
            &mut self,
            _: Vec<f32>,
            _: usize,
            _: f32,
            _: f32,
        ) -> Result<(), LoaderError> {
            panic!("zero diffusion must not prepare forcing")
        }
        fn validate_forcing_interval(&self, _: usize, _: f32, _: f32) -> Result<(), LoaderError> {
            panic!("zero diffusion must not validate forcing")
        }
        fn sample_fields(
            &self,
            _: impl ExactSizeIterator<Item = (f32, f32)>,
            _: usize,
            _: f32,
            _: ForcingFields,
        ) -> Vec<[f32; 5]> {
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
            let result = diffusion.diffusion_step(
                &UnusedForcing,
                &particles.view(),
                positions,
                20261105,
                1.0 / 96.0,
                12.0,
            );
            assert_eq!(result, [(3.0, 59.0), (4.0, 58.0)]);
            assert_eq!(result.as_ptr(), original_pointer);
        }
    }

    #[test]
    fn positive_constant_diffusion_still_moves_particles() {
        let tracer = GenericTracer::new(r#"{"wind_factor":0,"wind_deflection":0}"#, 1.0);
        let particles = Particles::new(0, TracerKind::Generic(tracer));
        let mut diffusion = Diffusion::new("constant", vec![10.0, 0.0]);
        let positions = vec![(0.0, 0.0); 100];
        let pointer = positions.as_ptr();
        let result = diffusion.diffusion_step(
            &UnusedForcing,
            &particles.view(),
            positions,
            20261105,
            1.0 / 96.0,
            12.0,
        );
        assert_eq!(result.as_ptr(), pointer);
        assert!(result.iter().any(|&(lon, lat)| lon != 0.0 || lat != 0.0));
    }

    #[test]
    fn smagorinsky_reuses_four_probes_per_particle_and_leaves_uniform_flows_unchanged() {
        struct Uniform;
        impl Forcing for Uniform {
            async fn prepare_forcing(
                &mut self,
                _: Vec<f32>,
                _: usize,
                _: f32,
                _: f32,
            ) -> Result<(), LoaderError> {
                Ok(())
            }
            fn validate_forcing_interval(
                &self,
                _: usize,
                _: f32,
                _: f32,
            ) -> Result<(), LoaderError> {
                Ok(())
            }
            fn cell_area_m2(&self, _: f32) -> f32 {
                640000.0
            }
            fn sample_fields(
                &self,
                positions: impl ExactSizeIterator<Item = (f32, f32)>,
                _: usize,
                _: f32,
                fields: ForcingFields,
            ) -> Vec<[f32; 5]> {
                assert_eq!(fields, ForcingFields::Currents);
                vec![[0.0; 5]; positions.len()]
            }
        }
        let tracer = GenericTracer::new(r#"{"wind_factor":0,"wind_deflection":0}"#, 1.0);
        let mut particles = Particles::new(2, TracerKind::Generic(tracer));
        particles.add_particle(3.0, 59.0);
        particles.add_particle(4.0, 60.0);
        let view = particles.view();
        let mut diffusion = Diffusion::new("smagorinsky", vec![0.0, 0.1]);
        let mut positions = vec![(3.01, 59.01), (4.01, 60.01)];
        let original = positions.clone();
        diffusion.diffusion_step_in_place(
            &Uniform,
            &view,
            &mut positions,
            20261105,
            1.0 / 96.0,
            12.0,
        );
        assert_eq!(positions, original);
        assert_eq!(diffusion.probes.len(), 8);
        let pointers = (
            diffusion.probes.as_ptr(),
            diffusion.samples.as_ptr(),
            positions.as_ptr(),
        );
        diffusion.diffusion_step_in_place(
            &Uniform,
            &view,
            &mut positions,
            20261105,
            1.0 / 96.0,
            12.25,
        );
        assert_eq!(
            pointers,
            (
                diffusion.probes.as_ptr(),
                diffusion.samples.as_ptr(),
                positions.as_ptr()
            )
        );
        assert_eq!(positions, original);
    }
}
