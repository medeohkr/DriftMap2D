use super::ParticleView;

macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into())
    }
}

pub enum Integrator {
    Euler,
    Midpoint,
    RK4
}

impl Integrator {
    pub fn integrate(
        &self,
        view: &ParticleView,
        dt: f32,
        get_velocities_view: impl Fn(&ParticleView) -> Vec<(f32, f32)>,
        get_velocities_slice: impl Fn(&[(f32, f32, f32)]) -> Vec<(f32, f32)>,
    ) -> Vec<(f32, f32, f32)> {
        match self {
            Integrator::Euler => euler_step(view, dt, get_velocities_view, get_velocities_slice),
            Integrator::Midpoint => midpoint_step(view, dt, get_velocities_view, get_velocities_slice),
            Integrator::RK4 => rk4_step(view, dt, get_velocities_view, get_velocities_slice),
        }
    }
}

pub fn euler_step(
    view: &ParticleView,
    dt: f32,
    get_velocities_view: impl Fn(&ParticleView) -> Vec<(f32, f32)>,
    get_velocities_slice: impl Fn(&[(f32, f32, f32)]) -> Vec<(f32, f32)>,
) -> Vec<(f32, f32, f32)> {
    let velocities = get_velocities_view(view);
    view.iter()
        .map(|(i, lon, lat, depth)| {
            (
                lon + dt * velocities[i].0,
                lat + dt * velocities[i].1,
                depth,
            )
        })
        .collect()
}

pub fn midpoint_step(
    view: &ParticleView,
    dt: f32,
    get_velocities_view: impl Fn(&ParticleView) -> Vec<(f32, f32)>,
    get_velocities_slice: impl Fn(&[(f32, f32, f32)]) -> Vec<(f32, f32)>,
) -> Vec<(f32, f32, f32)> {
    let k1 = get_velocities_view(view);
    let k2 = get_velocities_slice(&compute_intermediate_positions(view, 0.5 * dt, &k1));

    view.iter()
        .map(|(i, lon, lat, depth)| {
            (
                lon + dt * k2[i].0,
                lat + dt * k2[i].1,
                depth,
            )
        })
        .collect()
}

pub fn rk4_step(
    view: &ParticleView,
    dt: f32,
    get_velocities_view: impl Fn(&ParticleView) -> Vec<(f32, f32)>,
    get_velocities_slice: impl Fn(&[(f32, f32, f32)]) -> Vec<(f32, f32)>,
) -> Vec<(f32, f32, f32)> {
    let k1 = get_velocities_view(view);
    let k2 = get_velocities_slice(&compute_intermediate_positions(view, 0.5 * dt, &k1));
    let k3 = get_velocities_slice(&compute_intermediate_positions(view, 0.5 * dt, &k2));
    let k4 = get_velocities_slice(&compute_intermediate_positions(view, dt, &k3));

    view.iter()
        .map(|(i, lon, lat, depth)| {
            (
                lon + dt * (k1[i].0 + 2.0 * k2[i].0 + 2.0 * k3[i].0 + k4[i].0) / 6.0,
                lat + dt * (k1[i].1 + 2.0 * k2[i].1 + 2.0 * k3[i].1 + k4[i].1) / 6.0,
                depth,
            )
        })
        .collect()
}

fn compute_intermediate_positions(
    view: &ParticleView,
    step: f32,
    velocities: &Vec<(f32, f32)>,
) -> Vec<(f32, f32, f32)> {
    view.iter()
        .map(|(i, lon, lat, depth)| {
            (
                lon + step * velocities[i].0,
                lat + step * velocities[i].1,
                depth,
            )
        })
        .collect()
}
