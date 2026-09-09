mod generic;
mod oil;

pub use generic::{GenericTracer, GenericData};
pub use oil::{OilTracer, OilData};

pub enum TracerKind {
    Generic(GenericTracer),
    Oil(OilTracer),
}

pub enum TracerData {
    Generic(GenericData),
    Oil(OilData),
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
            TracerKind::Generic(t) => t.push(),
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
            TracerKind::Generic(t) => t.step(&indices, wind_speeds, sst_celsius, dt),
            TracerKind::Oil(t) => t.step(&indices, wind_speeds, sst_celsius, dt),
        }
    }

    fn wind_f(&self) -> f32 {
        match self {
            TracerKind::Generic(t) => t.properties.wind_factor,
            TracerKind::Oil(t) => t.properties.wind_factor,
        }
    }

    fn wind_deg(&self) -> Option<f32> {
        match self {
            TracerKind::Generic(t) => t.properties.wind_deflection,
            TracerKind::Oil(t) => t.properties.wind_deflection,
        }
    }
}