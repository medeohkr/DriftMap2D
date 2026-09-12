use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]

pub struct LeewayProperties {
    pub downwind: [f32; 3],
    pub right: [f32; 3],
    pub left: [f32; 3]
}