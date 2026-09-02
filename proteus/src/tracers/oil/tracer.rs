use super::super::Tracer;
use super::{weathering, OilData, OilProperties, OilPropertiesJson};

macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

pub struct OilTracer {
    pub properties: OilProperties,
    pub data: OilData,
}

impl Tracer for OilTracer {
    fn push(&mut self) {
        self.data.age.push(0.0);
        self.data
            .total_mass
            .push(self.properties.total_mass_per_particle);
        self.data
            .mass_components
            .extend_from_slice(&self.properties.initial_mass_components);
        self.data.f_evap.push(0.0);
        self.data.y_w.push(0.0);
        self.data.interfacial_area.push(0.0);
        self.data.emulsification_start_age.push(-1.0);
    }

    fn step(&mut self, indices: &[usize], wind_speeds: &[f32], sst_celsius: &[f32], dt: f32) {
        weathering::step_particle_weathering(
            &mut self.data,
            &indices,
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
    pub fn new(oil_json: &str, capacity: usize, total_mass_per_particle: f32) -> Self {
        let json: OilPropertiesJson =
            serde_json::from_str(oil_json).expect("Failed to parse oil JSON");

        let mass_components: Vec<f32> = json.component_mass_fractions
            .iter()
            .map(|&mass_frac| mass_frac * total_mass_per_particle)
            .collect();
        let boiling_points: Vec<f32> = json.boiling_points_c
            .iter()
            .map(|&temp_c| temp_c.max(0.0) + 273.15)
            .collect();

        let n_components = mass_components.len();

        Self {
            properties: OilProperties {
                total_mass_per_particle,
                wind_factor: 0.03,
                wind_deflection: None,
                product_type: json.product_type,
                api: json.api,
                density_kgm3: json.density_kgm3,
                dynamic_viscosity_cp: json.dynamic_viscosity_cp,
                interfacial_tension: json.interfacial_tension_n_m,
                asphaltenes_frac: json.sara_mass_fractions.asphaltenes,
                boiling_points: boiling_points,
                initial_mass_components: mass_components,
                molecular_weights: json.molecular_weights_kg_mol,
                bullwinkle_fraction: json.bullwinkle_fraction,
            },
            data: OilData {
                age: Vec::with_capacity(capacity),
                total_initial_mass: total_mass_per_particle,
                mass_components: Vec::with_capacity(capacity * n_components),
                total_mass: Vec::with_capacity(capacity),
                n_components,
                f_evap: Vec::with_capacity(capacity),
                emulsification_start_age: Vec::with_capacity(capacity),
                y_w: Vec::with_capacity(capacity),
                interfacial_area: Vec::with_capacity(capacity),
            },
        }
    }
}
