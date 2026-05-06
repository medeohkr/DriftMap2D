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
            len: 0,
            capacity,
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
    ) -> usize {
        self.x.push(x);
        self.y.push(y);
        self.depth.push(depth);
        self.concentration.push(concentration);
        self.mass.push(mass);
        self.age.push(age);
        self.active.push(active);
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
        }
        
        // Pop the last element
        self.x.pop();
        self.y.pop();
        self.depth.pop();
        self.concentration.pop();
        self.mass.pop();
        self.age.pop();
        self.active.pop();
        
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
        self.len = 0;
    }

    pub fn active_count(&self) -> usize {
        self.active.iter().filter(|&&a| a).count()
    }
    
    pub fn inactive_count(&self) -> usize {
        self.active.iter().filter(|&&a| !a).count()
    }

    pub fn bounding_box(&self) -> (f32, f32, f32, f32) {
        let mut xmin = f32::MAX;
        let mut xmax = f32::MIN;
        let mut ymin = f32::MAX;
        let mut ymax = f32::MIN;
        
        for i in 0..self.len {
            xmin = xmin.min(self.x[i]);
            xmax = xmax.max(self.x[i]);
            ymin = ymin.min(self.y[i]);
            ymax = ymax.max(self.y[i]);
        }
        
        (xmin, xmax, ymin, ymax)
    }

    pub fn bounding_box_array(&self) -> Vec<f32> {
        let mut xmin = f32::MAX;
        let mut xmax = f32::MIN;
        let mut ymin = f32::MAX;
        let mut ymax = f32::MIN;
        let mut bounding_box = Vec::with_capacity(4);
        
        for i in 0..self.len {
            xmin = xmin.min(self.x[i]);
            xmax = xmax.max(self.x[i]);
            ymin = ymin.min(self.y[i]);
            ymax = ymax.max(self.y[i]);       
        }
        bounding_box.push(xmin);
        bounding_box.push(xmax);
        bounding_box.push(ymin);
        bounding_box.push(ymax);
        
        bounding_box
    }
}