use wasm_bindgen::prelude::*;
use chrono::{NaiveDate, Days, Datelike};
use crate::simulation::{Simulation, SimulationConfig, Integrator};
use crate::release_manager::{ReleaseConfig, Schedule};
use crate::glorysloader::GlorysLoader;

macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}
#[wasm_bindgen]
pub struct Proteus {
    simulation: Simulation,
    loader: GlorysLoader,
    days_since_start: f32,      // Days elapsed since start_date
    start_date: NaiveDate,       // Base date for the simulation
}

#[wasm_bindgen]
pub fn setup_panic_hook() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
impl Proteus {
    #[wasm_bindgen(constructor)]
    pub fn new(lon: f32, 
        lat: f32,
        k_value: f32,
        particle_count: usize,
        spread_km: f32,
        start_year: i32,
        start_month: u32,
        start_day: u32) -> Self {

        let start_date = NaiveDate::from_ymd_opt(start_year, start_month, start_day).unwrap();
        // Configure release (default: Fukushima)
        let release_config = ReleaseConfig {
            lon: lon,
            lat: lat,
            schedule: Schedule::Instant,
            total_mass_bq: 16.0e15,
            particle_count: particle_count,
            spread_km: spread_km,
            depth_m: 0.0,
        };
        
        let sim_config = SimulationConfig {
            release_config,
            integrator: Integrator::RK4,
            max_particles: 50000,
            k_value: k_value
        };
        
        let simulation = Simulation::new(sim_config);
        
        // Create loader for your tile server
        let loader = GlorysLoader::new(
            "data/forecast_tiles",
            -180.0, -80.0
        );
        
        Self {
            simulation,
            loader,
            days_since_start: 0.0,
            start_date,
        }
    }
    
    /// Convert days since start to YYYYMMDD integer for tile lookup
    fn get_current_date_int(&self) -> u32 {
        let current_date = self.start_date + Days::new(self.days_since_start as u64);
        let year = current_date.year();
        let month = current_date.month();
        let day = current_date.day();
        (year as u32 * 10000) + (month * 100) + day
    }
    
    /// Advance simulation by dt_days and update all particles
    pub async fn step(&mut self, dt_days: f32) -> Result<(), JsValue> {
        // Advance time
        self.days_since_start += dt_days;
        
        // Get current date for tile lookup
        let current_date_int = self.get_current_date_int();
        self.loader.set_current_day(current_date_int);
        
        // Get needed tiles based on particle positions
        let needed_tiles = self.loader.update_tiles(&self.simulation.get_particles());
        
        // Load required tiles
        if let Err(e) = self.loader.load_by_date(current_date_int, &needed_tiles).await {
            web_sys::console::error_1(&format!("Failed to load tiles: {:?}", e).into());
            return Err(JsValue::from_str(&format!("{:?}", e)));
        }
        
        // Velocity function that uses the loader
        let velocity_fn = |lon, lat, depth| {
            self.loader.get_velocity(lon, lat, depth, current_date_int)
                .unwrap_or((0.0, 0.0))
        };
        
        // Update all particles
        self.simulation.update_particles(dt_days, self.days_since_start, velocity_fn);
        
        Ok(())
    }
    
    /// Get all active particle positions as flat array [lon0, lat0, lon1, lat1, ...]
    pub fn get_positions(&self) -> Vec<f32> {
        let particles = self.simulation.get_particles();
        let mut positions = Vec::with_capacity(particles.active_count() * 2);
        
        for i in 0..particles.len {
            if particles.active[i] {
                positions.push(particles.x[i]);
                positions.push(particles.y[i]);
            }
        }
        
        positions
    }
    
    /// Get particle concentrations
    pub fn get_concentrations(&self) -> Vec<f32> {
        let particles = self.simulation.get_particles();
        let mut concentrations = Vec::with_capacity(particles.active_count());
        
        for i in 0..particles.len {
            if particles.active[i] {
                concentrations.push(particles.concentration[i]);
            }
        }
        
        concentrations
    }
    
    /// Get number of active particles
    pub fn active_particle_count(&self) -> usize {
        self.simulation.get_particles().active_count()
    }
    
    /// Get current simulation day (days since start)
    pub fn current_day(&self) -> f32 {
        self.days_since_start
    }
    
    /// Set release location (call before starting simulation)
    pub fn set_release_location(&mut self, lon: f32, lat: f32) {
        // This will require recreating the simulation or updating release config
        web_sys::console::log_1(&format!("Setting release location to ({}, {})", lon, lat).into());
        // TODO: Update release manager config
    }
    
    /// Set number of particles
    pub fn set_particle_count(&mut self, count: usize) {
        web_sys::console::log_1(&format!("Setting particle count to {}", count).into());
        // TODO: Reinitialize simulation with new count
    }
    
    /// Reset simulation to day 0
    pub fn reset(&mut self) {
        self.days_since_start = 0.0;
        // TODO: Clear particles and reinitialize
        //web_sys::console::log_1("Simulation reset".into());
    }
    pub fn current_date_int(&self) -> u32 {
        let current_date = self.start_date + Days::new(self.days_since_start as u64);
        let year = current_date.year();
        let month = current_date.month();
        let day = current_date.day();
        (year as u32 * 10000) + (month * 100) + day
    }
    pub fn get_particle_bounding_box(&self) -> Vec<f32> {
        self.simulation.particles.bounding_box_array()
    }
}