// integrators.rs advances compact component arrays with reusable stage buffers

use super::ParticleView;
use crate::arrays::{Coordinates, VectorBuffer};

pub enum Integrator {
    Euler,
    Midpoint,
    RK4,
}

#[derive(Default)]
pub struct IntegrationBuffers {
    intermediate: VectorBuffer,
    k2: VectorBuffer,
    k3: VectorBuffer,
    k4: VectorBuffer,
}

impl Integrator {
    /// initial velocities include tracer changes at the beginning of this step
    pub fn integrate_soa_into(
        &self,
        positions: Coordinates<'_>,
        dt: f32,
        k1: Coordinates<'_>,
        mut sample: impl FnMut(Coordinates<'_>, f32, &mut VectorBuffer),
        buffers: &mut IntegrationBuffers,
        output: &mut VectorBuffer,
    ) {
        if matches!(self, Self::Euler) {
            advance(positions, dt, k1, output);
            return;
        }
        advance(positions, 0.5 * dt, k1, &mut buffers.intermediate);
        sample(
            buffers.intermediate.as_coordinates(),
            0.5 * dt,
            &mut buffers.k2,
        );
        if matches!(self, Self::Midpoint) {
            advance(positions, dt, buffers.k2.as_coordinates(), output);
            return;
        }
        advance(
            positions,
            0.5 * dt,
            buffers.k2.as_coordinates(),
            &mut buffers.intermediate,
        );
        sample(
            buffers.intermediate.as_coordinates(),
            0.5 * dt,
            &mut buffers.k3,
        );
        advance(
            positions,
            dt,
            buffers.k3.as_coordinates(),
            &mut buffers.intermediate,
        );
        sample(buffers.intermediate.as_coordinates(), dt, &mut buffers.k4);
        output.resize(positions.len());
        rk4_component(
            positions.x,
            dt,
            k1.x,
            &buffers.k2.x,
            &buffers.k3.x,
            &buffers.k4.x,
            &mut output.x,
        );
        rk4_component(
            positions.y,
            dt,
            k1.y,
            &buffers.k2.y,
            &buffers.k3.y,
            &buffers.k4.y,
            &mut output.y,
        );
    }

    /// compatibility adapter for callers with tuple coordinates
    pub fn integrate_into(
        &self,
        view: &ParticleView,
        dt: f32,
        k1: &[(f32, f32)],
        mut sample: impl FnMut(&[(f32, f32)], f32, &mut Vec<(f32, f32)>),
        buffers: &mut IntegrationBuffers,
        output: &mut Vec<(f32, f32)>,
    ) {
        let mut positions = VectorBuffer::default();
        positions.extend(view.iter().map(|(_, x, y)| (x, y)));
        let mut initial = VectorBuffer::default();
        initial.extend(k1.iter().copied());
        let mut result = VectorBuffer::default();
        let mut input = Vec::new();
        let mut velocities = Vec::new();
        self.integrate_soa_into(
            positions.as_coordinates(),
            dt,
            initial.as_coordinates(),
            |p, time, out| {
                input.clear();
                input.extend(p.iter());
                sample(&input, time, &mut velocities);
                out.clear();
                out.extend(velocities.iter().copied());
            },
            buffers,
            &mut result,
        );
        output.clear();
        output.extend(result.as_coordinates().iter());
    }

    pub fn integrate(
        &self,
        view: &ParticleView,
        dt: f32,
        get_velocities_view: impl Fn(&ParticleView) -> Vec<(f32, f32)>,
        get_velocities_slice: impl Fn(&[(f32, f32)], f32) -> Vec<(f32, f32)>,
    ) -> Vec<(f32, f32)> {
        let initial = get_velocities_view(view);
        let mut output = Vec::new();
        self.integrate_into(
            view,
            dt,
            &initial,
            |p, elapsed, out| {
                out.clear();
                out.extend(get_velocities_slice(p, elapsed));
            },
            &mut IntegrationBuffers::default(),
            &mut output,
        );
        output
    }
}

/// dense component arithmetic avoids active-index gathers in every RK stage
fn advance(
    positions: Coordinates<'_>,
    dt: f32,
    velocities: Coordinates<'_>,
    output: &mut VectorBuffer,
) {
    assert_eq!(positions.len(), velocities.len());
    output.resize(positions.len());
    advance_component(positions.x, dt, velocities.x, &mut output.x);
    advance_component(positions.y, dt, velocities.y, &mut output.y);
}

fn advance_component(position: &[f32], dt: f32, velocity: &[f32], output: &mut [f32]) {
    assert_eq!(position.len(), velocity.len());
    assert_eq!(position.len(), output.len());
    for ((out, &p), &v) in output.iter_mut().zip(position).zip(velocity) {
        *out = p + dt * v;
    }
}

fn rk4_component(
    position: &[f32],
    dt: f32,
    k1: &[f32],
    k2: &[f32],
    k3: &[f32],
    k4: &[f32],
    output: &mut [f32],
) {
    let count = position.len();
    assert_eq!(count, k1.len());
    assert_eq!(count, k2.len());
    assert_eq!(count, k3.len());
    assert_eq!(count, k4.len());
    assert_eq!(count, output.len());
    for i in 0..count {
        output[i] = position[i] + dt * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]) / 6.0;
    }
}

