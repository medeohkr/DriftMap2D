#[derive(Debug, Clone)]
pub struct OilData {
    pub age: Vec<f32>,
    pub total_initial_mass: f32,
    pub total_mass: Vec<f32>,
    pub mass_components: Vec<f32>,
    pub n_components: usize,
    pub f_evap: Vec<f32>,
    pub y_w: Vec<f32>,
    pub interfacial_area: Vec<f32>,
    pub emulsification_start_age: Vec<f32>,
}
