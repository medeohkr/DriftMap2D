// integrators.rs advances positions with reusable stage buffers

use super::ParticleView;

pub enum Integrator {
    Euler,
    Midpoint,
    RK4,
}

#[derive(Default)]
pub struct IntegrationBuffers {
    intermediate: Vec<(f32, f32)>,
    k2: Vec<(f32, f32)>,
    k3: Vec<(f32, f32)>,
    k4: Vec<(f32, f32)>,
}

impl Integrator {
    /// the initial velocity already includes tracer changes at the start of this step
    pub fn integrate_into(
        &self,
        view: &ParticleView,
        dt: f32,
        k1: &[(f32, f32)],
        mut sample: impl FnMut(&[(f32, f32)], f32, &mut Vec<(f32, f32)>),
        buffers: &mut IntegrationBuffers,
        output: &mut Vec<(f32, f32)>,
    ) {
        output.clear();
        if matches!(self, Self::Euler) {
            output.extend(
                view.iter()
                    .map(|(i, lon, lat)| (lon + dt * k1[i].0, lat + dt * k1[i].1)),
            );
            return;
        }
        intermediate_positions(view, 0.5 * dt, k1, &mut buffers.intermediate);
        sample(&buffers.intermediate, 0.5 * dt, &mut buffers.k2);
        if matches!(self, Self::Midpoint) {
            output.extend(
                view.iter()
                    .map(|(i, lon, lat)| (lon + dt * buffers.k2[i].0, lat + dt * buffers.k2[i].1)),
            );
            return;
        }
        intermediate_positions(view, 0.5 * dt, &buffers.k2, &mut buffers.intermediate);
        sample(&buffers.intermediate, 0.5 * dt, &mut buffers.k3);
        intermediate_positions(view, dt, &buffers.k3, &mut buffers.intermediate);
        sample(&buffers.intermediate, dt, &mut buffers.k4);
        output.extend(view.iter().map(|(i, lon, lat)| {
            (
                lon + dt
                    * (k1[i].0 + 2.0 * buffers.k2[i].0 + 2.0 * buffers.k3[i].0 + buffers.k4[i].0)
                    / 6.0,
                lat + dt
                    * (k1[i].1 + 2.0 * buffers.k2[i].1 + 2.0 * buffers.k3[i].1 + buffers.k4[i].1)
                    / 6.0,
            )
        }));
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
            |positions, elapsed, out| {
                out.clear();
                out.extend(get_velocities_slice(positions, elapsed));
            },
            &mut IntegrationBuffers::default(),
            &mut output,
        );
        output
    }
}

fn intermediate_positions(
    view: &ParticleView,
    step: f32,
    velocities: &[(f32, f32)],
    output: &mut Vec<(f32, f32)>,
) {
    output.clear();
    output.extend(
        view.iter()
            .map(|(i, lon, lat)| (lon + step * velocities[i].0, lat + step * velocities[i].1)),
    );
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
