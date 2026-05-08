// release_manager.rs

use rand::prelude::*;
use rand::distributions::Standard;
use rand_distr::{Normal, Distribution};

/// Release schedule type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Schedule {
    Instant,      // All particles released at start time
    Continuous {  // Released over a duration
        total_days: f32
    },
}

/// Release configuration
#[derive(Debug, Clone)]
pub struct ReleaseConfig {
    pub lon: f32,                    // Release longitude (degrees)
    pub lat: f32,                    // Release latitude (degrees)
    pub schedule: Schedule,          // Instant or continuous
    pub total_mass_bq: f64,          // Total activity (Bq) or mass (tons)
    pub particle_count: usize,       // Number of particles
    pub spread_km: f32,              // Initial spread (standard deviation in km)
    pub depth_m: f32,                // Initial depth (meters)
}

/// Manages particle release over time
pub struct ReleaseManager {
    config: ReleaseConfig,
    total_released: usize,
    particles_per_unit: f64,         // Mass per particle (total / count)
    rng: ThreadRng,
    normal: Normal<f32>,             // For Gaussian spread
}

impl ReleaseManager {
    pub fn new(config: ReleaseConfig) -> Self {
        let particles_per_unit = config.total_mass_bq / config.particle_count as f64;
        let normal = Normal::new(0.0, config.spread_km).expect("Invalid normal distribution");
        
        Self {
            config,
            total_released: 0,
            particles_per_unit,
            rng: thread_rng(),
            normal,
        }
    }
    
    /// Check if any particles should be released at this timestep
    /// Returns number of particles to release and their starting positions
    pub fn update(&mut self, dt_days: f32) -> Option<Vec<ParticleSeed>> {
        match self.config.schedule {
            Schedule::Instant => {
                if self.total_released == 0 {
                    self.total_released = self.config.particle_count;
                    Some(self.generate_particles(self.config.particle_count))
                } else {
                    None
                }
            }
            Schedule::Continuous { total_days } => {
                let rate = self.config.particle_count as f32 / total_days;
                let to_release = (rate * dt_days).floor() as usize;
                
                if to_release > 0 {
                    let remaining = self.config.particle_count - self.total_released;
                    let to_release = to_release.min(remaining);
                    self.total_released += to_release;
                    Some(self.generate_particles(to_release))
                } else {
                    None
                }
            }
        }
    }
    
    /// Generate particle seeds with Gaussian spread around release point
    fn generate_particles(&mut self, count: usize) -> Vec<ParticleSeed> {
        let km_to_deg = 1.0/111.12;
        
        (0..count)
            .map(|_| {
                let dx = self.normal.sample(&mut self.rng);
                let dy = self.normal.sample(&mut self.rng);
                let lon = self.config.lon + dx * km_to_deg;
                let lat = self.config.lat + dy * km_to_deg;
                
                ParticleSeed {
                    lon,
                    lat,
                    depth: self.config.depth_m,
                    mass: self.particles_per_unit,
                }
            })
            .collect()
    }
    
    /// Get total mass/activity released so far
    pub fn total_mass_released(&self) -> f64 {
        self.total_released as f64 * self.particles_per_unit
    }
    
    /// Get fraction of particles released (0.0 to 1.0)
    pub fn fraction_released(&self) -> f32 {
        self.total_released as f32 / self.config.particle_count as f32
    }
}

/// A single particle to be added to the simulation
#[derive(Debug, Clone)]
pub struct ParticleSeed {
    pub lon: f32,
    pub lat: f32,
    pub depth: f32,
    pub mass: f64,
}