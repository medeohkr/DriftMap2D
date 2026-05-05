use crate::release_manager::{ReleaseManager, ReleaseConfig, Schedule};
use crate::particles::Particles;
use crate::integrators;
use crate::diffusion::Diffusion;
use crate::glorysloader::GlorysLoader;

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
    pub k_value: f32,
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
        let diffusion = Diffusion::new(config.k_value);
        
        Self {
            config,
            particles,
            release_manager,
            diffusion,
        }
    }
    
    /// Original single-particle update (kept for compatibility)
    pub fn update_particles(
        &mut self, 
        dt_days: f32, 
        current_day: f32, 
        velocity_fn: impl Fn(f32, f32, f32) -> (f32, f32) + Copy
    ) {
        let dt: f32 = dt_days * 86400.0;
        
        // Release new particles if any
        if let Some(seeds) = self.release_manager.update(current_day, dt) {
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

        // Update all active particles
        for i in 0..self.particles.len {
            if !self.particles.active[i] {
                continue;
            }
            
            let lon = self.particles.x[i];
            let lat = self.particles.y[i];
            let depth = self.particles.depth[i];
            
            let (new_x, new_y) = match self.config.integrator {
                Integrator::Euler => {
                    integrators::euler_step(lon, lat, depth, dt, velocity_fn)
                }
                Integrator::Midpoint => {
                    integrators::midpoint_step(lon, lat, depth, dt, velocity_fn)
                }
                Integrator::RK4 => {
                    integrators::rk4_step(lon, lat, depth, dt, velocity_fn)
                }
            };
            
            let (dx, dy) = self.diffusion.apply_diffusion(dt_days, lat);
            
            self.particles.x[i] = new_x + dx;
            self.particles.y[i] = new_y + dy;
            self.particles.age[i] += dt_days;
        }
    }
    
    /// Batch update all particles using optimized grouped velocity lookups
    pub fn update_particles_batch(
        &mut self,
        dt_days: f32,
        current_day: f32,
        loader: &GlorysLoader,
        current_date_int: u32,
    ) {
        let dt: f32 = dt_days * 86400.0;
        
        // Release new particles if any
        if let Some(seeds) = self.release_manager.update(current_day, dt) {
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
        
        // Extract just positions for batch velocity lookup
        let positions: Vec<(f32, f32, f32)> = active_data.iter()
            .map(|&(_, lon, lat, depth)| (lon, lat, depth))
            .collect();
        
        // Check initial velocities for stranding BEFORE integration
        let initial_velocities = loader.get_velocities_batch_grouped(&positions, current_date_int);

        let mut stranded_indices = Vec::new();
        let mut active_integration_data = Vec::new();
        let mut active_velocity_data = Vec::new();
        
        // Separate stranded particles from active ones
        for (i, &(idx, lon, lat, depth)) in active_data.iter().enumerate() {
            let (u, v) = initial_velocities[i];
            if u == 0.0 && v == 0.0 {
                stranded_indices.push(idx);
            } else {
                active_integration_data.push((idx, lon, lat, depth));
                active_velocity_data.push((u, v));
            }
        }
 
        // If no active particles remain, return early
        if active_integration_data.is_empty() {
            return;
        }
        
        // Extract positions for the remaining active particles
        let active_positions: Vec<(f32, f32, f32)> = active_integration_data.iter()
            .map(|&(_, lon, lat, depth)| (lon, lat, depth))
            .collect();
        
        // Batch integration for non-stranded particles
        let new_positions = match self.config.integrator {
            Integrator::Euler => {
                let velocities = loader.get_velocities_batch_grouped(&active_positions, current_date_int);
                active_positions.iter()
                    .enumerate()
                    .map(|(i, &(lon, lat, _))| {
                        let (u, v) = velocities[i];
                        (lon + dt * u, lat + dt * v)
                    })
                    .collect()
            }
            Integrator::Midpoint => {
                let get_velocities = |pos: &[(f32, f32, f32)]| {
                    loader.get_velocities_batch_grouped(pos, current_date_int)
                };
                integrators::midpoint_step_batch(&active_positions, dt, get_velocities)
            }
            Integrator::RK4 => {
                let get_velocities = |pos: &[(f32, f32, f32)]| {
                    loader.get_velocities_batch_grouped(pos, current_date_int)
                };
                integrators::rk4_step_batch(&active_positions, dt, get_velocities)
            }
        };
        
        for &idx in &stranded_indices {
            self.particles.active[idx] = false;
        }
        // Apply new positions and diffusion
        for (i, &(idx, _, lat, _)) in active_integration_data.iter().enumerate() {
            let (new_lon, new_lat) = new_positions[i];
            let (dx, dy) = self.diffusion.apply_diffusion(dt_days, lat);
            
            self.particles.x[idx] = new_lon + dx;
            self.particles.y[idx] = new_lat + dy;
            self.particles.age[idx] += dt_days;


        }
    }
    
    /// Get reference to particles (for visualization)
    pub fn get_particles(&self) -> &Particles {
        &self.particles
    }
}