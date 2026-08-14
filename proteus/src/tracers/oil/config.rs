use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct OilConfig {
    pub adios_json: String,
    #[serde(default)]
    pub overrides: OilOverrides,
    pub total_mass_per_particle: f32
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct OilOverrides {
    pub api: Option<f32>,
    pub density_kgm3: Option<Vec<(f32, f32)>>,
    pub dynamic_viscosity_cp: Option<Vec<(f32, f32)>>,
    pub bullwinkle_fraction: Option<f32>,
    pub interfacial_tension: Option<Vec<(f32, f32)>>,
}