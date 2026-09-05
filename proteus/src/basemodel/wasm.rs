// wasm.rs
use crate::basemodel::release_manager::{ReleaseConfig, Schedule};
use crate::basemodel::simulation::{Simulation, SimulationConfig};
use crate::basemodel::DataLoader;
use crate::basemodel::LandMaskLoader;
use crate::tracers::{OilTracer, TracerKind};
use chrono::{Datelike, Days, NaiveDateTime};
use wasm_bindgen::prelude::*;

macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into())
    }
}

#[wasm_bindgen]
pub struct Proteus {
    simulation: Simulation,
    loader: DataLoader,
    landmask: LandMaskLoader,
    days_since_start: f32,
    start_date: NaiveDateTime,
    hour_count: u32,
    step_count: u32,
    steps_per_day: u32,
}

#[wasm_bindgen]
pub fn setup_panic_hook() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
impl Proteus {
    #[wasm_bindgen(constructor)]
    pub fn new(
        lon: f32,
        lat: f32,
        cs_value: f32,
        particle_count: usize,
        spread_km: f32,
        start_date_str: &str,
        steps_per_day: u32,
        release_amount: f64,
        release_duration: f32,
        tracer_type: &str,
        tracer_json: &str
    ) -> Self {
        let start_date =
            NaiveDateTime::parse_from_str(start_date_str, "%Y-%m-%d %H:%M").expect("Invalid date format");
        let release_type = if release_duration == 0.0 {
            Schedule::Instant
        } else {
            Schedule::Continuous {
                total_days: release_duration,
            }
        };

        let tracer = match tracer_type {
            "oil" => TracerKind::Oil(OilTracer::new(
                tracer_json,
                particle_count,
                release_amount as f32 / particle_count as f32,
            )),

            _ => TracerKind::Oil(OilTracer::new(
                tracer_json,
                particle_count,
                release_amount as f32 / particle_count as f32,
            )),
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
            cs: cs_value,
        };

        let simulation = Simulation::new(sim_config, tracer);
        let loader = DataLoader::new("https://tiles.driftmap2d.com/tiles", -180.0, -80.0);
        let landmask = LandMaskLoader::new(
            "https://tiles.driftmap2d.com/roaring_landmask",
            -180.0,
            -90.0,
            90.0,
        );

        Self {
            simulation,
            loader,
            landmask,
            days_since_start: 0.0,
            start_date,
            steps_per_day,
            hour_count: 0,
            step_count: 0,
        }
    }

    pub fn get_current_date_int(&self) -> usize {
        let current_date = self.start_date + Days::new(self.days_since_start as u64);
        let year = current_date.year();
        let month = current_date.month();
        let day = current_date.day();
        (year as usize * 10000) + (month as usize * 100) + day as usize
    }

    pub async fn init_landmask(&mut self, lon: f32, lat: f32) -> Result<(), JsValue> {
        let lon_idx = ((lon + 180.0) / 10.0).floor() as usize;
        let lat_idx = ((lat + 90.0) / 10.0).floor() as usize;

        // Load ONLY the exact tile containing the release point
        if let Err(e) = self.landmask.load_tile(lon_idx, lat_idx).await {
            web_sys::console::warn_1(&format!("Landmask tile load failed: {}", e).into());
        }

        Ok(())
    }

    pub async fn step(&mut self, step_count: u32) -> Result<(), JsValue> {
        let dt_days = 1.0 / self.steps_per_day as f32;
        self.step_count = step_count;
        let current_date_int = self.get_current_date_int();

        if step_count == 0 {
            self.simulation.release_particles(dt_days);
            return Ok(());
        }
        let hour = (24 * self.step_count / self.steps_per_day) % 24;

        self.loader.set_current_day(current_date_int, hour as usize);

        self.simulation.release_particles(dt_days);

        let needed_ocean_tiles = self.loader.update_tiles(&self.simulation.get_particles());

        if let Err(e) = self
            .loader
            .load_by_date(current_date_int, &needed_ocean_tiles)
            .await
        {
            web_sys::console::error_1(&format!("Failed to load ocean tiles: {:?}", e).into());
            return Err(JsValue::from_str(&format!("{:?}", e)));
        }

        let needed_landmask_tiles = self.landmask.update_tiles(&self.simulation.get_particles());

        for (lon_idx, lat_idx) in needed_landmask_tiles {
            if let Err(e) = self.landmask.load_tile(lon_idx, lat_idx).await {
                web_sys::console::warn_1(
                    &format!("Landmask tile load failed: {}_{}: {}", lon_idx, lat_idx, e).into(),
                );
            }
        }

        self.simulation.update_particles_batch(
            dt_days,
            &self.loader,
            hour as usize,
            &self.landmask,
        );

        self.step_count += 1;
        self.days_since_start = self.step_count as f32 / self.steps_per_day as f32;
        self.hour_count = hour as u32;
        Ok(())
    }

