use super::super::Tracer;
use super::{LeewayData, LeewayProperties};

pub struct LeewayTracer {
    pub properties: LeewayProperties,
    pub data: LeewayData
}

impl Tracer for LeewayTracer {
    fn push(&mut self) {}

    fn step(&mut self, _indices: &[usize], _wind_speeds: &[f32], _sst_celsius: &[f32], _dt: f32) {}

    fn windage(&self, wind_u: f32, wind_v: f32, lat: f32) -> (f32, f32) {
        
    }
}

impl LeewayTracer {
    pub fn new(Leeway_json: &str, mass_per_particle: f32) -> Self {
        // let json: LeewayProperties = serde_json::from_str(Leeway_json).expect("invalid JSON!");

        Self {
            properties: LeewayProperties {
                wind_factor: 0.03,
                wind_deflection: None
            },
            data: LeewayData {
                mass_per_particle
            }
        }
    }
}

