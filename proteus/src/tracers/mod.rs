mod generic;
mod oil;
mod leeway;

pub use generic::{GenericTracer, GenericData};
pub use oil::{OilTracer, OilData};
pub use leeway::{LeewayTracer, LeewayData};

pub enum TracerKind {
    Generic(GenericTracer),
    Oil(OilTracer),
    Leeway(LeewayTracer)
}

pub enum TracerData {
    Generic(GenericData),
    Oil(OilData),
    Leeway(LeewayData)
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

    fn windage(
        &self,
        wind_u: f32,
        wind_v: f32,
        lat: f32
    ) -> (f32, f32);
}

impl Tracer for TracerKind {
    fn push(&mut self) {
        match self {
            TracerKind::Generic(t) => t.push(),
            TracerKind::Oil(t) => t.push(),
            TracerKind::Leeway(t) => t.push(),
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
            TracerKind::Leeway(t) => t.step(&indices, wind_speeds, sst_celsius, dt),        }
    }

    fn windage(&self, wind_u: f32, wind_v: f32, lat: f32) -> (f32, f32) {
        match self {
            TracerKind::Generic(t) => t.windage(wind_u, wind_v, lat),
            TracerKind::Oil(t) => t.windage(wind_u, wind_v, lat),
            TracerKind::Leeway(t) => t.windage(wind_u, wind_v, lat)
        }
    }
}