    pub fn get_positions(&self) -> Vec<f32> {
        let particles = self.simulation.get_particles();
        let mut positions = Vec::with_capacity(particles.len);
        for i in 0..particles.len {
            positions.push(particles.lons[i]);
            positions.push(particles.lats[i]);
        }
        positions
    }

    pub fn get_stranded_positions(&self) -> Vec<f32> {
        let particles = self.simulation.get_particles();
        let mut positions = Vec::with_capacity(particles.len);
        for i in 0..particles.len {
            if particles.stranded[i] {
                positions.push(particles.lons[i]);
                positions.push(particles.lats[i]);
            }
        }
        positions
    }

    pub fn get_unstranded_positions(&self) -> Vec<f32> {
        let particles = self.simulation.get_particles();
        let mut positions = Vec::with_capacity(particles.len);
        for i in 0..particles.len {
            if !particles.stranded[i] {
                positions.push(particles.lons[i]);
                positions.push(particles.lats[i]);
            }
        }
        positions
    }

    pub fn stranded_particle_count(&self) -> usize {
        self.simulation.get_particles().stranded_count()
    }

    pub fn current_day(&self) -> f32 {
        self.days_since_start
    }

    pub fn current_time_str(&self) -> String {
        let current_date = self.start_date + Days::new(self.days_since_start as u64);
        let year = current_date.year();
        let month = current_date.month();
        let day = current_date.day();
        format!(
            "{:04}-{:02}-{:02} {:02}:00",
            year, month, day, self.hour_count
        )
    }

    pub fn get_particle_bounding_box(&self) -> Vec<f32> {
        self.simulation.particles.bounding_box_array()
    }
    pub fn stranded_fraction(&self) -> f32 {
        let particles = self.simulation.get_particles();
        let total: usize = particles.len;
        if total == 0 {
            return 0.0;
        }
        let stranded = (0..total).filter(|&i| particles.stranded[i]).count();
        stranded as f32 / total as f32 * 100.0
    }

    pub fn mass_weighted_evaporation(&self) -> f32 {
        let particles = self.simulation.get_particles();
        let mut total_initial = 0.0;
        let mut total_evaporated = 0.0;

        match &particles.tracer {
            TracerKind::Oil(oil) => {
                for i in 0..particles.len {
                    if !particles.stranded[i] {
                        let initial_mass = self.simulation.initial_mass_per_particle;
                        total_initial += initial_mass;
                        total_evaporated += initial_mass * oil.data.f_evap[i];
                    }
                }
            }
        }

        if total_initial > 0.0 {
            total_evaporated / total_initial * 100.0
        } else {
            0.0
        }
    }

    pub fn mass_weighted_emulsification(&self) -> f32 {
        let particles = self.simulation.get_particles();
        let mut total_initial = 0.0;
        let mut total_emulsified = 0.0;

        match &particles.tracer {
            TracerKind::Oil(oil) => {
                for i in 0..particles.len {
                    if !particles.stranded[i] {
                        let initial_mass = self.simulation.initial_mass_per_particle;
                        total_initial += initial_mass;
                        total_emulsified += initial_mass * oil.data.y_w[i];
                    }
                }
            }
        }

        if total_initial > 0.0 {
            total_emulsified / total_initial * 100.0
        } else {
            0.0
        }
    }

    pub fn total_floating_mass_tons(&self) -> f32 {
        let particles = self.simulation.get_particles();
        let mut total_mass = 0.0;

        match &particles.tracer {
            TracerKind::Oil(oil) => {
                for i in 0..particles.len {
                    if !particles.stranded[i] {
                        total_mass += oil.data.total_mass[i];
                    }
                }
            }
        }

        total_mass
    }

    pub fn get_unstranded_positions_with_mass(&self) -> Vec<f32> {
        let particles = self.simulation.get_particles();
        let mut data = Vec::with_capacity(particles.len * 3);

        match &particles.tracer {
            TracerKind::Oil(oil) => {
                for i in 0..particles.len {
                    if !particles.stranded[i] {
                        data.push(particles.lons[i]);
                        data.push(particles.lats[i]);
                        data.push(oil.data.total_mass[i]);
                    }
                }
            }
        }

        data
    }

    pub fn is_on_land(&self, lon: f32, lat: f32) -> bool {
        self.landmask.is_on_land(lon, lat)
    }
}
