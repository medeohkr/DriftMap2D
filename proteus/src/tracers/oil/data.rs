#[derive(Debug, Clone)]
pub struct OilData {
    pub age: f32,
    pub total_mass: f32,
    pub mass_components: Vec<f32>,
    pub f_evap: f32,
    pub y_w: f32,
    pub interfacial_area: f32,
    pub emulsification_start_age: f32,
}