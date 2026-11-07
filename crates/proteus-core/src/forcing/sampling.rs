// sampling.rs retains grouping and interpolation storage between queries

use super::{
    interpolation::{GridSample, InterpolationScratch},
    TileKey,
};
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct SamplingScratch<P = (usize, f32, f32)> {
    pub groups: HashMap<TileKey, Vec<P>>,
    pub samples: Vec<GridSample>,
    pub values: Vec<f32>,
    pub interpolation: InterpolationScratch,
}

impl<P> SamplingScratch<P> {
    pub fn clear_groups(&mut self) {
        for group in self.groups.values_mut() {
            group.clear();
        }
    }
}
