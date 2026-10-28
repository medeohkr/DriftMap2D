use super::ParticleView;

pub enum Integrator {
    Euler,
    Midpoint,
    RK4,
}

impl Integrator {
    pub fn integrate(
        &self,
        view: &ParticleView,
        dt: f32,
        get_velocities_view: impl Fn(&ParticleView) -> Vec<(f32, f32)>,
        get_velocities_slice: impl Fn(&[(f32, f32)], f32) -> Vec<(f32, f32)>,
    ) -> Vec<(f32, f32)> {
        match self {
            Integrator::Euler => euler_step(view, dt, get_velocities_view, get_velocities_slice),
            Integrator::Midpoint => {
                midpoint_step(view, dt, get_velocities_view, get_velocities_slice)
            }
            Integrator::RK4 => rk4_step(view, dt, get_velocities_view, get_velocities_slice),
        }
    }
}

pub fn euler_step(
    view: &ParticleView,
    dt: f32,
    get_velocities_view: impl Fn(&ParticleView) -> Vec<(f32, f32)>,
    _get_velocities_slice: impl Fn(&[(f32, f32)], f32) -> Vec<(f32, f32)>,
) -> Vec<(f32, f32)> {
    let velocities = get_velocities_view(view);
    view.iter()
        .map(|(i, lon, lat)| (lon + dt * velocities[i].0, lat + dt * velocities[i].1))
        .collect()
}

pub fn midpoint_step(
    view: &ParticleView,
    dt: f32,
    get_velocities_view: impl Fn(&ParticleView) -> Vec<(f32, f32)>,
    get_velocities_slice: impl Fn(&[(f32, f32)], f32) -> Vec<(f32, f32)>,
) -> Vec<(f32, f32)> {
    let k1 = get_velocities_view(view);
    let k2 = get_velocities_slice(
        &compute_intermediate_positions(view, 0.5 * dt, &k1),
        0.5 * dt,
    );

    view.iter()
        .map(|(i, lon, lat)| (lon + dt * k2[i].0, lat + dt * k2[i].1))
        .collect()
}

pub fn rk4_step(
    view: &ParticleView,
    dt: f32,
    get_velocities_view: impl Fn(&ParticleView) -> Vec<(f32, f32)>,
    get_velocities_slice: impl Fn(&[(f32, f32)], f32) -> Vec<(f32, f32)>,
) -> Vec<(f32, f32)> {
    let k1 = get_velocities_view(view);
    let k2 = get_velocities_slice(
        &compute_intermediate_positions(view, 0.5 * dt, &k1),
        0.5 * dt,
    );
    let k3 = get_velocities_slice(
        &compute_intermediate_positions(view, 0.5 * dt, &k2),
        0.5 * dt,
    );
    let k4 = get_velocities_slice(&compute_intermediate_positions(view, dt, &k3), dt);

    view.iter()
        .map(|(i, lon, lat)| {
            (
                lon + dt * (k1[i].0 + 2.0 * k2[i].0 + 2.0 * k3[i].0 + k4[i].0) / 6.0,
                lat + dt * (k1[i].1 + 2.0 * k2[i].1 + 2.0 * k3[i].1 + k4[i].1) / 6.0,
            )
        })
        .collect()
}

fn compute_intermediate_positions(
    view: &ParticleView,
    step: f32,
    velocities: &Vec<(f32, f32)>,
) -> Vec<(f32, f32)> {
    view.iter()
        .map(|(i, lon, lat)| (lon + step * velocities[i].0, lat + step * velocities[i].1))
        .collect()
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
