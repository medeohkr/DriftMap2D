use super::{meters_per_degree_lat, meters_per_degree_lon};
use rand::{rngs::ThreadRng, thread_rng};
use rand_distr::{Distribution, Normal};
use serde::Deserialize;

pub struct ReleaseManager {
    total_particles: usize,
    pub total_mass: f32,
    total_released: usize,
    hours_per_step: f32,
    releases: Vec<Release>,

    /// Number of particles allocated to each release (Hamilton method).
    /// Sum equals total_particles.
    release_allocations: Vec<usize>,

    /// How many particles each release has already emitted.
    /// Used to compute step_particles = target - released.
    released_per_release: Vec<usize>,

    /// Cumulative mass per release (amount × duration over all intervals).
    release_total_amounts: Vec<f32>,

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
        let releases: Vec<Release> =
            serde_json::from_str(releases_json).expect("invalid JSON!");
        let total_mass: f32 = releases
            .iter()
            .flat_map(|r| r.schedule.iter())
            .map(|iv| iv.amount)
            .sum();
        let hours_per_step = 24.0 / steps_per_day as f32;

        let release_total_amounts: Vec<f32> = releases
            .iter()
            .map(|r| r.schedule.iter().map(|iv| iv.amount).sum())
            .collect();

        let raw_shares: Vec<f32> = release_total_amounts
            .iter()
            .map(|&m| m * total_particles as f32 / total_mass.max(1e-12))
            .collect();
        let bases: Vec<usize> = raw_shares.iter().map(|s| s.floor() as usize).collect();
        let base_sum: usize = bases.iter().sum();
        let remaining = total_particles.saturating_sub(base_sum);

        let mut order: Vec<usize> = (0..releases.len()).collect();
        order.sort_by(|&a, &b| {
            let fa = raw_shares[a] - raw_shares[a].floor();
            let fb = raw_shares[b] - raw_shares[b].floor();
            fb.partial_cmp(&fa).unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut release_allocations = bases;
        for &idx in order.iter().take(remaining) {
            release_allocations[idx] += 1;
        }

        let n = releases.len();
        Self {
            total_particles,
            total_mass,
            total_released: 0,
            hours_per_step,
            releases,
            release_allocations,
            released_per_release: vec![0; n],
            release_total_amounts,
            rng: thread_rng(),
        }
    }

    fn cumulative_mass(&self, release: &Release, hours: f32) -> f32 {
        let mut cumulative = 0.0;
        let mut elapsed = 0.0;
        for interval in release.schedule.iter() {
            if hours < elapsed {
                break;
            }
            if interval.duration <= 0.0 {
                if hours >= elapsed {
                    cumulative += interval.amount;
                }
            } else {
                let covered = (hours - elapsed).min(interval.duration).max(0.0);
                cumulative += interval.amount * (covered / interval.duration);
            }
            elapsed += interval.duration;
        }
        cumulative
    }

    pub fn update(&mut self, step_count: u32) -> Vec<ParticleSeed> {
        let mut seeds = Vec::with_capacity(self.total_particles);
        let hours = self.hours_per_step * (step_count + 1) as f32;

        for (i, release) in self.releases.iter().enumerate() {
            if self.total_released >= self.total_particles {
                break;
            }
            if self.released_per_release[i] >= self.release_allocations[i] {
                continue;
            }

            let cumulative_amount = self.cumulative_mass(release, hours);
            if cumulative_amount <= 0.0 {
                continue;
            }
            let release_total = self.release_total_amounts[i];
            if release_total <= 0.0 {
                continue;
            }

            let target_float = self.release_allocations[i] as f32
                * (cumulative_amount / release_total);
            let target_int = (target_float.floor() as usize)
                .min(self.release_allocations[i])
                .min(self.total_particles - self.total_released + self.released_per_release[i]);

            let step_particles = target_int.saturating_sub(self.released_per_release[i]);
            if step_particles == 0 {
                continue;
            }

            let mass_per_particle = release_total / self.release_allocations[i] as f32;

            let normal = Normal::new(0.0, release.radius).unwrap();
            let release_seeds = seed(
                step_particles,
                release,
                normal,
                &mut self.rng,
                mass_per_particle,
            );

            self.released_per_release[i] = target_int;
            self.total_released += step_particles;
            seeds.extend(release_seeds);
        }
        seeds
    }

    pub fn initial_mass_per_particle(&self) -> f32 {
        self.total_mass / self.total_particles.max(1) as f32
    }

    pub fn total_released(&self) -> usize {
        self.total_released
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
            let dx = normal.sample(rng);
            let dy = normal.sample(rng);

            let lat = release.lat + meters_per_degree_lat(dy * 1000.0, release.lat);
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