// simulation.rs
use crate::basemodel::{
    ReleaseManager,
    ReleaseConfig,
    Particles,
    integrators,
    Diffusion,
    DataLoader,
    LandMaskLoader};
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
    tracer: TracerKind,
    pub initial_mass_per_particle: f32,
}

pub struct SimulationConfig {
    pub release_config: ReleaseConfig,
    pub max_particles: usize,
    pub cs: f32,
}

impl Simulation {
    pub fn new(config: SimulationConfig, tracer: TracerKind) -> Self {
        let release_config = config.release_config.clone();
        let max_particles = config.max_particles;
        let cs = config.cs;

        let release_manager = ReleaseManager::new(release_config.clone());
        let particles = Particles::new(max_particles);
        let diffusion = Diffusion::new(cs);
        let initial_mass_per_particle =
            release_config.total_mass_bq as f32 * 1000.0 / release_config.particle_count as f32;

        Self {
            particles,
            release_manager,
            diffusion,
            tracer,
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
                    self.tracer.seed(),
                );
            }
        }
    }

    pub fn update_particles_batch(
        &mut self,
        dt_days: f32,
        loader: &DataLoader,
        hour: u32,
        landmask: &LandMaskLoader,
    ) {
        let dt = dt_days * 86400.0;

        for i in 0..self.particles.len {
            while self.particles.x[i] < -180.0 { self.particles.x[i] += 360.0; }
            while self.particles.x[i] >= 180.0 { self.particles.x[i] -= 360.0; }
        }

        // ---- Collect active, unstranded particles ----
        let unstranded_data: Vec<(usize, f32, f32, f32)> = (0..self.particles.len)
            .filter(|&i| !self.particles.stranded[i])
            .map(|i| (i, self.particles.x[i], self.particles.y[i], self.particles.depth[i]))
            .collect();

        if unstranded_data.is_empty() {
            return;
        }

        let positions: Vec<(f32, f32, f32)> = unstranded_data.iter()
            .map(|&(_, lon, lat, depth)| (lon, lat, depth))
            .collect();

        // ---- Environmental data ----
        let env_data = loader.get_velocities_wind_sst(
            &positions, loader.current_day, hour,
        );

        // ---- Weathering ----
        for (i, &(idx, _, _, _)) in unstranded_data.iter().enumerate() {
            let ((_cu, _cv), (wu_raw, wv_raw), sst) = env_data[i];
            let wind_speed = (wu_raw * wu_raw + wv_raw * wv_raw).sqrt();
            let sst_celsius = sst - 273.15;
            let data = &mut self.particles.data[idx];
            self.tracer.step(data, wind_speed, sst_celsius, dt)
        }

        // ---- Build combined velocity closure ----
        let get_combined_velocities = |pos: &[(f32, f32, f32)]| -> Vec<(f32, f32)> {
            let env = loader.get_velocities_wind_sst(pos, loader.current_day, hour);

            pos.iter()
                .enumerate()
                .map(|(i, &(_, lat, _depth))| {
                    let ((cu, cv), (wu_raw, wv_raw), _sst) = env[i];
                    let w_factor = self.tracer.wind_f();

                    let wind_speed = (wu_raw * wu_raw + wv_raw * wv_raw).sqrt().max(0.1);
                    let theta_deg = self.tracer.wind_deg().unwrap_or_else(|| 25.0 * (-wind_speed.powi(3) / 1184.75).exp());
                    let theta = if lat >= 0.0 {
                        theta_deg.to_radians()
                    } else {
                        -theta_deg.to_radians()
                    };
                    let cos_t = theta.cos();
                    let sin_t = theta.sin();

                    let u_drift = w_factor * (wu_raw * cos_t - wv_raw * sin_t);
                    let v_drift = w_factor * (wu_raw * sin_t + wv_raw * cos_t);

                    let meters_per_degree_lat = 111_120.0;
                    let meters_per_degree_lon = 111_120.0 * lat.to_radians().cos();

                    (
                        cu + u_drift / meters_per_degree_lon,
                        cv + v_drift / meters_per_degree_lat,
                    )
                })
                .collect()
        };

        // ---- Integration ----
        let new_positions = 
        integrators::rk4_step_batch(&positions, dt, &get_combined_velocities);

        // ---- Apply positions, diffusion, and stranding ----
        for (i, &(idx, _, _, depth)) in unstranded_data.iter().enumerate() {
            let (mut new_lon, mut new_lat) = new_positions[i];
            
            // NORMALIZE FIRST - before any data queries
            while new_lon < -180.0 { new_lon += 360.0; }
            while new_lon >= 180.0 { new_lon -= 360.0; }
            new_lat = new_lat.clamp(-80.0, 90.0);


            // Now use the normalized position for diffusion
            let (dx, dy) = self.diffusion.smagorinsky_step(
                loader, landmask, new_lon, new_lat, depth, loader.current_day, dt_days, hour,
            );

            let final_lon = new_lon + dx;
            let final_lat = new_lat + dy;

            // Normalize again after adding diffusion offset
            let mut final_lon = final_lon;
            while final_lon < -180.0 { final_lon += 360.0; }
            while final_lon >= 180.0 { final_lon -= 360.0; }
            let final_lat = final_lat.clamp(-80.0, 90.0);
            
            if landmask.is_on_land(final_lon, final_lat) {
                self.particles.stranded[idx] = true;
            }

            self.particles.x[idx] = final_lon;
            self.particles.y[idx] = final_lat;
        }
    }
    pub fn get_particles(&self) -> &Particles {
        &self.particles
    }
}