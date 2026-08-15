use super::weathering;
use super::super::Tracer;
use super::OilConfig;
use super::OilData;
use super::AdiosOil;

pub struct OilTracer {
    pub config: OilConfig,
    pub wind_factor: f32,
    pub wind_deflection: Option<f32>,
    pub product_type: String,
    pub api: f32,
    pub density_kgm3: Vec<(f32, f32)>,
    pub dynamic_viscosity_cp: Vec<(f32, f32)>,
    pub distillation_cuts: Vec<(f32, f32)>,
    pub initial_mass_components: Vec<f32>,
    pub molecular_weights: Vec<f32>,
    pub bullwinkle_fraction: f32,
    pub interfacial_tension: Vec<(f32, f32)>,
}

impl Tracer for OilTracer {
    type Data = OilData;

    fn seed(&self) -> Self::Data {
        OilData {
            age: 0.0,
            total_mass: self.config.total_mass_per_particle,
            mass_components: self.initial_mass_components.clone(),
            f_evap: 0.0,
            y_w: 0.0,
            interfacial_area: 0.0,
            emulsification_start_age: -1.0
        }
    }

    fn step(
        &mut self,
        data: &mut Self::Data,
        wind_speed: f32,
        sst_celsius: f32,
        dt: f32,
    ) {
        weathering::step_particle_weathering(
            data,
            self,
            wind_speed,
            sst_celsius,
            dt,
        );

    }

    fn wind_f(&self) -> f32 {
        self.wind_factor
    }

    fn wind_deg(&self) -> Option<f32> {
        self.wind_deflection
    }
}

impl OilTracer {
    pub fn new(config: OilConfig) -> Self {
        let adios: AdiosOil = serde_json::from_str(&config.adios_json)
            .expect("Failed to parse ADIOS JSON");
        Self {
            config: config.clone(),
            wind_factor: config.overrides.wind_factor.unwrap_or(0.03),
            wind_deflection: config.overrides.wind_factor,
            product_type: adios.metadata.product_type.clone(),
            api: config.overrides.api.unwrap_or(adios.metadata.api),
            density_kgm3: config.overrides.density_kgm3.unwrap_or(adios.densities()),
            dynamic_viscosity_cp: config.overrides.dynamic_viscosity_cp.unwrap_or(adios.viscosities()),
            distillation_cuts: adios.distillation_cuts().unwrap_or(adios.distillation_cuts_from_api(10)),
            initial_mass_components: adios.initial_mass_components(config.total_mass_per_particle),
            molecular_weights: adios.molecular_weights(),
            bullwinkle_fraction: config.overrides.bullwinkle_fraction.unwrap_or(adios.bullwinkle_fraction()),
            interfacial_tension: adios.interfacial_tension()
        }
    }
}