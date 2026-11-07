// simulation.rs
use super::integrators::IntegrationBuffers;
use super::{
    meters_per_degree_lat, meters_per_degree_lon, normalize_lon, Diffusion, Integrator,
    LandMaskLoader, Particles, ReleaseManager,
};
use crate::arrays::{Coordinates, FieldBuffers, VectorBuffer};
use crate::forcing::{Forcing, ForcingFields};
use crate::tracers::{GenericTracer, LeewayTracer, OilTracer, Tracer, TracerKind};
use crate::TileFetcher;

pub struct Simulation {
    pub particles: Particles,
    pub release_manager: ReleaseManager,
    diffusion: Diffusion,
    pub total_particles: usize,
    pub integrator: Integrator,
    buffers: StepBuffers,
}

impl Simulation {
    pub fn new(
        tracer_type: &str,
        tracer_json: &str,
        releases_json: &str,
        total_particles: usize,
        step_count: u32,
        advection_scheme: &str,
        diffusion_scheme: &str,
        diffusion_coeffs: Vec<f32>,
    ) -> Self {
        let release_manager = ReleaseManager::new(releases_json, total_particles, step_count);
        let tracer = match tracer_type {
            "generic" => TracerKind::Generic(GenericTracer::new(
                tracer_json,
                release_manager.initial_mass_per_particle(),
            )),

            "oil" => TracerKind::Oil(OilTracer::new(
                tracer_json,
                total_particles,
                release_manager.initial_mass_per_particle(),
            )),

            "sar" => TracerKind::Leeway(LeewayTracer::new(tracer_json, total_particles)),

            _ => TracerKind::Generic(GenericTracer::new(
                tracer_json,
                release_manager.initial_mass_per_particle(),
            )),
        };
        let particles = Particles::new(total_particles, tracer);
        let integrator = match advection_scheme {
            "euler" => Integrator::Euler,
            "rk2" => Integrator::Midpoint,
            "rk4" => Integrator::RK4,
            _ => Integrator::Euler,
        };
        let diffusion = Diffusion::new(diffusion_scheme, diffusion_coeffs);

        Self {
            particles,
            release_manager,
            diffusion,
            total_particles,
            integrator,
            buffers: StepBuffers::default(),
        }
    }

    pub fn release_particles(&mut self, step_count: u32) {
        let seeds = self.release_manager.update(step_count);
        for seed in seeds {
            self.particles.add_particle(seed.lon, seed.lat);
        }
    }

    fn calculate_total_velocity(
        tracer: &TracerKind,
        index: usize,
        lat: f32,
        current_u: f32,
        current_v: f32,
        wind_u: f32,
        wind_v: f32,
    ) -> (f32, f32) {
        let windage = tracer.windage(index, lat, wind_u, wind_v);

        (
            current_u + meters_per_degree_lon(windage.0, lat),
            current_v + meters_per_degree_lat(windage.1, lat),
        )
    }

    pub fn update_particles_batch<F: TileFetcher>(
        &mut self,
        dt_days: f32,
        forcing: &impl Forcing,
        current_day: usize,
        hour: f32,
        landmask: &LandMaskLoader<F>,
    ) {
        let dt = dt_days * 86400.0;

        let StepBuffers {
            indices,
            environment,
            stage_environment,
            wind_speeds,
            sst_celsius,
            initial_velocities,
            starting_positions,
            positions,
            integration,
        } = &mut self.buffers;
        indices.clear();
        indices.extend((0..self.particles.len).filter(|&i| !self.particles.stranded[i]));
        if indices.is_empty() {
            return;
        }
        let oil = matches!(self.particles.tracer, TracerKind::Oil(_));
        let generic = matches!(self.particles.tracer, TracerKind::Generic(_));
        starting_positions.clear();
        starting_positions.extend(
            indices
                .iter()
                .map(|&i| (self.particles.lons[i], self.particles.lats[i])),
        );
        forcing.sample_soa_into(
            starting_positions.as_coordinates(),
            current_day,
            hour,
            if oil {
                ForcingFields::All
            } else {
                ForcingFields::CurrentsWind
            },
            environment,
        );
        wind_speeds.clear();
        sst_celsius.clear();
        if !generic {
            wind_speeds.extend(
                environment.components[2]
                    .iter()
                    .zip(&environment.components[3])
                    .map(|(&u, &v)| (u * u + v * v).sqrt()),
            );
        }
        if oil {
            sst_celsius.extend(environment.components[4].iter().map(|v| v - 273.15));
        }
        self.particles
            .tracer
            .step(indices, wind_speeds, sst_celsius, dt);
        // retain the second wind retry and tracer update order used by the original path
        if environment.components[2]
            .iter()
            .chain(&environment.components[3])
            .any(|v| !v.is_finite())
        {
            forcing.sample_soa_into(
                starting_positions.as_coordinates(),
                current_day,
                hour,
                ForcingFields::Wind,
                stage_environment,
            );
            environment.components[2].copy_from_slice(&stage_environment.components[2]);
            environment.components[3].copy_from_slice(&stage_environment.components[3]);
        }
        let tracer = &self.particles.tracer;
        total_velocities(
            tracer,
            indices,
            starting_positions.as_coordinates(),
            environment,
            initial_velocities,
        );
        self.integrator.integrate_soa_into(
            starting_positions.as_coordinates(),
            dt,
            initial_velocities.as_coordinates(),
            |positions, elapsed, velocities| {
                forcing.sample_soa_into(
                    positions,
                    current_day,
                    hour + elapsed / 3600.0,
                    ForcingFields::CurrentsWind,
                    stage_environment,
                );
                total_velocities(tracer, indices, positions, stage_environment, velocities);
            },
            integration,
            positions,
        );
        self.diffusion.diffusion_soa_in_place(
            forcing,
            starting_positions.as_coordinates(),
            positions,
            current_day,
            dt_days,
            hour,
        );

        for (i, &idx) in indices.iter().enumerate() {
            let (mut lon, mut lat) = (positions.x[i], positions.y[i]);

            lon = normalize_lon(lon);
            lat = lat.clamp(-80.0, 90.0);

            if landmask.is_on_land(lon, lat) {
                let position = landmask.shoreline_position(
                    (self.particles.lons[idx], self.particles.lats[idx]),
                    (lon, lat),
                );
                self.particles.lons[idx] = position.0;
                self.particles.lats[idx] = position.1;
                self.particles.stranded[idx] = true;
            } else {
                self.particles.lons[idx] = lon;
                self.particles.lats[idx] = lat;
            }
        }
    }
    pub fn get_particles(&self) -> &Particles {
        &self.particles
    }
}

fn total_velocities(
    tracer: &TracerKind,
    indices: &[usize],
    positions: Coordinates<'_>,
    environment: &FieldBuffers,
    output: &mut VectorBuffer,
) {
    output.resize(indices.len());
    let [u, v, wind_u, wind_v, _] = &environment.components;
    for i in 0..indices.len() {
        let velocity = Simulation::calculate_total_velocity(
            tracer,
            indices[i],
            positions.y[i],
            u[i],
            v[i],
            wind_u[i],
            wind_v[i],
        );
        output.x[i] = velocity.0;
        output.y[i] = velocity.1;
    }
}

#[derive(Default)]
struct StepBuffers {
    indices: Vec<usize>,
    environment: FieldBuffers,
    stage_environment: FieldBuffers,
    wind_speeds: Vec<f32>,
    sst_celsius: Vec<f32>,
    initial_velocities: VectorBuffer,
    starting_positions: VectorBuffer,
    positions: VectorBuffer,
    integration: IntegrationBuffers,
}

#[cfg(test)]
mod tests;
