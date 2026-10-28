// cache.rs defines decoded native forcing tile data

use crate::forcing::GridData;

pub(super) struct Tile {
    pub timestamps: Vec<i64>,
    pub grid: GridData<5>,
}
