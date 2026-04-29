use rand::prelude::*;
use rand_distr::{Normal, Distribution};

pub struct Diffusion {
    horizontal_k: f32,
    normal: Normal<f32>,
    rng: ThreadRng,
}

impl Diffusion {
    pub fn new(k_value: f32) -> Self {
        let normal = Normal::new(0.0, 1.0).unwrap();
        let rng = thread_rng();

        Self {
            horizontal_k: k_value,
            normal,
            rng,
        }
    }

    pub fn apply_diffusion(&mut self, lon: &mut f32, lat: &mut f32, dt_days: f32) {
        if self.horizontal_k <= 0.0 {
            return;
        }
        let dt_seconds: f32 = dt_days * 86400.0;
        let sigma = (2.0 * self.horizontal_k * dt_seconds).sqrt();
        let dx_m = self.normal.sample(&mut self.rng) * sigma;
        let dy_m = self.normal.sample(&mut self.rng) * sigma;
        
        let meters_per_degree_lat = 111_000.0;
        let meters_per_degree_lon = 111_000.0 * (*lat).to_radians().cos();
        
        *lon += dx_m / meters_per_degree_lon;
        *lat += dy_m / meters_per_degree_lat;
    }
    
    pub fn set_k(&mut self, k: f32) {
        self.horizontal_k = k;
    }
    
    pub fn get_k(&self) -> f32 {
        self.horizontal_k
    }
}