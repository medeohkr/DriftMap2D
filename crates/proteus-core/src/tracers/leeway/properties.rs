use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]

pub struct LeewayProperties {
    pub downwind: [f32; 3],
    pub right: [f32; 3],
    pub left: [f32; 3],

    pub jibe_probability: f32,

    pub capsizing: bool,
    pub capsize_threshold: f32,
    pub capsize_fraction: f32,
    pub capsize_sigma: f32,
}