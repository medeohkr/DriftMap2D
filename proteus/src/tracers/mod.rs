mod oil;
// mod plastic;
// mod leeway;
// mod generic;

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

pub enum TracerConfig {
    // Generic(GenericConfig),
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
    type Config;
    type Data;

    fn seed(&self, config: &Self::Config) -> Self::Data;
    fn step(
        &mut self,
        data: &mut Self::Data,
        wind_speed: f32,
        sst_celsius: f32,
        dt: f32,
    );
}

impl Tracer for TracerKind {
    type Config = TracerConfig;
    type Data = TracerData;

    fn seed(&self, config: &Self::Config) -> Self::Data {
        match (self, config) {
            (TracerKind::Oil(t), TracerConfig::Oil(c)) => {
                TracerData::Oil(t.seed(c))
            }
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
}