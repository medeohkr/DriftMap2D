// simulation.rs
use super::{
    DataLoader, Diffusion, LandMaskLoader, ParticleView, Particles, ReleaseConfig, ReleaseManager, integrators, normalize_lon, meters_per_degree_lat, meters_per_degree_lon
};
use crate::tracers::{Tracer, TracerKind};

macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

pub struct Simulation {
    pub particles: Particles,
    release_manager: ReleaseManager,
    diffusion: Diffusion,
    pub initial_mass_per_particle: f32,
}

pub struct SimulationConfig {
    pub release_config: ReleaseConfig,
    pub cs: f32,
}

impl Simulation {
    pub fn new(config: SimulationConfig, tracer: TracerKind) -> Self {
        let release_config = config.release_config.clone();
        let cs = config.cs;

        let release_manager = ReleaseManager::new(release_config.clone());
        let particles = Particles::new(config.release_config.particle_count, tracer);
        let diffusion = Diffusion::new(cs);
        let initial_mass_per_particle =
            release_config.total_mass_bq as f32 * 1000.0 / release_config.particle_count as f32;

        Self {
            particles,
            release_manager,
            diffusion,
            initial_mass_per_particle,
        }
    }

    pub fn release_particles(&mut self, dt_days: f32) {
        if let Some(seeds) = self.release_manager.update(dt_days) {
            for seed in seeds {
                self.particles.add_particle(
                    seed.lon,
                    seed.lat,
                    seed.depth,
                );
            }
        }
    }

    fn calculate_total_velocity(
        &self,
        lat: f32,
        current_u: f32,
        current_v: f32,
        wind_u_m: f32,
        wind_v_m: f32
    ) -> (f32, f32) {
        let w_factor = self.particles.tracer.wind_f();
        let wind_speed = (wind_u_m * wind_u_m + wind_v_m * wind_v_m).sqrt().max(0.1);

        let theta_deg = self.particles.tracer.wind_deg().unwrap_or_else(|| 25.0 * (-wind_speed.powi(3) / 1184.75).exp());
        let theta = if lat >= 0.0 {
            theta_deg.to_radians()
        } else {
            -theta_deg.to_radians()
        };

        let cos_t = theta.cos();
        let sin_t = theta.sin();

        let u_drift = w_factor * (wind_u_m * cos_t - wind_v_m * sin_t);
        let v_drift = w_factor * (wind_u_m * sin_t + wind_v_m * cos_t);
        (current_u + meters_per_degree_lon(u_drift, lat), current_v + meters_per_degree_lat(v_drift))
    }

    pub fn update_particles_batch(
        &mut self,
        dt_days: f32,
        loader: &DataLoader,
        hour: usize,
        landmask: &LandMaskLoader,
    ) {
        let dt = dt_days * 86400.0;

        let (wind_speeds, sst_celsius): (Vec<f32>, Vec<f32>) = {
            let temp_view = self.particles.view();
            let wind_sst = loader.get_wind_sst(&temp_view, loader.current_day, hour);
            wind_sst.iter()
                .map(|(u_wind_m, v_wind_m, sst_k)| {
                    ((u_wind_m * u_wind_m + v_wind_m * v_wind_m).sqrt(), sst_k - 273.15)
                })
                .unzip()
        };

        self.particles.tracer.step(&wind_speeds, &sst_celsius, dt);

        let unstranded_view = self.particles.view();

        let get_velocities_view = |view: &ParticleView| -> Vec<(f32, f32)> {
            let env = loader.get_velocities_wind(view, loader.current_day, hour);

            env.iter().copied().enumerate().map(
                |(i, (current_u, current_v, wind_u_m, wind_v_m))|
                self.calculate_total_velocity(view.lat(i), current_u, current_v, wind_u_m, wind_v_m)
            ).collect()
        };

        let get_velocities_slice = |slice: &[(f32, f32, f32)]| -> Vec<(f32, f32)> {
            let env = loader.get_velocities_wind_slice(slice, loader.current_day, hour);
            env.iter().copied().enumerate().map(
                |(i, (current_u, current_v, wind_u_m, wind_v_m))|
                self.calculate_total_velocity(slice[i].1, current_u, current_v, wind_u_m, wind_v_m)
            ).collect()
        };
        let advected_positions = integrators::rk4_step(&unstranded_view, dt, &get_velocities_view, &get_velocities_slice);
        let final_positions = self.diffusion.smagorinsky_step(loader, &unstranded_view, &advected_positions, loader.current_day, dt_days, hour);
      
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