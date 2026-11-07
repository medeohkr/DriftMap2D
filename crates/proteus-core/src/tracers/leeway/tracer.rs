use super::super::Tracer;
use super::{LeewayData, LeewayProperties};
use rand::{Rng, thread_rng};
use rand::rngs::ThreadRng;
use rand_distr::StandardNormal;

pub struct LeewayTracer {
    pub properties: LeewayProperties,
    pub data: LeewayData,
    pub rng: ThreadRng
}

impl Tracer for LeewayTracer {
    fn push(&mut self) {
        self.data.capsized.push(false);
        self.data.orientation.push(self.data.orientation.len() % 2 == 0);
        loop {
            let rdw: f32 = self.rng.sample(StandardNormal);
            let dweps = rdw * self.properties.downwind[2];
            if self.properties.downwind[0] + dweps / 20.0 >= 0.0 {
                self.data.downwind_eps.push(dweps);
                break
            }
        }

        let rcw: f32 = self.rng.sample(StandardNormal);
        self.data.right_eps.push(rcw * self.properties.right[2]);
        self.data.left_eps.push(rcw * self.properties.left[2]);
    }

    fn step(&mut self, indices: &[usize], wind_speeds: &[f32], _sst_celsius: &[f32], dt: f32) {
        let dt_hours = dt / 3600.0;
        let p_jibe = 1.0 - (1.0 - self.properties.jibe_probability).powf(dt_hours);
        
        for (pos, &idx) in indices.iter().enumerate() {
            if self.rng.gen::<f32>() < p_jibe {
                self.data.orientation[idx] = !self.data.orientation[idx];
            }
            
            if !self.properties.capsizing || self.data.capsized[idx] {
                continue;
            }
            
            let wind = wind_speeds[pos];
            let p_hour = 0.5 + 0.5 * ((wind - self.properties.capsize_threshold) / self.properties.capsize_sigma).tanh();
            let p_step = p_hour * dt_hours;
            
            if self.rng.gen::<f32>() < p_step {
                self.data.capsized[idx] = true;
            }
        }
    }

    fn windage(&self, index: usize, _lat: f32, wind_u: f32, wind_v: f32) -> (f32, f32) {
        let wind_speed = (wind_u * wind_u + wind_v * wind_v).sqrt();

        let wind_dir_u = wind_u / wind_speed;
        let wind_dir_v = wind_v / wind_speed;
        
        let cross_dir_u = wind_dir_v;
        let cross_dir_v = -wind_dir_u;
        
        let dweps = self.data.downwind_eps[index];
        let downwind_mag = leeway(
            wind_speed,
            self.properties.downwind[0],
            self.properties.downwind[1],
            dweps,
        );

        let (cwslope, cwoffset, cweps) = if self.data.orientation[index] {
            (self.properties.right[0], self.properties.right[1], self.data.right_eps[index])
        } else {
            (self.properties.left[0], self.properties.left[1], self.data.left_eps[index])
        };
        let crosswind_mag = leeway(wind_speed, cwslope, cwoffset, cweps);
        
        let capsize_factor = if self.data.capsized[index] {
            self.properties.capsize_fraction
        } else {
            1.0
        };
        
        let u = (downwind_mag * wind_dir_u + crosswind_mag * cross_dir_u) * capsize_factor;
        let v = (downwind_mag * wind_dir_v + crosswind_mag * cross_dir_v) * capsize_factor;

        (u, v)
    }
}

impl LeewayTracer {
    pub fn new(leeway_json: &str, capacity: usize) -> Self {
        let json: LeewayProperties = serde_json::from_str(leeway_json).expect("invalid JSON!");

        Self {
            properties: json,
            data: LeewayData {
                capsized: Vec::with_capacity(capacity),
                orientation: Vec::with_capacity(capacity),
                downwind_eps: Vec::with_capacity(capacity),
                right_eps: Vec::with_capacity(capacity),
                left_eps: Vec::with_capacity(capacity)
            },
            rng: thread_rng()
        }
    }
}

fn leeway(wind: f32, slope: f32, offset: f32, eps: f32) -> f32 {
    0.01 * ((slope + eps / 20.0) * wind + offset + eps / 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orientation_alternates_across_release_batches_without_a_setting() {
        let properties = serde_json::json!({
            "downwind": [0.96, 0.0, 12.0],
            "right": [0.54, 0.0, 9.4],
            "left": [-0.54, 0.0, 9.4],
            "jibe_probability": 0.0,
            "capsizing": false,
            "capsize_threshold": 30.0,
            "capsize_fraction": 0.4,
            "capsize_sigma": 5.0,
        });
        let mut first = LeewayTracer::new(&properties.to_string(), 10);
        let mut second = LeewayTracer::new(&properties.to_string(), 10);
        for batch in [3, 2, 5] {
            for _ in 0..batch {
                first.push();
                second.push();
            }
        }
        assert_eq!(first.data.orientation, vec![true, false, true, false, true, false, true, false, true, false]);
        assert_eq!(first.data.orientation, second.data.orientation);
    }
}
