use crate::tracers::{TracerData};

pub struct Particles {
    // positions
    pub x: Vec<f32>,
    pub y: Vec<f32>,
    pub depth: Vec<f32>,
    
    // tracer-specific 
    pub tracer_data: Vec<TracerData>,
    pub concentration: Vec<f32>,

    // state
    pub stranded: Vec<bool>,
    
    // metadata
    pub len: usize,
    pub capacity: usize,
}

impl Particles {
    // particle vector allocation
    pub fn new(capacity: usize) -> Self {
        Self {
            x: Vec::with_capacity(capacity),
            y: Vec::with_capacity(capacity),
            depth: Vec::with_capacity(capacity),
            tracer_data: Vec::with_capacity(capacity),
            concentration: Vec::with_capacity(capacity),
            stranded: Vec::with_capacity(capacity),
            len: 0,
            capacity,
        }
    }
    
    pub fn stranded_count(&self) -> usize {
        self.stranded.iter().filter(|&&a| a).count()
    }

    //needed in array for wasm
    pub fn bounding_box_array(&self) -> Vec<f32> {
        let mut xmin = f32::MAX;
        let mut xmax = f32::MIN;
        let mut ymin = f32::MAX;
        let mut ymax = f32::MIN;
        
        for i in 0..self.len {
            if !self.stranded[i] {
                xmin = xmin.min(self.x[i]);
                xmax = xmax.max(self.x[i]);
                ymin = ymin.min(self.y[i]);
                ymax = ymax.max(self.y[i]);
            }
        }
        
        vec![xmin, xmax, ymin, ymax]
    }
}