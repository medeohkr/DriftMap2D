use super::super::Tracer;
use super::{GenericData, GenericProperties};

pub struct GenericTracer {
    pub properties: GenericProperties,
    pub data: GenericData
}

impl Tracer for GenericTracer {
    fn push(&mut self) {}

    fn step(&mut self, _indices: &[usize], _wind_speeds: &[f32], _sst_celsius: &[f32], _dt: f32) {}

    fn wind_f(&self) -> f32 {
        self.properties.wind_factor
    }

    fn wind_deg(&self) -> Option<f32> {
        self.properties.wind_deflection
    }
}

impl GenericTracer {
    pub fn new(generic_json: &str, mass_per_particle: f32) -> Self {
        // let json: GenericProperties = serde_json::from_str(generic_json).expect("invalid JSON!");

        Self {
            properties: GenericProperties {
                wind_factor: 0.03,
                wind_deflection: None
            },
            data: GenericData {
                mass_per_particle
            }
        }
    }
}

