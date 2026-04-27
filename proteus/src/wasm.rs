use wasm_bindgen::prelude::*;
use crate::particles::Particles;
use crate::simulation::{Simulation, SimulationConfig, Integrator};
use crate::glorysloader::GlorysLoader;
use std::cell::RefCell;
use std::rc::Rc;

/// Main WASM export for Driftmap
#[wasm_bindgen]
pub struct Driftmap {
    simulation: Simulation,
    loader: GlorysLoader,
    current_day: f32,
}

#[wasm_bindgen]
impl Driftmap {
    /// Create a new simulation instance
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {

        let sim_config = SimulationConfig {
            integrator: Integrator::RK4,
            max_particles: 50000,
        };
        
        let simulation = Simulation::new(sim_config);
        
        // Create loader for Pacific region
        let loader = GlorysLoader::new(
            "glorys_tiles_surface",  // CDN URL (or local path for dev)
            -180.0, 180.0,   // min_lon, max_lon
            -80.0, 90.0,      // min_lat, max_lat
        );
        let start_day = 20110106;
        
        Self {
            simulation,
            loader,
            current_day: start_day as f32,
        }
    }
    
    pub async fn step(&mut self, dt_days: f32) -> Result<(), JsValue> {
        let current_day_int = self.current_day as u32;
        self.loader.set_current_day(current_day_int);
        let needed_tiles = self.loader.update_tiles(&self.simulation.get_particles());
        
        // Test velocity at Fukushima
        let test_vel = self.loader.get_velocity(142.03, 37.42, current_day_int);
        web_sys::console::log_1(&format!("Velocity at Fukushima: {:?}", test_vel).into());
        // Load tiles asynchronously
        if let Err(e) = self.loader.load_by_date(current_day_int, &needed_tiles).await {
            web_sys::console::error_1(&format!("Failed to load tiles: {:?}", e).into());
            return Err(JsValue::from_str(&format!("{:?}", e)));
        }
        
        let velocity_fn = |lon, lat| {
            self.loader.get_velocity(lon, lat, current_day_int)
                .unwrap_or((0.0, 0.0))
        };
        
        self.simulation.update_particles(dt_days, self.current_day, velocity_fn);
        self.current_day += dt_days;
        Ok(())
    }
    
    /// Get all particle positions as a flat array [x0, y0, x1, y1, ...]
    pub fn get_positions(&self) -> Vec<f32> {
        let particles = self.simulation.get_particles();
        let mut positions = Vec::with_capacity(particles.len * 2);
        
        for i in 0..particles.len {
            if particles.active[i] {
                positions.push(particles.x[i]);
                positions.push(particles.y[i]);
            }
        }
        
        positions
    }

    pub fn release_particles(&self) {
        

    }
    
    /// Get number of active particles
    pub fn active_particle_count(&self) -> usize {
        self.simulation.get_particles().active_count()
    }
    
    /// Get current simulation day
    pub fn current_day(&self) -> f32 {
        self.current_day
    }
    
    /// Set release location (for user interaction)
    pub fn set_release_location(&mut self, lon: f32, lat: f32) {
        // This would require regenerating the simulation
        // For now, log it
        web_sys::console::log_1(&format!("Setting release to ({}, {})", lon, lat).into());
    }
    
    /// Set number of particles
    pub fn set_particle_count(&mut self, count: usize) {
        web_sys::console::log_1(&format!("Setting particle count to {}", count).into());
        // TODO: Recreate simulation with new count
    }
}