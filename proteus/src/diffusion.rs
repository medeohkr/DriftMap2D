use rand::prelude::*;
use rand_distr::{Normal, Distribution};

pub struct Diffusion {
    pub horizontal_k: f32,
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

    pub fn apply_diffusion(&mut self, dt_days: f32, lat: &f32) -> (f32, f32) {
        let dt_seconds: f32 = dt_days * 86400.0;
        let sigma = (2.0 * self.horizontal_k * dt_seconds).sqrt();
        let dx = self.normal.sample(&mut self.rng) * sigma;
        let dy = self.normal.sample(&mut self.rng) * sigma;
        
        let meters_per_degree_lat: f32 = 111_000.0;
        let meters_per_degree_lon: f32 = 111_000.0 * (lat).to_radians().cos();
        
        dx / meters_per_degree_lon;
        dy / meters_per_degree_lat;

        (dx, dy)
    }
    
    pub fn set_k(&mut self, k: f32) {
        self.horizontal_k = k;
    }
    
    pub fn get_k(&self) -> f32 {
        self.horizontal_k
    }
}