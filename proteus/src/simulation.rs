use crate::release_manager::{ReleaseManager, ReleaseConfig, Schedule};
use crate::particles::Particles;
use crate::integrators;

pub struct Simulation {
    config: SimulationConfig,
    pub particles: Particles,           // Made pub for testing
    release_manager: ReleaseManager,
}

pub struct SimulationConfig {
    pub release_config: ReleaseConfig,
    pub integrator: Integrator,
    pub max_particles: usize,
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
        
        Self {
            config,
            particles,
            release_manager,
        }
    }
    
    /// Update all particles using the chosen integrator
    pub fn update_particles(&mut self, dt_days: f32, current_day: f32, velocity_fn: impl Fn(f32, f32, f32) -> (f32, f32) + Copy) {
        // Release new particles if any
        let dt: f32 = dt_days * 86400.0;
        if let Some(seeds) = self.release_manager.update(current_day, dt) {
            for seed in seeds {
                self.particles.add_particle(
                    seed.lon,
                    seed.lat,
                    seed.depth,
                    0.0,        // initial concentration (will be calculated)
                    seed.mass as f32,
                    0.0,        // initial age
                    true,       // active
                    vec![],     // empty history
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
            let (u, v) = velocity_fn(lon, lat, depth);
            //web_sys::console::log_1(&format!(
            //    "Particle {}: lon={:.2}, lat={:.2}, u={:.4}, v={:.4}, dt={}",
            //    i, lon, lat, u, v, dt
            //).into());
            // Integrate based on chosen method
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
            
            self.particles.x[i] = new_x;
            self.particles.y[i] = new_y;
            self.particles.age[i] += dt;
            
            // Update history for visualization
            self.particles.update_history(i, 50);
        }
    }
    
    /// Get reference to particles (for visualization)
    pub fn get_particles(&self) -> &Particles {
        &self.particles
    }
    
    /// Get mutable reference to particles (for external updates)
    pub fn get_particles_mut(&mut self) -> &mut Particles {
        &mut self.particles
    }
}