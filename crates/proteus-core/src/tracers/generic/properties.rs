use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]

pub struct GenericProperties {
    pub wind_factor: f32,
    pub wind_deflection: f32
}