#[derive(Debug, Clone)]
pub struct OilData {
    pub age: Vec<f32>,
    pub total_initial_mass: Vec<f32>,
    pub total_mass: Vec<f32>,
    pub mass_components: Vec<f32>,
    pub n_components: usize,
    pub f_evap: Vec<f32>,
    pub y_w: Vec<f32>,
    pub interfacial_area: Vec<f32>,
    pub emulsification_start_age: Vec<f32>,
}

impl OilData {
    pub fn mass_components_from_index(&self, index: usize) -> &[f32] {
        &self.mass_components[(index * self.n_components)..((index + 1) * self.n_components)]
    }
}