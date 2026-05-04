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
    
    /// Remove a particle by index (swap_remove for O(1)).
    /// Swaps with the last particle and pops all arrays.
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
    
    /// Remove all particles that satisfy a predicate.
    /// Returns the number of removed particles.
    pub fn retain(&mut self, mut predicate: impl FnMut(usize, &Self) -> bool) -> usize {
        let mut removed = 0;
        let mut i = 0;
        
        while i < self.len {
            if !predicate(i, self) {
                self.remove_particle(i);
                removed += 1;
                // Don't increment i — the next particle moved into this index
            } else {
                i += 1;
            }
        }
        
        removed
    }
    
    /// Clear all particles (reset to empty).
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
    
    // ========== ACCESSORS ==========
    
    /// Get a slice of all x positions.
    pub fn x_slice(&self) -> &[f32] {
        &self.x
    }
    
    /// Get a mutable slice of all x positions.
    pub fn x_slice_mut(&mut self) -> &mut [f32] {
        &mut self.x
    }
    
    /// Get a slice of all y positions.
    pub fn y_slice(&self) -> &[f32] {
        &self.y
    }
    
    /// Get a mutable slice of all y positions.
    pub fn y_slice_mut(&mut self) -> &mut [f32] {
        &mut self.y
    }
    
    /// Get a slice of all active flags.
    pub fn active_slice(&self) -> &[bool] {
        &self.active
    }
    
    /// Get a mutable slice of all active flags.
    pub fn active_slice_mut(&mut self) -> &mut [bool] {
        &mut self.active
    }
    
    // ========== BATCH OPERATIONS ==========
    
    /// Update all x positions by adding a delta array.
    /// Assumes delta_x has same length as self.x.
    pub fn add_to_x(&mut self, delta_x: &[f32]) {
        for i in 0..self.len {
            self.x[i] += delta_x[i];
        }
    }
    
    /// Update all y positions by adding a delta array.
    pub fn add_to_y(&mut self, delta_y: &[f32]) {
        for i in 0..self.len {
            self.y[i] += delta_y[i];
        }
    }
    
    /// Scale all concentrations by a factor (e.g., decay).
    pub fn scale_concentration(&mut self, factor: f32) {
        for i in 0..self.len {
            self.concentration[i] *= factor;
        }
    }
    
    /// Apply a function to all active particles.
    /// Skips inactive particles efficiently.
    pub fn for_active(&mut self, mut f: impl FnMut(&mut Particles, usize)) {
        let mut i = 0;
        while i < self.len {
            if self.active[i] {
                f(self, i);
            }
            i += 1;
        }
    }
    
    // ========== UTILITIES ==========
    
    /// Count number of active particles.
    pub fn active_count(&self) -> usize {
        self.active.iter().filter(|&&a| a).count()
    }
    
    /// Get bounding box of active particles in projected coordinates.
    /// Returns (xmin, xmax, ymin, ymax).

    pub fn bounding_box(&self) -> (f32, f32, f32, f32) {
        let mut xmin = f32::MAX;
        let mut xmax = f32::MIN;
        let mut ymin = f32::MAX;
        let mut ymax = f32::MIN;
        
        for i in 0..self.len {
            if self.active[i] {
                xmin = xmin.min(self.x[i]);
                xmax = xmax.max(self.x[i]);
                ymin = ymin.min(self.y[i]);
                ymax = ymax.max(self.y[i]);
            }
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
            if self.active[i] {
                xmin = xmin.min(self.x[i]);
                xmax = xmax.max(self.x[i]);
                ymin = ymin.min(self.y[i]);
                ymax = ymax.max(self.y[i]);
            }
        }
        bounding_box.push(xmin);
        bounding_box.push(xmax);
        bounding_box.push(ymin);
        bounding_box.push(ymax);
        
        bounding_box
    }
    
    /// Get bounding box in geographic coordinates (lon/lat).
    pub fn geographic_bounding_box(
        &self,
        ref_lon: f32,
        ref_lat: f32,
        lon_scale: f32,
        lat_scale: f32,
    ) -> (f32, f32, f32, f32) {
        let (xmin, xmax, ymin, ymax) = self.bounding_box();
        
        let lon_min = ref_lon + xmin / lon_scale;
        let lon_max = ref_lon + xmax / lon_scale;
        let lat_min = ref_lat + ymin / lat_scale;
        let lat_max = ref_lat + ymax / lat_scale;
        
        (lon_min, lon_max, lat_min, lat_max)
    }
    
    /// Check if the particle set is empty (no active particles).
    pub fn is_empty(&self) -> bool {
        self.active_count() == 0
    }
    
    /// Ensure capacity for additional particles (pre-allocate).
    pub fn reserve(&mut self, additional: usize) {
        self.x.reserve(additional);
        self.y.reserve(additional);
        self.depth.reserve(additional);
        self.concentration.reserve(additional);
        self.mass.reserve(additional);
        self.age.reserve(additional);
        self.active.reserve(additional);
    }
}