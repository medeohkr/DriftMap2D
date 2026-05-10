// simulation.rs
use crate::release_manager::{ReleaseManager, ReleaseConfig, Schedule};
use crate::particles::Particles;
use crate::integrators;
use crate::diffusion::Diffusion;
use crate::data_loader::DataLoader;
use crate::landmask_loader::LandMaskLoader;

macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

pub struct Simulation {
    config: SimulationConfig,
    pub particles: Particles,
    release_manager: ReleaseManager,
    diffusion: Diffusion,
}

pub struct SimulationConfig {
    pub release_config: ReleaseConfig,
    pub integrator: Integrator,
    pub max_particles: usize,
    pub cs: f32,
}

pub enum Integrator {
    Euler,
    Midpoint,
    RK4,
}

impl Simulation {
    pub fn new(config: SimulationConfig) -> Self {
        let release_manager = ReleaseManager::new(config.release_config.clone());
        let particles = Particles::new(config.max_particles);
        let diffusion = Diffusion::new(config.cs);
        
        Self {
            config,
            particles,
            release_manager,
            diffusion,
        }
    }
    
    pub fn update_particles_batch(
        &mut self,
        dt_days: f32,
        loader: &DataLoader,
        hour: u32,
        landmask: &LandMaskLoader,
    ) {
        let dt: f32 = dt_days * 86400.0;
        
        // Release new particles if any
        if let Some(seeds) = self.release_manager.update(dt_days) {
            for seed in seeds {
                self.particles.add_particle(
                    seed.lon,
                    seed.lat,
                    seed.depth,
                    0.0,
                    seed.mass as f32,
                    0.0,
                    true,
                );
            }
        }
        
        // Collect active particle indices and positions
        let active_data: Vec<(usize, f32, f32, f32)> = (0..self.particles.len)
            .filter(|&i| self.particles.active[i])
            .map(|i| (i, self.particles.x[i], self.particles.y[i], self.particles.depth[i]))
            .collect();
        
        if active_data.is_empty() {
            return;
        }
        
        // Extract positions for batch velocity lookup
        let positions: Vec<(f32, f32, f32)> = active_data.iter()
            .map(|&(_, lon, lat, depth)| (lon, lat, depth))
            .collect();
        
        // Batch integration
        let new_positions = match self.config.integrator {
            Integrator::Euler => {
                let velocities = loader.get_velocities_batch_grouped(&positions, loader.current_day, hour);
                positions.iter()
                    .enumerate()
                    .map(|(i, &(lon, lat, _))| {
                        let (u, v) = velocities[i];
                        (lon + dt * u, lat + dt * v)
                    })
                    .collect()
            }
            Integrator::Midpoint => {
                let get_velocities = |pos: &[(f32, f32, f32)]| {
                    loader.get_velocities_batch_grouped(pos, loader.current_day, hour)
                };
                integrators::midpoint_step_batch(&positions, dt, get_velocities)
            }
            Integrator::RK4 => {
                let get_velocities = |pos: &[(f32, f32, f32)]| {
                    loader.get_velocities_batch_grouped(pos, loader.current_day, hour)
                };
                integrators::rk4_step_batch(&positions, dt, get_velocities)
            }
        };
        
        // Apply new positions, diffusion, and landmask check
        for (i, &(idx, lon, lat, depth)) in active_data.iter().enumerate() {
            let (new_lon, new_lat) = new_positions[i];
            let (dx, dy) = self.diffusion.smagorinsky_step(
                loader, lon, lat, depth, loader.current_day, dt_days, hour
            );
            
            let final_lon = new_lon + dx;
            let final_lat = new_lat + dy;
            
            // Check landmask — strand if on land
            if landmask.is_on_land(final_lon, final_lat) {
                self.particles.active[idx] = false;
            }
            
            self.particles.x[idx] = final_lon;
            self.particles.y[idx] = final_lat;
            self.particles.age[idx] += dt_days;
        }
    }
    
    pub fn get_particles(&self) -> &Particles {
        &self.particles
    }
}