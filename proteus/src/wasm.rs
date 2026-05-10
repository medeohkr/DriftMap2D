// wasm.rs
use wasm_bindgen::prelude::*;
use chrono::{NaiveDate, Days, Datelike};
use crate::simulation::{Simulation, SimulationConfig, Integrator};
use crate::release_manager::{ReleaseConfig, Schedule};
use crate::data_loader::DataLoader;
use crate::landmask_loader::LandMaskLoader;

macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

#[wasm_bindgen]
pub struct Proteus {
    simulation: Simulation,
    loader: DataLoader,
    landmask: LandMaskLoader,
    days_since_start: f32,
    start_date: NaiveDate,
    hour_count: u32
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
        cs_value: f32,
        particle_count: usize,
        spread_km: f32,
        start_year: i32,
        start_month: u32,
        start_day: u32,
        release_amount: f64,
        release_duration: f32) -> Self {

        let start_date = NaiveDate::from_ymd_opt(start_year, start_month, start_day).unwrap();
        let release_type =
            if release_duration == 0.0 { Schedule::Instant }
            else { Schedule::Continuous{total_days: release_duration} };

        let release_config = ReleaseConfig {
            lon: lon,
            lat: lat,
            schedule: release_type,
            total_mass_bq: release_amount,
            particle_count: particle_count,
            spread_km: spread_km,
            depth_m: 0.0,
        };
        
        let sim_config = SimulationConfig {
            release_config,
            integrator: Integrator::RK4,
            max_particles: 50000,
            cs: cs_value
        };
        
        let simulation = Simulation::new(sim_config);
        let loader = DataLoader::new(
            "https://tiles.driftmap2d.com/tiles",
            -180.0, -80.0
        );
        let landmask = LandMaskLoader::new(
            "https://tiles.driftmap2d.com/landmask",
            -180.0, -80.0
        );
        
        Self {
            simulation,
            loader,
            landmask,
            days_since_start: 0.0,
            start_date,
            hour_count: 0
        }
    }
    
    fn get_current_date_int(&self) -> u32 {
        let current_date = self.start_date + Days::new(self.days_since_start as u64);
        let year = current_date.year();
        let month = current_date.month();
        let day = current_date.day();
        (year as u32 * 10000) + (month * 100) + day
    }

    pub async fn init_landmask(&mut self) -> Result<(), JsValue> {
        // Preload landmask tiles for the release location
        let lon_idx = ((self.simulation.particles.x[0] + 180.0) / 10.0).floor() as usize;
        let lat_idx = ((self.simulation.particles.y[0] + 80.0) / 10.0).floor() as usize;
        
        // Load a 3×3 block of landmask tiles around the release point
        for dx in -1i32..=1 {
            for dy in -1i32..=1 {
                let lx = (lon_idx as i32 + dx).max(0).min(35) as usize;
                let ly = (lat_idx as i32 + dy).max(0).min(16) as usize;
                if let Err(e) = self.landmask.load_tile(lx, ly).await {
                    web_sys::console::warn_1(&format!("Landmask tile load failed: {}", e).into());
                }
            }
        }
        Ok(())
    }

    pub async fn step(&mut self, dt_days: f32) -> Result<(), JsValue> {
        let current_date_int = self.get_current_date_int();
        self.loader.set_current_day(current_date_int, self.hour_count);
        
        let needed_tiles = self.loader.update_tiles(&self.simulation.get_particles());
        
        if let Err(e) = self.loader.load_by_date(current_date_int, &needed_tiles).await {
            web_sys::console::error_1(&format!("Failed to load tiles: {:?}", e).into());
            return Err(JsValue::from_str(&format!("{:?}", e)));
        }
        
        // Load landmask tiles for needed region
        for tile in &needed_tiles {
            if let Err(e) = self.landmask.load_tile(tile.lon_idx, tile.lat_idx).await {
                // Silently skip missing landmask tiles — just won't strand there
            }
        }
        
        self.simulation.update_particles_batch(dt_days, &self.loader, self.hour_count, &self.landmask);
        
        self.days_since_start += dt_days;
        let total_hours = self.days_since_start * 24.0;
        self.hour_count = (total_hours.floor() % 24.0) as u32;
        
        Ok(())
    }
    
    pub fn get_positions(&self) -> Vec<f32> {
        let particles = self.simulation.get_particles();
        let mut positions = Vec::with_capacity(particles.len);
        for i in 0..particles.len {
            positions.push(particles.x[i]);
            positions.push(particles.y[i]);
        }
        positions
    }

    pub fn get_active_positions(&self) -> Vec<f32> {
        let particles = self.simulation.get_particles();
        let mut positions = Vec::with_capacity(particles.len);
        for i in 0..particles.len {
            if particles.active[i] {
                positions.push(particles.x[i]);
                positions.push(particles.y[i]);
            }
        }
        positions
    }

    pub fn get_inactive_positions(&self) -> Vec<f32> {
        let particles = self.simulation.get_particles();
        let mut positions = Vec::with_capacity(particles.len);
        for i in 0..particles.len {
            if !particles.active[i] {
                positions.push(particles.x[i]);
                positions.push(particles.y[i]);
            }
        }
        positions
    }
    
    pub fn active_particle_count(&self) -> usize {
        self.simulation.get_particles().active_count()
    }

    pub fn inactive_particle_count(&self) -> usize {
        self.simulation.get_particles().inactive_count()
    }
    
    pub fn current_day(&self) -> f32 {
        self.days_since_start
    }
    
    pub fn current_date_int(&self) -> u32 {
        let current_date = self.start_date + Days::new(self.days_since_start as u64);
        let year = current_date.year();
        let month = current_date.month();
        let day = current_date.day();
        (year as u32 * 10000) + (month * 100) + day
    }
    
    pub fn current_time_str(&self) -> String {
        let current_date = self.start_date + Days::new(self.days_since_start as u64);
        let year = current_date.year();
        let month = current_date.month();
        let day = current_date.day();
        format!("{:04}-{:02}-{:02} {:02}:00", year, month, day, self.hour_count)
    }

    pub fn get_particle_bounding_box(&self) -> Vec<f32> {
        self.simulation.particles.bounding_box_array()
    }
}