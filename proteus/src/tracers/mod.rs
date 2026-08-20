mod oil;
// mod plastic;
// mod leeway;
// mod generic;

use serde::{Deserialize};
// pub use generic::GenericTracer;
pub use oil::{OilTracer, OilConfig, OilData};
// pub use plastic::PlasticTracer;
// pub use leeway::LeewayTracer;

pub enum TracerKind {
    // Generic(GenericTracer)
    Oil(OilTracer),
    // Plastic(PlasticTracer),
    // Leeway(LeewayTracer)
}

#[derive(Debug, Clone, Deserialize)]  
#[serde(tag = "type")]
pub enum TracerConfig {
    // Generic(GenericConfig),
    #[serde(rename = "oil")]
    Oil(OilConfig),
    // Plastic(PlasticConfig),
    // Leeway(LeewayConfig)
}

pub enum TracerData {
    // Generic(GenericData),
    Oil(OilData),
    // Plastic(PlasticData),
    // Leeway(LeewayData)
}
pub trait Tracer {
    type Data;

    fn seed(&self) -> Self::Data;
    fn step(
        &mut self,
        data: &mut Self::Data,
        wind_speed: f32,
        sst_celsius: f32,
        dt: f32,
    );
    fn wind_f(&self) -> f32;
    fn wind_deg(&self) -> Option<f32>;
}

impl Tracer for TracerKind {
    type Data = TracerData;

    fn seed(&self) -> Self::Data {
        match self {
            TracerKind::Oil(t) => { TracerData::Oil(t.seed()) }
        }
    }

    fn step(
        &mut self,
        data: &mut Self::Data,
        wind_speed: f32,
        sst_celsius: f32,
        dt: f32,
    ) {
        match (self, data) {
            (TracerKind::Oil(t), TracerData::Oil(d)) => {
                t.step(d, wind_speed, sst_celsius, dt)
            }
        }
    }

    fn wind_f(&self) -> f32 {
        match self {
            TracerKind::Oil(t) => { t.wind_factor }
        }
    }

    fn wind_deg(&self) -> Option<f32> {
        match self {
            TracerKind::Oil(t) => { t.wind_deflection }
        }
    }
}