use js_sys::Float32Array;

/// Structure of Arrays (SoA) particle storage for cache-efficient simulation.
/// All fields are public for direct access by integrators and physics modules.
pub struct Particles {
    // Core position fields (hot — accessed every step)
    pub x: Vec<f32>,
    pub y: Vec<f32>,
    pub depth: Vec<f32>,
    
    // Tracer fields (accessed frequently)
    pub concentration: Vec<f32>,
    pub mass: Vec<f32>,
    pub age: Vec<f32>,
    
    // State fields
    pub active: Vec<bool>,
    pub stranded: Vec<bool>,
    pub f_evap: Vec<f32>,
    pub y_w: Vec<f32>,
    pub mu_bulk_cp: Vec<f32>, 
    pub rho_bulk: Vec<f32>,   
    pub mass_components: Vec<Vec<f32>>,  // per particle: mass of each distillation cut
    
    // Metadata
    pub len: usize,
    pub capacity: usize,
}

impl Particles {
    // ========== CONSTRUCTORS ==========
    
    /// Create a new particle set with given capacity.
    /// All vectors are pre-allocated but empty.
    pub fn new(capacity: usize) -> Self {
        Self {
            x: Vec::with_capacity(capacity),
            y: Vec::with_capacity(capacity),
            depth: Vec::with_capacity(capacity),
            concentration: Vec::with_capacity(capacity),
            mass: Vec::with_capacity(capacity),
            age: Vec::with_capacity(capacity),
            active: Vec::with_capacity(capacity),
            stranded: Vec::with_capacity(capacity),
            f_evap: Vec::with_capacity(capacity),
            y_w: Vec::with_capacity(capacity),
            mu_bulk_cp: Vec::with_capacity(capacity),
            rho_bulk: Vec::with_capacity(capacity),
            len: 0,
            capacity,
            mass_components: Vec::with_capacity(capacity),
        }
    }
    
    // ========== PARTICLE MANAGEMENT ==========
    
    /// Add a single particle. Returns its index.
    pub fn add_particle(
        &mut self,
        x: f32,
        y: f32,
        depth: f32,
        concentration: f32,
        mass: f32,
        age: f32,
        active: bool,
        stranded: bool,
        f_evap: f32,
        y_w: f32,
        mu_bulk_cp: f32,
        rho_bulk: f32
    ) -> usize {
        self.x.push(x);
        self.y.push(y);
        self.depth.push(depth);
        self.concentration.push(concentration);
        self.mass.push(mass);
        self.age.push(age);
        self.active.push(active);
        self.stranded.push(stranded);
        self.f_evap.push(f_evap);
        self.y_w.push(y_w);
        self.mu_bulk_cp.push(mu_bulk_cp);
        self.rho_bulk.push(rho_bulk);
        self.len += 1;
        self.len - 1
    }
    
    pub fn remove_particle(&mut self, index: usize) {
        if index >= self.len {
            return;
        }
        
        let last = self.len - 1;
        
        if index != last {
            // Swap with last element
            self.x.swap(index, last);
            self.y.swap(index, last);
            self.depth.swap(index, last);
            self.concentration.swap(index, last);
            self.mass.swap(index, last);
            self.age.swap(index, last);
            self.active.swap(index, last);
            self.stranded.swap(index, last);
            self.f_evap.swap(index, last);
            self.y_w.swap(index, last);
            self.mu_bulk_cp.swap(index, last);
            self.rho_bulk.swap(index, last);
        }
        
        // Pop the last element
        self.x.pop();
        self.y.pop();
        self.depth.pop();
        self.concentration.pop();
        self.mass.pop();
        self.age.pop();
        self.active.pop();
        self.stranded.pop();
        self.f_evap.pop();
        self.y_w.pop();
        self.mu_bulk_cp.pop();
        self.rho_bulk.pop();
        
        self.len -= 1;
    }
    
    pub fn clear(&mut self) {
        self.x.clear();
        self.y.clear();
        self.depth.clear();
        self.concentration.clear();
        self.mass.clear();
        self.age.clear();
        self.active.clear();
        self.stranded.clear();
        self.f_evap.clear();
        self.y_w.clear();
        self.mu_bulk_cp.clear();
        self.rho_bulk.clear();
        self.len = 0;
    }
    
    pub fn inactive_count(&self) -> usize {
        self.active.iter().filter(|&&a| !a).count()
    }

    pub fn stranded_count(&self) -> usize {
        self.stranded.iter().filter(|&&a| a).count()
    }

    pub fn bounding_box_array(&self) -> Vec<f32> {
        let mut xmin = f32::MAX;
        let mut xmax = f32::MIN;
        let mut ymin = f32::MAX;
        let mut ymax = f32::MIN;
        
        for i in 0..self.len {
            if self.active[i] && !self.stranded[i] {
                xmin = xmin.min(self.x[i]);
                xmax = xmax.max(self.x[i]);
                ymin = ymin.min(self.y[i]);
                ymax = ymax.max(self.y[i]);
            }
        }
        
        vec![xmin, xmax, ymin, ymax]
    }
}