use super::weathering;
use super::super::Tracer;
use super::OilConfig;
use super::OilData;
use super::adios::{AdiosOil, boiling_points};
use super::OilProperties;

macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

pub struct OilTracer {
    pub config: OilConfig,
    pub properties: OilProperties,
    pub data: OilData,
}

impl Tracer for OilTracer {
    fn push(&mut self) {
        self.data.age.push(0.0);
        self.data.total_mass.push(self.properties.total_mass_per_particle);
        self.data.mass_components.extend_from_slice(&self.properties.initial_mass_components);
        self.data.emulsification_start_age.push(-1.0);
        self.data.f_evap.push(0.0);
    }
    fn step(
        &mut self,
        wind_speeds: &[f32],
        sst_celsius: &[f32],
        dt: f32,
    ) {
        weathering::step_particle_weathering(
            &mut self.data,
            &self.properties,
            wind_speeds,
            sst_celsius,
            dt,
        );
    }

    fn wind_f(&self) -> f32 {
        self.properties.wind_factor
    }

    fn wind_deg(&self) -> Option<f32> {
        self.properties.wind_deflection
    }
}

impl OilTracer {
    pub fn new(config: OilConfig, capacity: usize, total_mass_per_particle: f32) -> Self {
        let adios: AdiosOil = serde_json::from_str(&config.adios_json)
            .expect("Failed to parse ADIOS JSON");

        let initial_mass_components = adios.initial_mass_components(total_mass_per_particle);
        let n_components = initial_mass_components.len();

        Self {
            config: config.clone(),
            properties: OilProperties {
                total_mass_per_particle,
                wind_factor: config.overrides.wind_factor.unwrap_or(0.03),
                wind_deflection: config.overrides.wind_factor,
                product_type: adios.metadata.product_type.clone(),
                api: config.overrides.api.unwrap_or(adios.metadata.api),
                density_kgm3: config.overrides.density_kgm3.unwrap_or(adios.densities()),
                dynamic_viscosity_cp: config.overrides.dynamic_viscosity_cp.unwrap_or(adios.viscosities()),
                boiling_points: boiling_points(adios.distillation_cuts().unwrap_or(adios.distillation_cuts_from_api(10))),
                initial_mass_components,
                molecular_weights: adios.molecular_weights(),
                bullwinkle_fraction: config.overrides.bullwinkle_fraction.unwrap_or(adios.bullwinkle_fraction()),
                interfacial_tension: adios.interfacial_tension(),
            },
            data: OilData {
                age: Vec::with_capacity(capacity),
                total_initial_mass: total_mass_per_particle,
                mass_components: Vec::with_capacity(capacity * n_components),
                total_mass: Vec::with_capacity(capacity),
                n_components: n_components,
                f_evap: Vec::with_capacity(capacity),
                emulsification_start_age: Vec::with_capacity(capacity),
                y_w: Vec::with_capacity(capacity),
                interfacial_area: Vec::with_capacity(capacity)
            }
        }
    }
}