use super::{meters_per_degree_lat, meters_per_degree_lon};
use rand::{rngs::ThreadRng, thread_rng};
use rand_distr::{Distribution, Normal};
use serde::Deserialize;

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

pub struct ReleaseManager {
    total_particles: usize,
    pub total_mass: f32,
    hours_per_step: f32,
    releases: Vec<Release>,
    particles_released_per_release: Vec<usize>,
    total_released: usize,
    rng: ThreadRng,
}

impl ReleaseManager {
    pub fn new(releases_json: &str, total_particles: usize, steps_per_day: u32) -> Self {
        let releases: Vec<Release> =
            serde_json::from_str(releases_json).expect("invalid JSON!");

        let total_mass: f32 = releases
            .iter()
            .flat_map(|release| release.schedule.iter())
            .map(|interval| interval.amount)
            .sum();

        let hours_per_step = 24.0 / steps_per_day as f32;
        let num_releases = releases.len();

        Self {
            total_particles,
            total_mass,
            hours_per_step,
            releases,
            particles_released_per_release: vec![0; num_releases],
            total_released: 0,
            rng: thread_rng(),
        }
    }

    pub fn update(&mut self, step_count: u32) -> Vec<ParticleSeed> {
        let hours = self.hours_per_step * step_count as f32;

        let targets = self.compute_targets(hours);

        let mut seeds = Vec::new();

        for (i, (release, &target)) in
            self.releases.iter().zip(&targets).enumerate()
        {
            let to_release = target.saturating_sub(self.particles_released_per_release[i]);

            if to_release == 0 {
                continue;
            }

            let normal = Normal::new(0.0, release.radius)
                .expect("invalid radius for normal distribution");

            let release_seeds = seed(
                to_release,
                release,
                normal,
                &mut self.rng,
                self.total_mass / self.total_particles as f32,
            );

            seeds.extend(release_seeds);
            self.particles_released_per_release[i] = target;
        }

        self.total_released = targets.iter().sum();
        seeds
    }

    fn compute_targets(&self, hours: f32) -> Vec<usize> {
        let fractions: Vec<f32> = self
            .releases
            .iter()
            .map(|release| {
                let mass = mass_released_by_time(release, hours);
                mass / self.total_mass * self.total_particles as f32
            })
            .collect();

        let floors: Vec<usize> = fractions.iter().map(|&f| f.floor() as usize).collect();
        let remainders: Vec<f32> = fractions
            .iter()
            .zip(&floors)
            .map(|(&f, &fl)| f - fl as f32)
            .collect();

        let total_floor: usize = floors.iter().sum();

        let mut to_distribute = self.total_particles.saturating_sub(total_floor);

        let mut indices: Vec<usize> = (0..self.releases.len()).collect();
        indices.sort_by(|&a, &b| {
            remainders[b]
                .partial_cmp(&remainders[a])
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut targets = floors;
        for &i in &indices {
            if to_distribute == 0 {
                break;
            }
            targets[i] += 1;
            to_distribute -= 1;
        }

        targets
    }

    pub fn initial_mass_per_particle(&self) -> f32 {
        self.total_mass / self.total_particles as f32
    }
}

fn mass_released_by_time(release: &Release, hours: f32) -> f32 {
    let mut cumulative = 0.0;
    let mut elapsed = 0.0;

    for interval in &release.schedule {
        if hours <= elapsed {
            break;
        }

        let time_in_interval = (hours - elapsed).min(interval.duration);

        if interval.duration > 0.0 {
            cumulative += interval.amount * time_in_interval / interval.duration;
        } else {
            cumulative += interval.amount;
        }

        elapsed += interval.duration;
    }

    cumulative
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