// cache.rs groups decoded global currents and atmospheric fields

use crate::forcing::GridData;

pub struct Tile {
    pub currents: GridData<2>,
    pub atmosphere: Option<GridData<3>>,
    pub current_origin: Option<(f32, f32)>,
    pub wind_origin: Option<(f32, f32)>,
}
