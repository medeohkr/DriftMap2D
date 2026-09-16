use super::{meters_per_degree_lat, meters_per_degree_lon};
use rand::{rngs::ThreadRng, thread_rng};
use rand_distr::{Distribution, Normal};
use serde::Deserialize;

macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into())
    }
}

const EPSILON: f32 = 1e-3;

pub struct ReleaseManager {
    total_particles: usize,
    pub total_mass: f32,
    total_released: usize,
    accumulated_fraction: f32,
    hours_per_step: f32,
    releases: Vec<Release>,
    rng: ThreadRng,
}

#[derive(Debug, Clone)]
pub struct ParticleSeed {
    pub lon: f32,
    pub lat: f32,
    pub depth: f32,
    pub mass: f32,
}

#[derive(Debug, Deserialize)]
struct Release {
    lon: f32,
    lat: f32,
    radius: f32,
    schedule: Vec<Schedule>,
}

#[derive(Debug, Deserialize)]

struct Schedule {
    amount: f32,
    duration: f32,
}

impl ReleaseManager {
    pub fn new(releases_json: &str, total_particles: usize, steps_per_day: u32) -> Self {
        let releases: Vec<Release> = serde_json::from_str(releases_json).expect("invalid JSON!");
        let total_mass: f32 = releases
            .iter()
            .flat_map(|release| release.schedule.iter())
            .map(|interval| interval.amount)
            .sum();
        let hours_per_step = 24.0 / steps_per_day as f32;

        Self {
            total_particles,
            total_mass,
            total_released: 0,
            accumulated_fraction: 0.0,
            hours_per_step,
            releases,
            rng: thread_rng(),
        }
    }

    pub fn update(&mut self, step_count: u32) -> Vec<ParticleSeed> {
        let mut seeds = Vec::with_capacity(self.total_particles);
        let hours = self.hours_per_step * step_count as f32;
        for release in self.releases.iter() {
            let hour_index = if hours == 0.0 { 0 } else {
                match release
                .schedule
                .iter()
                .scan(0.0, |acc, interval| {
                    *acc += interval.duration;
                    Some(*acc)
                })
                .position(|cumulative| hours < cumulative)
            {
                Some(index) => index,
                None => continue
            }};
            let amount = release.schedule[hour_index].amount;
            let duration = release.schedule[hour_index].duration;
            let seed_mass = 
                if duration >= self.hours_per_step { self.hours_per_step * amount / duration } else { amount };

            let mut seed_particles = seed_mass * self.total_particles as f32 / self.total_mass;
            
            self.accumulated_fraction += seed_particles - seed_particles.floor();
            if self.accumulated_fraction >= 1.0 - EPSILON {
                seed_particles += 1.0;
                self.accumulated_fraction -= 1.0;
            }
            
            let normal = Normal::new(0.0, release.radius).unwrap();
            let release_seeds = seed(
                seed_particles as usize,
                release,
                normal,
                &mut self.rng,
                self.total_mass / self.total_particles as f32,
            );
            self.total_released += seed_particles as usize;
            seeds.extend(release_seeds);
        }
        seeds
    }

    pub fn initial_mass_per_particle(&self) -> f32 {
        self.total_mass / self.total_particles as f32
    }
}

fn seed(
    count: usize,
    release: &Release,
    normal: Normal<f32>,
    rng: &mut ThreadRng,
    mass_per_particle: f32,
) -> Vec<ParticleSeed> {
    (0..count)
        .map(|_| {
            let mut dx: f32;
            let mut dy: f32;
            loop {
                dx = normal.sample(rng);
                dy = normal.sample(rng);
                let r = (dx * dx + dy * dy).sqrt();
                if r <= release.radius {
                    break;
                }
            }
            let lat = release.lat + meters_per_degree_lat(dy * 1000.0);
            let lon = release.lon + meters_per_degree_lon(dx * 1000.0, lat);

            ParticleSeed {
                lon,
                lat,
                depth: 0.0,
                mass: mass_per_particle,
            }
        })
        .collect()
}