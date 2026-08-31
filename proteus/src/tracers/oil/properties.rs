#[derive(Debug, Clone)]
pub struct OilProperties {
    pub total_mass_per_particle: f32,
    pub wind_factor: f32,
    pub wind_deflection: Option<f32>,
    pub product_type: String,
    pub api: f32,
    pub density_kgm3: Vec<(f32, f32)>,
    pub dynamic_viscosity_cp: Vec<(f32, f32)>,
    pub boiling_points: Vec<f32>,
    pub initial_mass_components: Vec<f32>,
    pub molecular_weights: Vec<f32>,
    pub bullwinkle_fraction: f32,
    pub interfacial_tension: Vec<(f32, f32)>,
}
