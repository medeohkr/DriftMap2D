// simulation.rs
use super::{
    integrators, meters_per_degree_lat, meters_per_degree_lon, normalize_lon, DataLoader,
    Diffusion, LandMaskLoader, ParticleView, Particles, ReleaseManager,
};
use crate::tracers::{GenericTracer, OilTracer, Tracer, TracerKind};

macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

pub struct Simulation {
    pub particles: Particles,
    pub release_manager: ReleaseManager,
    diffusion: Diffusion,
    pub total_particles: usize,
    pub cs: f32,
}

impl Simulation {
    pub fn new(
        tracer_type: &str,
        tracer_json: &str,
        releases_json: &str,
        total_particles: usize,
        cs: f32,
    ) -> Self {

        let release_manager = ReleaseManager::new(releases_json, total_particles);
        let tracer = match tracer_type {
            "generic"=> TracerKind::Generic(GenericTracer::new(
                tracer_json,
                release_manager.initial_mass_per_particle()
            )),

            "oil" => TracerKind::Oil(OilTracer::new(
                tracer_json,
                total_particles,
                release_manager.initial_mass_per_particle()
            )),

            _ => TracerKind::Generic(GenericTracer::new(
                tracer_json,
                release_manager.initial_mass_per_particle()
            )),
        };
        let particles = Particles::new(total_particles, tracer);
        let diffusion = Diffusion::new(cs);

        Self {
            particles,
            release_manager,
            diffusion,
            total_particles,
            cs
        }
    }

    pub fn release_particles(&mut self, days_since_start: f32, dt_days: f32) {
        let seeds = self.release_manager.update(days_since_start * 24.0, dt_days * 24.0);
        for seed in seeds {
            self.particles.add_particle(seed.lon, seed.lat, seed.depth);
        }
    }

    fn calculate_total_velocity(
        &self,
        lat: f32,
        wind_u: f32,
        wind_v: f32,
        current_u: f32,
        current_v: f32,
    ) -> (f32, f32) {
        let u_drift = self.particles.tracer.windage(wind_u, wind_v, lat).0;
        let v_drift = self.particles.tracer.windage(wind_u, wind_v, lat).1;
        (
            current_u + meters_per_degree_lon(u_drift, lat),
            current_v + meters_per_degree_lat(v_drift),
        )
    }

    pub fn update_particles_batch(
        &mut self,
        dt_days: f32,
        loader: &DataLoader,
        hour: usize,
        landmask: &LandMaskLoader,
    ) {
        let dt = dt_days * 86400.0;

        let (indices, (wind_speeds, sst_celsius)): (Vec<usize>, (Vec<f32>, Vec<f32>)) = {
            let temp_view = self.particles.view();
            let wind_sst = loader.get_wind_sst(&temp_view, loader.current_day, hour);

            (
                temp_view.indices,
                wind_sst
                    .iter()
                    .map(|(u_wind_m, v_wind_m, sst_k)| {
                        (
                            (u_wind_m * u_wind_m + v_wind_m * v_wind_m).sqrt(),
                            sst_k - 273.15,
                        )
                    })
                    .unzip(),
            )
        };

        self.particles
            .tracer
            .step(&indices, &wind_speeds, &sst_celsius, dt);

        let unstranded_view = self.particles.view();

        let get_velocities_view = |view: &ParticleView| -> Vec<(f32, f32)> {
            let env = loader.get_velocities_wind(view, loader.current_day, hour);

            env.iter()
                .copied()
                .enumerate()
                .map(|(i, (current_u, current_v, wind_u_m, wind_v_m))| {
                    self.calculate_total_velocity(
                        view.lat(i),
                        current_u,
                        current_v,
                        wind_u_m,
                        wind_v_m,
                    )
                })
                .collect()
        };

        let get_velocities_slice = |slice: &[(f32, f32, f32)]| -> Vec<(f32, f32)> {
            let env = loader.get_velocities_wind_slice(slice, loader.current_day, hour);
            env.iter()
                .copied()
                .enumerate()
                .map(|(i, (current_u, current_v, wind_u_m, wind_v_m))| {
                    self.calculate_total_velocity(
                        slice[i].1, current_u, current_v, wind_u_m, wind_v_m,
                    )
                })
                .collect()
        };
        let advected_positions = integrators::rk4_step(
            &unstranded_view,
            dt,
            &get_velocities_view,
            &get_velocities_slice,
        );
        let final_positions = self.diffusion.smagorinsky_step(
            loader,
            &unstranded_view,
            &advected_positions,
            loader.current_day,
            dt_days,
            hour,
        );

        for (i, &idx) in unstranded_view.indices.iter().enumerate() {
            let (mut lon, mut lat) = final_positions[i];

            lon = normalize_lon(lon);
            lat = lat.clamp(-80.0, 90.0);

            if landmask.is_on_land(lon, lat) {
                self.particles.stranded[idx] = true;
            } else {
                self.particles.lons[idx] = lon;
                self.particles.lats[idx] = lat;
            }
        }
    }
    pub fn get_particles(&self) -> &Particles {
        &self.particles
    }
}
