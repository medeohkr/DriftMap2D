use super::{meters_per_degree_lat, meters_per_degree_lon};
use rand::{rngs::ThreadRng, thread_rng};
use rand_distr::{Distribution, Normal};
use serde::Deserialize;

const EPSILON: f32 = 1e-6;

pub struct ReleaseManager {
    total_particles: usize,
    pub total_mass: f32,
    total_released: usize,
    accumulated_fraction: f32,
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
    pub fn new(releases_json: &str, total_particles: usize) -> Self {
        let releases: Vec<Release> = serde_json::from_str(releases_json).expect("invalid JSON!");
        let total_mass: f32 = releases
            .iter()
            .flat_map(|release| release.schedule.iter())
            .map(|interval| interval.amount)
            .sum();

        Self {
            total_particles,
            total_mass,
            total_released: 0,
            releases,
            accumulated_fraction: 0.0,
            rng: thread_rng(),
        }
    }

    pub fn update(&mut self, hours_since_start: f32, dt_hours: f32) -> Vec<ParticleSeed> {
        let mut seeds = Vec::with_capacity(self.total_particles);
        for release in self.releases.iter() {
            let hour_index = match release
                .schedule
                .iter()
                .scan(0.0, |acc, interval| {
                    *acc += interval.duration;
                    Some(*acc)
                })
                .position(|hours| hours_since_start < hours)
            {
                Some(index) => index,
                None => continue
            };

            let amount = release.schedule[hour_index].amount;
            let duration = release.schedule[hour_index].duration;
            let rate = if duration > 0.0 { amount / duration } else { amount };

            let seed_mass = rate * dt_hours;
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
            let lat = release.lat + meters_per_degree_lat(dy);
            let lon = release.lon + meters_per_degree_lon(dx, lat);

            ParticleSeed {
                lon,
                lat,
                depth: 0.0,
                mass: mass_per_particle,
            }
        })
        .collect()
}
