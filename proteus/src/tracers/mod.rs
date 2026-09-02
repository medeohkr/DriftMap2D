mod oil;
// mod plastic;
// mod leeway;
// mod generic;
use serde::{Deserialize};
// pub use generic::GenericTracer;
pub use oil::{OilTracer, OilData};

// pub use plastic::PlasticTracer;
// pub use leeway::LeewayTracer;

pub enum TracerKind {
    // Generic(GenericTracer)
    Oil(OilTracer),
    // Plastic(PlasticTracer),
    // Leeway(LeewayTracer)
}

pub enum TracerData {
    // Generic(GenericData),
    Oil(OilData),
    // Plastic(PlasticData),
    // Leeway(LeewayData)
}

pub trait Tracer {
    fn push(&mut self);

    fn step(
        &mut self,
        indices: &[usize],
        wind_speeds: &[f32],
        sst_celsius: &[f32],
        dt: f32,
    );

    fn wind_f(&self) -> f32;

    fn wind_deg(&self) -> Option<f32>;
}

impl Tracer for TracerKind {
    fn push(&mut self) {
        match self {
            TracerKind::Oil(t) => t.push(),
        }
    }

    fn step(
        &mut self,
        indices: &[usize],
        wind_speeds: &[f32],
        sst_celsius: &[f32],
        dt: f32,
    ) {
        match self {
            TracerKind::Oil(t) => t.step(&indices, wind_speeds, sst_celsius, dt),
        }
    }

    fn wind_f(&self) -> f32 {
        match self {
            TracerKind::Oil(t) => t.properties.wind_factor,
        }
    }

    fn wind_deg(&self) -> Option<f32> {
        match self {
            TracerKind::Oil(t) => t.properties.wind_deflection,
        }
    }
}