pub fn euler_step(
    view: &ParticleView,
    dt: f32,
    initial: impl Fn(&ParticleView) -> Vec<(f32, f32)>,
    stages: impl Fn(&[(f32, f32)], f32) -> Vec<(f32, f32)>,
) -> Vec<(f32, f32)> {
    Integrator::Euler.integrate(view, dt, initial, stages)
}
pub fn midpoint_step(
    view: &ParticleView,
    dt: f32,
    initial: impl Fn(&ParticleView) -> Vec<(f32, f32)>,
    stages: impl Fn(&[(f32, f32)], f32) -> Vec<(f32, f32)>,
) -> Vec<(f32, f32)> {
    Integrator::Midpoint.integrate(view, dt, initial, stages)
}
pub fn rk4_step(
    view: &ParticleView,
    dt: f32,
    initial: impl Fn(&ParticleView) -> Vec<(f32, f32)>,
    stages: impl Fn(&[(f32, f32)], f32) -> Vec<(f32, f32)>,
) -> Vec<(f32, f32)> {
    Integrator::RK4.integrate(view, dt, initial, stages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        basemodel::Particles,
        tracers::{GenericTracer, TracerKind},
    };
    use std::cell::RefCell;

    fn particles() -> Particles {
        let tracer = GenericTracer::new(r#"{"wind_factor":0,"wind_deflection":0}"#, 1.0);
        let mut particles = Particles::new(3, TracerKind::Generic(tracer));
        particles.add_particle(1.0, 2.0);
        particles.add_particle(99.0, 99.0);
        particles.add_particle(3.0, 4.0);
        particles.stranded[1] = true;
        particles
    }

    #[test]
    fn all_integrators_preserve_surface_order_and_skip_stranded_particles() {
        let particles = particles();
        let view = particles.view();
        assert_eq!(
            view.iter().collect::<Vec<_>>(),
            [(0, 1.0, 2.0), (1, 3.0, 4.0)]
        );
        for integrator in [Integrator::Euler, Integrator::Midpoint, Integrator::RK4] {
            let positions = integrator.integrate(
                &view,
                2.0,
                |view| vec![(0.5, -0.25); view.indices.len()],
                |positions, _| vec![(0.5, -0.25); positions.len()],
            );
            assert_eq!(positions, [(2.0, 1.5), (4.0, 3.5)]);
        }
    }

    #[test]
    fn intermediate_surface_queries_keep_their_stage_times() {
        let particles = particles();
        let view = particles.view();
        for (integrator, expected_times, displacement) in [
            (Integrator::Euler, vec![], 0.0),
            (Integrator::Midpoint, vec![1.0], 2.0),
            (Integrator::RK4, vec![1.0, 1.0, 2.0], 2.0),
        ] {
            let times = RefCell::new(Vec::new());
            let positions = integrator.integrate(
                &view,
                2.0,
                |view| vec![(0.0, -1.0); view.indices.len()],
                |positions, elapsed| {
                    times.borrow_mut().push(elapsed);
                    vec![(elapsed, -1.0); positions.len()]
                },
            );
            assert_eq!(*times.borrow(), expected_times);
            assert_eq!(
                positions,
                [(1.0 + displacement, 0.0), (3.0 + displacement, 2.0)]
            );
        }
    }
}

#[cfg(test)]
mod component_tests {
    use super::*;

    #[test]
    fn dense_stages_preserve_scalar_arithmetic_and_odd_tails() {
        for count in [0, 1, 3, 4, 7, 17, 257] {
            for scheme in [Integrator::Euler, Integrator::Midpoint, Integrator::RK4] {
                let mut start = VectorBuffer::default();
                start.extend(
                    (0..count).map(|i| (3.0 + i as f32 * 0.0003, 59.0 - i as f32 * 0.0002)),
                );
                let velocity = |p: (f32, f32), t: f32| {
                    (
                        p.0 * 0.00001 + t * 0.00000001,
                        p.1 * (-0.000001) - t * 0.00000001,
                    )
                };
                let mut k1 = VectorBuffer::default();
                k1.extend(start.as_coordinates().iter().map(|p| velocity(p, 0.0)));
                let mut result = VectorBuffer::default();
                let mut scratch = IntegrationBuffers::default();
                let dt = 900.0;
                scheme.integrate_soa_into(
                    start.as_coordinates(),
                    dt,
                    k1.as_coordinates(),
                    |p, t, out| {
                        out.clear();
                        out.extend(p.iter().map(|p| velocity(p, t)));
                    },
                    &mut scratch,
                    &mut result,
                );
                for (i, (x, y)) in start.as_coordinates().iter().enumerate() {
                    let k1 = velocity((x, y), 0.0);
                    let k2 = velocity((x + 0.5 * dt * k1.0, y + 0.5 * dt * k1.1), 0.5 * dt);
                    let k3 = velocity((x + 0.5 * dt * k2.0, y + 0.5 * dt * k2.1), 0.5 * dt);
                    let k4 = velocity((x + dt * k3.0, y + dt * k3.1), dt);
                    let expected = match scheme {
                        Integrator::Euler => (x + dt * k1.0, y + dt * k1.1),
                        Integrator::Midpoint => (x + dt * k2.0, y + dt * k2.1),
                        Integrator::RK4 => (
                            x + dt * (k1.0 + 2.0 * k2.0 + 2.0 * k3.0 + k4.0) / 6.0,
                            y + dt * (k1.1 + 2.0 * k2.1 + 2.0 * k3.1 + k4.1) / 6.0,
                        ),
                    };
                    assert_eq!(result.x[i].to_bits(), expected.0.to_bits());
                    assert_eq!(result.y[i].to_bits(), expected.1.to_bits());
                }
            }
        }
    }
}
