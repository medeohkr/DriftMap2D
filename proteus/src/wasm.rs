// wasm.rs
use wasm_bindgen::prelude::*;
use chrono::{NaiveDate, Days, Datelike};
use crate::simulation::{Simulation, SimulationConfig};
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
        release_duration: f32,
        oil_type: String) -> Self {

        let start_date = NaiveDate::from_ymd_opt(start_year, start_month, start_day).unwrap();
        let release_type =
            if release_duration == 0.0 { Schedule::Instant }
            else { Schedule::Continuous{total_days: release_duration} };
        let oil_type = match oil_type.as_str() {
            "marine-diesel" => crate::oil_library::OilType::MarineDiesel,
            "bonny-light" => crate::oil_library::OilType::BonnyLight,
            "arabian-light" => crate::oil_library::OilType::ArabianLight,
            "venezuelan-heavy" => crate::oil_library::OilType::VenezuelanHeavy,
            "ifo-380" => crate::oil_library::OilType::IFO380,
            _ => crate::oil_library::OilType::MarineDiesel,
        };
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
            max_particles: 50000,
            cs: cs_value,
            oil_type: oil_type
        };
        
        let simulation = Simulation::new(sim_config);
        let loader = DataLoader::new(
            "https://tiles.driftmap2d.com/tiles",
            -180.0, -80.0
        );
        let landmask = LandMaskLoader::new(
            "https://tiles.driftmap2d.com/roaring_landmask", // Local path for landmask tiles
            -180.0, -90.0, 90.0
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

    pub async fn init_landmask(&mut self, lon: f32, lat: f32) -> Result<(), JsValue> {
        let lon_idx = ((lon + 180.0) / 10.0).floor() as usize;
        let lat_idx = ((lat + 90.0) / 10.0).floor() as usize;

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
        
        // ========== OCEAN TILES ==========
        let needed_ocean_tiles = self.loader.update_tiles(&self.simulation.get_particles());
        
        if let Err(e) = self.loader.load_by_date(current_date_int, &needed_ocean_tiles).await {
            web_sys::console::error_1(&format!("Failed to load ocean tiles: {:?}", e).into());
            return Err(JsValue::from_str(&format!("{:?}", e)));
        }
        
        // ========== LANDMASK TILES (calculated separately with min_lat = -90°) ==========
        let particles = self.simulation.get_particles();
        let mut landmask_tiles = std::collections::HashSet::new();
        
        for i in 0..particles.len {
            if particles.active[i] && !particles.stranded[i] {
                // Calculate tile indices using landmask coordinate system (min_lat = -90°)
                let lon_idx = ((particles.x[i] + 180.0) / 10.0).floor() as i32;
                let lat_idx = ((particles.y[i] + 90.0) / 10.0).floor() as i32;  // +90 for landmask
                
                // Add surrounding tiles (buffer of 1)
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        let lx = lon_idx + dx;
                        let ly = lat_idx + dy;
                        // 36 longitude tiles (360°/10°), 18 latitude tiles (180°/10°)
                        if lx >= 0 && lx < 36 && ly >= 0 && ly < 18 {
                            landmask_tiles.insert((lx as usize, ly as usize));
                        }
                    }
                }
            }
        }
        
        // Load landmask tiles
        for (lon_idx, lat_idx) in landmask_tiles {
            if let Err(e) = self.landmask.load_tile(lon_idx, lat_idx).await {
                // Log warning but don't fail simulation
                web_sys::console::warn_1(&format!("Landmask tile load failed: {}_{}: {}", lon_idx, lat_idx, e).into());
            }
        }
        
        // ========== RUN SIMULATION STEP ==========
        self.simulation.update_particles_batch(dt_days, &self.loader, self.hour_count, &self.landmask);
        
        // ========== UPDATE TIME ==========
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

    // pub fn get_active_positions(&self) -> Vec<f32> {
    //     let particles = self.simulation.get_particles();
    //     let mut positions = Vec::with_capacity(particles.len);
    //     for i in 0..particles.len {
    //         if particles.active[i] {
    //             positions.push(particles.x[i]);
    //             positions.push(particles.y[i]);
    //         }
    //     }
    //     positions
    // }

    // pub fn get_inactive_positions(&self) -> Vec<f32> {
    //     let particles = self.simulation.get_particles();
    //     let mut positions = Vec::with_capacity(particles.len);
    //     for i in 0..particles.len {
    //         if !particles.active[i] {
    //             positions.push(particles.x[i]);
    //             positions.push(particles.y[i]);
    //         }
    //     }
    //     positions
    // }

    pub fn get_stranded_positions(&self) -> Vec<f32> {
        let particles = self.simulation.get_particles();
        let mut positions = Vec::with_capacity(particles.len);
        for i in 0..particles.len {
            if particles.stranded[i] {
                positions.push(particles.x[i]);
                positions.push(particles.y[i]);
            }
        }
        positions
    }

    pub fn get_unstranded_positions(&self) -> Vec<f32> {
        let particles = self.simulation.get_particles();
        let mut positions = Vec::with_capacity(particles.len);
        for i in 0..particles.len {
            if !particles.stranded[i] {
                positions.push(particles.x[i]);
                positions.push(particles.y[i]);
            }
        }
        positions
    }
    

    pub fn stranded_particle_count(&self) -> usize {
        self.simulation.get_particles().stranded_count()
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
    pub fn stranded_fraction(&self) -> f32 {
        let particles = self.simulation.get_particles();
        let total: usize = particles.len;
        if total == 0 { return 0.0; }
        let stranded = (0..total).filter(|&i| particles.stranded[i]).count();
        stranded as f32 / total as f32 * 100.0
    }

    pub fn mass_weighted_evaporation(&self) -> f32 {
        let particles = self.simulation.get_particles();
        let mut total_initial = 0.0_f32;
        let mut total_evaporated = 0.0_f32;
        for i in 0..particles.len {
            total_initial += self.simulation.initial_mass_per_particle;
            total_evaporated += self.simulation.initial_mass_per_particle * particles.f_evap[i];
        }
        if total_initial > 0.0 { total_evaporated / total_initial * 100.0 } else { 0.0 }
    }

    pub fn mass_weighted_emulsification(&self) -> f32 {
        let particles = self.simulation.get_particles();
        let mut total_mass = 0.0_f32;
        let mut weighted_y_w = 0.0_f32;
        for i in 0..particles.len {
            if particles.active[i] && !particles.stranded[i] {
                total_mass += particles.mass[i];
                weighted_y_w += particles.y_w[i] * particles.mass[i];
            }
        }
        if total_mass > 0.0 { weighted_y_w / total_mass * 100.0 } else { 0.0 }
    }

    pub fn total_floating_mass_tons(&self) -> f32 {
        let particles = self.simulation.get_particles();
        let mut total = 0.0_f32;
        for i in 0..particles.len {
            if particles.active[i] && !particles.stranded[i] {
                total += particles.mass[i];
            }
        }
        total / 1000.0
    }
    pub fn get_unstranded_positions_with_mass(&self) -> Vec<f32> {
        let particles = self.simulation.get_particles();
        let mut data = Vec::with_capacity(particles.len * 3);
        for i in 0..particles.len {
            if particles.active[i] && !particles.stranded[i] {
                data.push(particles.x[i]);
                data.push(particles.y[i]);
                data.push(particles.mass[i] / 1000.0); // current mass in tons
            }
        }
        data
    }
}