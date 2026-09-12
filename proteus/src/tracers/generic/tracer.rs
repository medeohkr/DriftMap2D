use super::super::Tracer;
use super::{GenericData, GenericProperties};

pub struct GenericTracer {
    pub properties: GenericProperties,
    pub data: GenericData
}

impl Tracer for GenericTracer {
    fn push(&mut self) {}

    fn step(&mut self, _indices: &[usize], _wind_speeds: &[f32], _sst_celsius: &[f32], _dt: f32) {}

    fn windage(&self, wind_u: f32, wind_v: f32, lat: f32) -> (f32, f32) {
            let w_factor = self.properties.wind_factor;

            let theta_deg = self.properties.wind_deflection;
            let theta = if lat >= 0.0 {
                theta_deg.to_radians()
            } else {
                -theta_deg.to_radians()
            };

            let cos_t = theta.cos();
            let sin_t = theta.sin();

            let u_drift = w_factor * (wind_u * cos_t - wind_v * sin_t);
            let v_drift = w_factor * (wind_u * sin_t + wind_v * cos_t);

            (u_drift, v_drift)
    }
}

impl GenericTracer {
    pub fn new(generic_json: &str, mass_per_particle: f32) -> Self {
        // let json: GenericProperties = serde_json::from_str(generic_json).expect("invalid JSON!");

        Self {
            properties: GenericProperties {
                wind_factor: 0.03,
                wind_deflection: 20.0
            },
            data: GenericData {
                mass_per_particle
            }
        }
    }
}

