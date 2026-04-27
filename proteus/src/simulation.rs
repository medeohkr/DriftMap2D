use crate::particles::Particles;
use crate::integrators;

pub struct Simulation {
    config: SimulationConfig,
    pub particles: Particles,           // Made pub for testing
}

pub struct SimulationConfig {
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
        let particles = Particles::new(config.max_particles);
        
        Self {
            config,
            particles,
        }
    }
    
    /// Update all particles using the chosen integrator
    pub fn update_particles(&mut self, dt: f32, current_day: f32, velocity_fn: impl Fn(f32, f32) -> (f32, f32) + Copy) {
        for i in 0..self.particles.len {
            if !self.particles.active[i] {
                continue;
            }
            
            let lon = self.particles.x[i];
            let lat = self.particles.y[i];
            let (u, v) = velocity_fn(lon, lat);
            let (new_x, new_y) = match self.config.integrator {
                Integrator::Euler => {
                    integrators::euler_step(lon, lat, dt, velocity_fn)
                }
                Integrator::Midpoint => {
                    integrators::midpoint_step(lon, lat, dt, velocity_fn)
                }
                Integrator::RK4 => {
                    integrators::rk4_step(lon, lat, dt, velocity_fn)
                }
            };
            
            self.particles.x[i] = new_x;
            self.particles.y[i] = new_y;
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

    pub fn add_particle(&mut self, x: f32, y: f32) -> () {
        self.particles.add_particle(x, y, true);
    }
}