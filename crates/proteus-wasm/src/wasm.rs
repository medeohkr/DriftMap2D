use proteus_core::basemodel::{LandMaskLoader, Simulation};
use proteus_core::forcing::DataLoader;
use proteus_core::tracers::{TracerKind};
use super::fetch::GlooFetcher;
use chrono::Duration;
use chrono::Timelike;
use chrono::{Datelike, Days, NaiveDateTime};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Proteus {
    simulation: Simulation,
    loader: DataLoader<GlooFetcher>,
    landmask: LandMaskLoader<GlooFetcher>,
    start_date: NaiveDateTime,
    steps_per_day: u32,
    last_step_count: u32,
}

#[wasm_bindgen]
pub fn setup_panic_hook() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
impl Proteus {
    #[wasm_bindgen(constructor)]
    pub fn new(
        tracer_type: &str,
        tracer_json: &str,
        start_date_str: &str,
        releases_json: &str,
        particle_count: usize,
        time_step_minutes: f32,
        advection_scheme: &str,
        diffusion_scheme: &str,
        diffusion_coeffs: Vec<f32>
    ) -> Self {
        console_log::init_with_level(log::Level::Debug).ok();
        console_error_panic_hook::set_once();

        let start_date = NaiveDateTime::parse_from_str(start_date_str, "%Y-%m-%d %H:%M")
            .expect("Invalid date format");
        let steps_per_day = (1440.0 / time_step_minutes) as u32;

        let simulation = Simulation::new(
            tracer_type,
            tracer_json,
            releases_json,
            particle_count,
            steps_per_day,
            advection_scheme,
            diffusion_scheme,
            diffusion_coeffs
        );

        let loader = DataLoader::new(
            "https://tiles.driftmap2d.com/tiles",
            -180.0,
            -80.0,
            GlooFetcher,
        );
        let landmask = LandMaskLoader::new(
            "https://tiles.driftmap2d.com/roaring_landmask",
            -180.0,
            -90.0,
            90.0,
            GlooFetcher,
        );

        Self {
            simulation,
            loader,
            landmask,
            start_date,
            steps_per_day,
            last_step_count: 0,
        }
    }

    pub async fn step(&mut self, step_count: u32) -> Result<(), JsValue> {
        let dt_days = 1.0 / self.steps_per_day as f32;

        if step_count == 0 {
            self.simulation.release_particles(step_count);
            self.last_step_count = 0;
            return Ok(());
        }

        self.simulation.release_particles(step_count);

        let hour = (24.0 * step_count.saturating_sub(1) as f32 / self.steps_per_day as f32) % 24.0;

        // Forcing date: the day containing the START of the advance, (step_count - 1) * dt
        let forcing_day_offset = step_count.saturating_sub(1) / self.steps_per_day;
        let forcing_date = self.start_date + Days::new(forcing_day_offset as u64);
        let current_date_int = forcing_date.year() as usize * 10000
            + forcing_date.month() as usize * 100
            + forcing_date.day() as usize;

        self.loader
            .load_ocean_tiles(self.get_unstranded_positions(), current_date_int)
            .await;
        self.landmask
            .load_landmask_tiles(self.get_unstranded_positions())
            .await;

        self.simulation.update_particles_batch(
            dt_days,
            &self.loader,
            current_date_int,
            hour,
            &self.landmask,
        );

        self.last_step_count = step_count;
        Ok(())
    }

    pub async fn load_landmask_tiles(&mut self, positions: Vec<f32>) {
        self.landmask.load_landmask_tiles(positions).await
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

    pub fn get_total_mass(&self) -> f32 {
        self.simulation.release_manager.total_mass
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

    pub fn active_particle_count(&self) -> usize {
        self.simulation.get_particles().unstranded_count()
    }

    pub fn current_day(&self) -> f32 {
        self.last_step_count as f32 / self.steps_per_day as f32
    }

    pub fn get_current_date_int(&self) -> usize {
        let day_offset = self.last_step_count / self.steps_per_day;
        let date = self.start_date + Days::new(day_offset as u64);
        date.year() as usize * 10000
            + date.month() as usize * 100
            + date.day() as usize
    }
    
    pub fn current_time_str(&self) -> String {
        let seconds = (self.current_day() * 86400.0) as i64;
        let current_date = self.start_date + Duration::seconds(seconds);

        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}",
            current_date.year(),
            current_date.month(),
            current_date.day(),
            current_date.hour(),
            current_date.minute()
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
                        let initial_mass =
                            self.simulation.release_manager.initial_mass_per_particle();
                        total_initial += initial_mass;
                        total_evaporated += initial_mass * oil.data.f_evap[i];
                    }
                }
            }

            _ => {}
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
                        let initial_mass =
                            self.simulation.release_manager.initial_mass_per_particle();
                        total_initial += initial_mass;
                        total_emulsified += initial_mass * oil.data.y_w[i];
                    }
                }
            }

            _ => {}
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
            TracerKind::Generic(generic) => {
                for i in 0..particles.len {
                    if !particles.stranded[i] {
                        total_mass += generic.data.mass_per_particle;
                    }
                }
            }

            TracerKind::Oil(oil) => {
                for i in 0..particles.len {
                    if !particles.stranded[i] {
                        total_mass += oil.data.total_mass[i];
                    }
                }
            }

            _ => {
                for i in 0..particles.len {
                    if !particles.stranded[i] {
                        total_mass += 1.0;
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
            TracerKind::Generic(generic) => {
                for i in 0..particles.len {
                    if !particles.stranded[i] {
                        data.push(particles.lons[i]);
                        data.push(particles.lats[i]);
                        data.push(generic.data.mass_per_particle);
                    }
                }
            }

            TracerKind::Oil(oil) => {
                for i in 0..particles.len {
                    if !particles.stranded[i] {
                        data.push(particles.lons[i]);
                        data.push(particles.lats[i]);
                        data.push(oil.data.total_mass[i]);
                    }
                }
            }

            _ => {
                for i in 0..particles.len {
                    if !particles.stranded[i] {
                        data.push(particles.lons[i]);
                        data.push(particles.lats[i]);
                        data.push(1.0);
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
