// cache.rs defines tile identifiers and cached field data

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub struct TileKey {
    pub lon_idx: usize,
    pub lat_idx: usize,
    pub day: usize,
}

/// parsed tile data
/// x and y ocean current in m/s (u, v)
/// x and y surface wind in m/s (wind_u, wind_v)
/// sea surface temperature in K (sst_k)
/// usize fields are tile header data
/// see daily_cron.py at project root for more info
pub struct TileData {
    pub current_origin: Option<(f32, f32)>,
    pub wind_origin: Option<(f32, f32)>,
    pub u: Vec<f32>,
    pub v: Vec<f32>,
    pub wind_u: Vec<f32>,
    pub wind_v: Vec<f32>,
    pub sst_k: Vec<f32>,
    pub depths: Vec<f32>,
    pub n_lon: usize,
    pub n_lat: usize,
    pub n_lon_wind: usize,
    pub n_lat_wind: usize,
    pub n_hours: usize,
    pub n_steps: usize,
}
