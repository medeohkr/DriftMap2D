// arrays.rs stores compact coordinate and field components for dense numerical loops

/// borrowed components in stable active-particle order
#[derive(Clone, Copy)]
pub struct Coordinates<'a> {
    pub x: &'a [f32],
    pub y: &'a [f32],
}

impl<'a> Coordinates<'a> {
    pub fn new(x: &'a [f32], y: &'a [f32]) -> Self {
        assert_eq!(x.len(), y.len());
        Self { x, y }
    }
    pub fn len(self) -> usize {
        self.x.len()
    }
    pub fn is_empty(self) -> bool {
        self.x.is_empty()
    }
    pub fn iter(self) -> impl ExactSizeIterator<Item = (f32, f32)> + 'a {
        self.x.iter().copied().zip(self.y.iter().copied())
    }
}

/// retained component buffers used for coordinates, velocities, and displacements
#[derive(Default)]
pub struct VectorBuffer {
    pub x: Vec<f32>,
    pub y: Vec<f32>,
}

impl VectorBuffer {
    pub fn as_coordinates(&self) -> Coordinates<'_> {
        Coordinates::new(&self.x, &self.y)
    }
    pub fn clear(&mut self) {
        self.x.clear();
        self.y.clear();
    }
    pub fn resize(&mut self, count: usize) {
        self.x.resize(count, 0.0);
        self.y.resize(count, 0.0);
    }
    pub fn len(&self) -> usize {
        self.x.len()
    }
    pub fn extend(&mut self, positions: impl IntoIterator<Item = (f32, f32)>) {
        for (x, y) in positions {
            self.x.push(x);
            self.y.push(y);
        }
    }
}

/// current u/v, wind u/v, and kelvin temperature in separate contiguous arrays
#[derive(Default)]
pub struct FieldBuffers {
    pub components: [Vec<f32>; 5],
}

impl FieldBuffers {
    pub fn len(&self) -> usize {
        self.components[0].len()
    }
}

/// statically dispatched output storage keeps legacy exports outside the SoA engine path
pub(crate) trait FieldOutput {
    fn reset(&mut self, count: usize);
    fn get(&self, field: usize, index: usize) -> f32;
    fn set(&mut self, field: usize, index: usize, value: f32);
}

impl FieldOutput for FieldBuffers {
    fn reset(&mut self, count: usize) {
        for values in &mut self.components {
            values.clear();
            values.resize(count, f32::NAN);
        }
    }
    fn get(&self, field: usize, index: usize) -> f32 {
        self.components[field][index]
    }
    fn set(&mut self, field: usize, index: usize, value: f32) {
        self.components[field][index] = value;
    }
}

impl FieldOutput for Vec<[f32; 5]> {
    fn reset(&mut self, count: usize) {
        self.clear();
        self.resize(count, [f32::NAN; 5]);
    }
    fn get(&self, field: usize, index: usize) -> f32 {
        self[index][field]
    }
    fn set(&mut self, field: usize, index: usize, value: f32) {
        self[index][field] = value;
    }
}
