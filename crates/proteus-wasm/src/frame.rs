//! frame.rs collects positions and statistics together for an immutable browser frame

use proteus_core::{basemodel::Particles, tracers::TracerKind};
use wasm_bindgen::JsValue;

#[derive(Default)]
pub(crate) struct ParticleFrame {
    active: Vec<f32>,
    stranded: Vec<f32>,
    bounds: [f32; 4],
    // active count, stranded percent, evaporation percent, emulsification percent, floating tons
    summary: [f64; 5],
}

impl ParticleFrame {
    pub fn collect(&mut self, particles: &Particles, initial_mass: f32) {
        self.active.clear();
        self.stranded.clear();
        self.bounds = [f32::MAX, f32::MIN, f32::MAX, f32::MIN];
        let (mut total_mass, mut total_initial, mut evaporated, mut emulsified) =
            (0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32);
        for i in 0..particles.len {
            let pair = [particles.lons[i], particles.lats[i]];
            if particles.stranded[i] {
                self.stranded.extend_from_slice(&pair);
                continue;
            }
            self.active.extend_from_slice(&pair);
            self.bounds[0] = self.bounds[0].min(pair[0]);
            self.bounds[1] = self.bounds[1].max(pair[0]);
            self.bounds[2] = self.bounds[2].min(pair[1]);
            self.bounds[3] = self.bounds[3].max(pair[1]);
            match &particles.tracer {
                TracerKind::Generic(tracer) => total_mass += tracer.data.mass_per_particle,
                TracerKind::Leeway(_) => total_mass += 1.0,
                TracerKind::Oil(tracer) => {
                    total_mass += tracer.data.total_mass[i];
                    total_initial += initial_mass;
                    evaporated += initial_mass * tracer.data.f_evap[i];
                    emulsified += initial_mass * tracer.data.y_w[i];
                }
            }
        }
        let percentage = |value: f32| {
            if total_initial > 0.0 {
                value / total_initial * 100.0
            } else {
                0.0
            }
        };
        let stranded = if particles.len > 0 {
            (self.stranded.len() / 2) as f32 / particles.len as f32 * 100.0
        } else {
            0.0
        };
        self.summary = [
            (self.active.len() / 2) as f64,
            stranded as f64,
            percentage(evaporated) as f64,
            percentage(emulsified) as f64,
            total_mass as f64,
        ];
    }

    /// typed arrays own their data so later Rust buffer reuse cannot alter saved snapshots
    pub fn to_js(&self) -> JsValue {
        let object = js_sys::Object::new();
        for (name, value) in [
            (
                "activePositions",
                JsValue::from(js_sys::Float32Array::from(self.active.as_slice())),
            ),
            (
                "strandedPositions",
                JsValue::from(js_sys::Float32Array::from(self.stranded.as_slice())),
            ),
            (
                "boundingBox",
                JsValue::from(js_sys::Float32Array::from(self.bounds.as_slice())),
            ),
            (
                "summary",
                JsValue::from(js_sys::Float64Array::from(self.summary.as_slice())),
            ),
        ] {
            js_sys::Reflect::set(&object, &JsValue::from_str(name), &value).unwrap();
        }
        object.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proteus_core::tracers::GenericTracer;

    #[test]
    fn collection_preserves_order_bounds_statistics_and_storage() {
        let mut particles = Particles::new(
            3,
            TracerKind::Generic(GenericTracer::new(
                r#"{"wind_factor":0,"wind_deflection":0}"#,
                0.25,
            )),
        );
        for (x, y) in [(3.0, 59.0), (5.0, 60.0), (4.0, 58.0)] {
            particles.add_particle(x, y);
        }
        particles.stranded[1] = true;
        let mut frame = ParticleFrame::default();
        frame.collect(&particles, 0.25);
        assert_eq!(frame.active, [3.0, 59.0, 4.0, 58.0]);
        assert_eq!(frame.stranded, [5.0, 60.0]);
        assert_eq!(frame.bounds, particles.bounding_box_array().as_slice());
        assert_eq!(
            frame.summary,
            [2.0, (1.0_f32 / 3.0 * 100.0) as f64, 0.0, 0.0, 0.5]
        );
        let pointers = (frame.active.as_ptr(), frame.stranded.as_ptr());
        frame.collect(&particles, 0.25);
        assert_eq!(pointers, (frame.active.as_ptr(), frame.stranded.as_ptr()));
        particles.stranded.fill(true);
        frame.collect(&particles, 0.25);
        assert!(frame.active.is_empty());
        assert_eq!(frame.summary, [0.0, 100.0, 0.0, 0.0, 0.0]);
        frame.collect(
            &Particles::new(
                0,
                TracerKind::Generic(GenericTracer::new(
                    r#"{"wind_factor":0,"wind_deflection":0}"#,
                    1.0,
                )),
            ),
            1.0,
        );
        assert_eq!(frame.summary, [0.0; 5]);
    }
}
