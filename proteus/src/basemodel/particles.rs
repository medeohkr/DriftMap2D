use crate::tracers::{Tracer, TracerKind};

pub struct Particles {
    pub lons: Vec<f32>,
    pub lats: Vec<f32>,
    pub depths: Vec<f32>,
    pub tracer: TracerKind,
    pub stranded: Vec<bool>,
    pub len: usize,
}

impl Particles {
    // particle vector allocation
    pub fn new(capacity: usize, tracer: TracerKind) -> Self {
        Self {
            lons: Vec::with_capacity(capacity),
            lats: Vec::with_capacity(capacity),
            depths: Vec::with_capacity(capacity),
            tracer: tracer,
            stranded: Vec::with_capacity(capacity),
            len: 0,
        }
    }

    pub fn add_particle(
        &mut self,
        lon: f32,
        lat: f32,
        depth: f32,
    ) {
        self.lons.push(lon);
        self.lats.push(lat);
        self.depths.push(depth);
        self.tracer.push();
        self.stranded.push(false);
        self.len += 1;
    }
    
    pub fn stranded_count(&self) -> usize {
        self.stranded.iter().filter(|&&a| a).count()
    }

    // needed in array for wasm
    pub fn bounding_box_array(&self) -> Vec<f32> {
        let mut lon_min = f32::MAX;
        let mut lon_max = f32::MIN;
        let mut lat_min = f32::MAX;
        let mut lat_max = f32::MIN;
        
        for i in 0..self.len {
            if !self.stranded[i] {
                lon_min = lon_min.min(self.lons[i]);
                lon_max = lon_max.max(self.lons[i]);
                lat_min = lat_min.min(self.lats[i]);
                lat_max = lat_max.max(self.lats[i]);
            }
        }
        
        vec![lon_min, lon_max, lat_min, lat_max]
    }
    pub fn view(&self) -> ParticleView {
        let indices: Vec<usize> = (0..self.len).filter(|&i| !self.stranded[i]).collect();

        ParticleView {
            lons: &self.lons,
            lats: &self.lats,
            depths: &self.depths,
            indices
        }
    }
}

pub struct ParticleView<'a> {
    lons: &'a [f32],
    lats: &'a [f32],
    depths: &'a [f32],
    pub indices: Vec<usize>,
}

impl ParticleView<'_> {
    pub fn lon(&self, i: usize) -> f32 {
        self.lons[self.indices[i]]
    }

    pub fn lat(&self, i: usize) -> f32 {
        self.lats[self.indices[i]]
    }

    pub fn depth(&self, i: usize) -> f32 {
        self.depths[self.indices[i]]
    }

    pub fn iter(&self) -> impl Iterator<Item = (usize, f32, f32, f32)> + '_ {
        self.indices.iter().enumerate().map(|(pos, &idx)| {
            (pos, self.lons[idx], self.lats[idx], self.depths[idx])
        })
    }
}
