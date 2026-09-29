use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct OilProperties {
    pub mass_per_particle: f32,
    pub wind_factor: f32,
    pub wind_deflection: Option<f32>,
    pub product_type: String,
    pub api: f32,
    pub density_kgm3: Vec<(f32, f32)>,
    pub dynamic_viscosity_cp: Vec<(f32, f32)>,
    pub interfacial_tension: Vec<(f32, f32)>,
    pub asphaltenes_frac: f32,
    pub boiling_points: Vec<f32>,
    pub molecular_weights: Vec<f32>,
    pub initial_mass_components: Vec<f32>,
    pub bullwinkle_fraction: f32,
    pub bulltime: f32,
}

#[derive(Debug, Deserialize)]
pub struct OilPropertiesJson {
    pub wind_factor: f32,
    pub wind_deflection: Option<f32>,
    pub product_type: String,
    pub api: f32,
    pub density_kgm3: Vec<(f32, f32)>,
    pub dynamic_viscosity_cp: Vec<(f32, f32)>,
    pub interfacial_tension_n_m: Vec<(f32, f32)>,
    pub sara_mass_fractions: SaraFractions,
    pub _distillation_cuts: Vec<DistillationCut>,
    pub boiling_points_c: Vec<f32>,
    pub molecular_weights_kg_mol: Vec<f32>,
    pub component_mass_fractions: Vec<f32>,
    pub bullwinkle_fraction: f32,
    pub bulltime: f32,
}

#[derive(Debug, Deserialize)]
pub struct SaraFractions {
    pub _saturates: f32,
    pub _aromatics: f32,
    pub _resins: f32,
    pub asphaltenes: f32
}

#[derive(Debug, Deserialize)]
pub struct DistillationCut {
    pub _cumulative_fraction: f32,
    pub _vapor_temperature_c: f32
}