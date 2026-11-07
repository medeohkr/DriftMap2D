// tests.rs checks shared queries, tracer timing, and reusable step storage

use super::*;
use crate::{forcing::LoaderError, BoxFuture, FetchError};
use std::cell::RefCell;

struct UnusedFetcher;
impl TileFetcher for UnusedFetcher {
    fn fetch_bytes<'a>(&'a self, _: &'a str) -> BoxFuture<'a, Result<Vec<u8>, FetchError>> {
        Box::pin(async { panic!("unit test must not fetch tiles") })
    }
}

#[derive(Default)]
struct RecordingForcing {
    calls: RefCell<Vec<(ForcingFields, f32, Vec<(f32, f32)>)>>,
    missing_wind: bool,
}

impl Forcing for RecordingForcing {
    async fn prepare_forcing(
        &mut self,
        _: Vec<f32>,
        _: usize,
        _: f32,
        _: f32,
    ) -> Result<(), LoaderError> {
        Ok(())
    }
    fn validate_forcing_interval(&self, _: usize, _: f32, _: f32) -> Result<(), LoaderError> {
        Ok(())
    }
    fn cell_area_m2(&self, _: f32) -> f32 {
        640000.0
    }
    fn sample_fields(
        &self,
        positions: impl ExactSizeIterator<Item = (f32, f32)>,
        _: usize,
        hour: f32,
        fields: ForcingFields,
    ) -> Vec<[f32; 5]> {
        let positions: Vec<_> = positions.collect();
        let missing = self.missing_wind && self.calls.borrow().is_empty();
        self.calls
            .borrow_mut()
            .push((fields, hour, positions.clone()));
        positions
            .iter()
            .map(|_| [0.0, 0.0, if missing { f32::NAN } else { 20.0 }, 0.0, 280.0])
            .collect()
    }
}

fn mask() -> LandMaskLoader<UnusedFetcher> {
    LandMaskLoader::new("unused", -180.0, -90.0, 90.0, UnusedFetcher)
}

fn leeway(capsizing: bool) -> String {
    serde_json::json!({
        "downwind": [1.0,0.0,0.0], "right": [0.0,0.0,0.0], "left": [0.0,0.0,0.0],
        "jibe_probability": 0.0, "capsizing": capsizing,
        "capsize_threshold": -100.0, "capsize_fraction": 0.0, "capsize_sigma": 1.0,
    })
    .to_string()
}

fn simulation(kind: &str, properties: &str, scheme: &str) -> Simulation {
    let mut simulation = Simulation::new(
        kind,
        properties,
        r#"[{"lon":3,"lat":59,"radius":0,"schedule":[{"amount":1,"duration":0}]}]"#,
        3,
        96,
        scheme,
        "constant",
        vec![0.0, 0.0],
    );
    simulation.release_particles(0);
    simulation
}

#[test]
fn initial_wind_is_shared_and_intermediate_stages_keep_their_times_and_positions() {
    for (scheme, offsets) in [
        ("euler", vec![0.0]),
        ("rk2", vec![0.0, 0.125]),
        ("rk4", vec![0.0, 0.125, 0.125, 0.25]),
    ] {
        for (kind, properties) in [
            (
                "generic",
                r#"{"wind_factor":0.02,"wind_deflection":0}"#.to_owned(),
            ),
            ("sar", leeway(false)),
        ] {
            let mut simulation = simulation(kind, &properties, scheme);
            simulation.particles.stranded[1] = true;
            let forcing = RecordingForcing::default();
            simulation.update_particles_batch(1.0 / 96.0, &forcing, 20261105, 23.9, &mask());
            let calls = forcing.calls.borrow();
            assert_eq!(calls.len(), offsets.len());
            for (call, offset) in calls.iter().zip(offsets.iter()) {
                assert_eq!(call.0, ForcingFields::CurrentsWind);
                assert!((call.1 - (23.9 + offset)).abs() < 0.00001);
                assert_eq!(call.2.len(), 2);
            }
            assert_eq!(calls[0].2, [(3.0, 59.0); 2]);
            if calls.len() > 1 {
                assert!(calls[1].2[0].0 > calls[0].2[0].0);
            }
            assert_eq!(
                (simulation.particles.lons[1], simulation.particles.lats[1]),
                (3.0, 59.0)
            );
        }
    }
}

#[test]
fn tracer_state_changes_are_applied_before_using_the_shared_velocity() {
    let mut simulation = simulation("sar", &leeway(true), "euler");
    simulation.particles.stranded[1] = true;
    let forcing = RecordingForcing::default();
    simulation.update_particles_batch(1.0 / 24.0, &forcing, 20261105, 12.0, &mask());
    let TracerKind::Leeway(tracer) = &simulation.particles.tracer else {
        unreachable!()
    };
    assert_eq!(tracer.data.capsized, [true, false, true]);
    assert_eq!(simulation.particles.lons, [3.0; 3]);
    assert_eq!(forcing.calls.borrow().len(), 1);
}

#[test]
fn step_buffers_keep_capacity_when_active_particles_shrink_and_reappear() {
    let mut simulation = simulation(
        "generic",
        r#"{"wind_factor":0.02,"wind_deflection":0}"#,
        "rk4",
    );
    let forcing = RecordingForcing::default();
    simulation.update_particles_batch(1.0 / 96.0, &forcing, 20261105, 12.0, &mask());
    let pointers = |b: &StepBuffers| {
        (
            b.indices.as_ptr(),
            b.environment.components[0].as_ptr(),
            b.stage_environment.components[0].as_ptr(),
            b.initial_velocities.x.as_ptr(),
            b.positions.x.as_ptr(),
        )
    };
    let original = pointers(&simulation.buffers);
    simulation.particles.stranded[1] = true;
    simulation.update_particles_batch(1.0 / 96.0, &forcing, 20261105, 12.25, &mask());
    assert_eq!(pointers(&simulation.buffers), original);
    assert_eq!(simulation.buffers.indices, [0, 2]);
    assert_eq!(simulation.buffers.environment.len(), 2);
    assert_eq!(simulation.buffers.positions.len(), 2);
    simulation.particles.stranded.fill(true);
    let queries = forcing.calls.borrow().len();
    simulation.update_particles_batch(1.0 / 96.0, &forcing, 20261105, 12.5, &mask());
    assert_eq!(queries, forcing.calls.borrow().len());
    simulation.particles.stranded.fill(false);
    simulation.update_particles_batch(1.0 / 96.0, &forcing, 20261105, 12.75, &mask());
    assert_eq!(pointers(&simulation.buffers), original);
    assert_eq!(simulation.buffers.positions.len(), 3);
}

#[test]
fn missing_wind_retains_the_previous_second_retry_without_resampling_currents() {
    let mut simulation = simulation(
        "generic",
        r#"{"wind_factor":0.02,"wind_deflection":0}"#,
        "euler",
    );
    let forcing = RecordingForcing {
        missing_wind: true,
        ..Default::default()
    };
    simulation.update_particles_batch(1.0 / 96.0, &forcing, 20261105, 12.0, &mask());
    let calls = forcing.calls.borrow();
    assert_eq!(
        calls.iter().map(|c| c.0).collect::<Vec<_>>(),
        [ForcingFields::CurrentsWind, ForcingFields::Wind]
    );
    assert!(simulation
        .particles
        .lons
        .iter()
        .all(|v| v.is_finite() && *v > 3.0));
}

#[test]
fn oil_uses_the_combined_temperature_sample_in_active_particle_order() {
    let properties = serde_json::json!({
        "wind_factor":0.02,"wind_deflection":0.0,"product_type":"test","api":30.0,
        "density_kgm3":[[900.0,15.0]],"dynamic_viscosity_cp":[[9.0,15.0]],
        "interfacial_tension_n_m":[[0.03,15.0]],"sara_mass_fractions":{"asphaltenes":0.03},
        "boiling_points_c":[100.0,400.0],"molecular_weights_kg_mol":[0.1,0.3],
        "component_mass_fractions":[0.5,0.5],"bullwinkle_fraction":0.05,"bulltime":0.0,
    });
    let mut simulation = simulation("oil", &properties.to_string(), "euler");
    simulation.particles.stranded[1] = true;
    let forcing = RecordingForcing::default();
    simulation.update_particles_batch(1.0 / 96.0, &forcing, 20261105, 12.0, &mask());
    assert_eq!(forcing.calls.borrow()[0].0, ForcingFields::All);
    assert_eq!(forcing.calls.borrow().len(), 1);
    assert_eq!(simulation.buffers.wind_speeds, [20.0; 2]);
    assert_eq!(simulation.buffers.sst_celsius, [280.0 - 273.15; 2]);
    let TracerKind::Oil(tracer) = &simulation.particles.tracer else {
        unreachable!()
    };
    assert_eq!(tracer.data.age, [900.0, 0.0, 900.0]);
}
