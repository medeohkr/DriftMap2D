// data_loader.rs
// maps particles to tiles (0.1° buffer for intermediate steps)
// fetches and caches needed tiles from Cloudflare bucket
// groups tiles for cache friendliness
// bilinear + temporal interpolation

use super::{
    bilerp, lerp, meters_per_degree_lat, meters_per_degree_lon, normalize_lon, ParticleView,
};
use crate::fetch::{FetchError, TileFetcher};
use half::f16;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use thiserror::Error;
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

/// generic F allows the same loader to work in WASM
/// (GlooFetcher) and in the CLI (ReqwestFetcher)
pub struct DataLoader<F: TileFetcher> {
    min_lon: f32,
    min_lat: f32,
    step: f32,
    step_wind: f32,
    tile_size: f32,
    base_url: String,
    fetcher: F,

    pub cache: HashMap<TileKey, RefCell<TileData>>,
    pending: HashSet<TileKey>, // ensures tiles mid-download aren't requested
}

#[derive(Error, Debug)]
pub enum LoaderError {
    #[error("Network request failed: {0}")]
    Network(String),
    #[error("Failed to parse tile data: {0}")]
    Parse(String),
    #[error("Tile not found: {0}")]
    NotFound(String),
    #[error("HTTP error: {0}")]
    Http(u16),
}

impl<F: TileFetcher> DataLoader<F> {
    pub fn new(base_url: &str, min_lon: f32, min_lat: f32, fetcher: F) -> Self {
        Self {
            min_lon,
            min_lat,
            step: 1.0 / 12.0,
            step_wind: 1.0 / 4.0,
            tile_size: 10.0,
            base_url: base_url.to_string(),
            fetcher,
            cache: HashMap::new(),
            pending: HashSet::new(),
        }
    }

    /// group positions from a generic iterator, avoiding extra vec allocation
    fn group_positions<I>(
        &self,
        iter: I,
        day: usize,
    ) -> HashMap<TileKey, Vec<(usize, f32, f32, f32)>>
    where
        I: IntoIterator<Item = (usize, f32, f32, f32)>,
    {
        let mut groups: HashMap<TileKey, Vec<(usize, f32, f32, f32)>> = HashMap::new();
        for (i, lon, lat, depth) in iter {
            let lon = normalize_lon(lon);
            let key = self.get_tile_key(lon, lat, day);
            groups
                .entry(key)
                .or_insert_with(Vec::new)
                .push((i, lon, lat, depth));
        }
        groups
    }

    // enumerates positions from slice to build iterator to pass to group_positions
    fn group_positions_slice(
        &self,
        positions: &[(f32, f32, f32)],
        day: usize,
    ) -> HashMap<TileKey, Vec<(usize, f32, f32, f32)>> {
        self.group_positions(
            positions
                .iter()
                .enumerate()
                .map(|(i, &(lon, lat, depth))| (i, lon, lat, depth)),
            day,
        )
    }

    /// Compute the geographic origin (min_lon, min_lat) of a tile.
    fn tile_origin(&self, key: &TileKey) -> (f32, f32) {
        (
            self.min_lon + (key.lon_idx as f32) * self.tile_size,
            self.min_lat + (key.lat_idx as f32) * self.tile_size,
        )
    }

    /// sample each tile as a batch before converting current units
    fn sample_groups(
        &self,
        groups: HashMap<TileKey, Vec<(usize, f32, f32, f32)>>,
        count: usize,
        hour: f32,
        want_current: bool,
        want_wind: bool,
        want_sst: bool,
    ) -> Vec<[f32; 5]> {
        let mut results = vec![[0.0; 5]; count];
        let mut scratch = Vec::new();
        for (key, group) in groups {
            let Some(cached) = self.cache.get(&key) else {
                continue;
            };
            let mut tile = cached.borrow_mut();
            let (min_lon, min_lat) = self.tile_origin(&key);
            let samples = |step: f32, origin: Option<(f32, f32)>| -> Vec<GridSample> {
                let (min_lon, min_lat) = origin.unwrap_or((min_lon, min_lat));
                group
                    .iter()
                    .map(|&(output_index, lon, lat, _)| GridSample {
                        output_index,
                        x: (lon - min_lon) / step,
                        y: (lat - min_lat) / step,
                    })
                    .collect()
            };
            if want_current {
                let samples = samples(self.step, tile.current_origin);
                let (width, height, times, depths) =
                    (tile.n_lon, tile.n_lat, tile.n_hours, tile.depths.len());
                // current queries retain the existing surface layer behavior
                let TileData { u, v, .. } = &mut *tile;
                for (component, data) in [(0, u), (1, v)] {
                    let values = interpolate_time_batch(
                        data,
                        width,
                        height,
                        times,
                        depths,
                        hour,
                        &samples,
                        &mut scratch,
                    );
                    for (sample, value) in samples.iter().zip(values) {
                        results[sample.output_index][component] = value;
                    }
                }
            }
            if want_wind || want_sst {
                let samples = samples(self.step_wind, tile.wind_origin);
                let TileData {
                    wind_u,
                    wind_v,
                    sst_k,
                    n_lon_wind,
                    n_lat_wind,
                    n_steps,
                    ..
                } = &mut *tile;
                for (component, data, requested) in [
                    (2, wind_u, want_wind),
                    (3, wind_v, want_wind),
                    (4, sst_k, want_sst),
                ] {
                    if !requested {
                        continue;
                    }
                    let values = interpolate_time_batch(
                        data,
                        *n_lon_wind,
                        *n_lat_wind,
                        *n_steps,
                        1,
                        hour / 6.0,
                        &samples,
                        &mut scratch,
                    );
                    for (sample, value) in samples.iter().zip(values) {
                        results[sample.output_index][component] = value;
                    }
                }
            }
            if want_current {
                for &(idx, _, lat, _) in &group {
                    results[idx][0] = meters_per_degree_lon(results[idx][0], lat);
                    results[idx][1] = meters_per_degree_lat(results[idx][1], lat);
                }
            }
        }
        results
    }

    pub fn get_velocities_wind(
        &self,
        view: &ParticleView,
        day: usize,
        hour: f32,
    ) -> Vec<(f32, f32, f32, f32)> {
        self.sample_groups(
            self.group_positions(view.iter(), day),
            view.indices.len(),
            hour,
            true,
            true,
            false,
        )
        .into_iter()
        .map(|v| (v[0], v[1], v[2], v[3]))
        .collect()
    }

    pub fn get_velocities_wind_slice(
        &self,
        positions: &[(f32, f32, f32)],
        day: usize,
        hour: f32,
    ) -> Vec<(f32, f32, f32, f32)> {
        self.sample_groups(
            self.group_positions_slice(positions, day),
            positions.len(),
            hour,
            true,
            true,
            false,
        )
        .into_iter()
        .map(|v| (v[0], v[1], v[2], v[3]))
        .collect()
    }

    pub fn get_velocities(
        &self,
        positions: &[(f32, f32, f32)],
        day: usize,
        hour: f32,
    ) -> Vec<(f32, f32)> {
        self.sample_groups(
            self.group_positions_slice(positions, day),
            positions.len(),
            hour,
            true,
            false,
            false,
        )
        .into_iter()
        .map(|v| (v[0], v[1]))
        .collect()
    }

    pub fn get_wind_sst(&self, view: &ParticleView, day: usize, hour: f32) -> Vec<(f32, f32, f32)> {
        self.sample_groups(
            self.group_positions(view.iter(), day),
            view.indices.len(),
            hour,
            false,
            true,
            true,
        )
        .into_iter()
        .map(|v| (v[2], v[3], v[4]))
        .collect()
    }

    // ===== Loading / tiling =====

    fn tile_url(&self, date: usize, tile: &TileKey) -> String {
        let year = date / 10000;
        let month = (date / 100) % 100;
        let day = date % 100;
        format!(
            "{}/{:04}/{:02}/{:02}/{:03}_{:03}.bin",
            self.base_url, year, month, day, tile.lon_idx, tile.lat_idx,
        )
    }

    async fn load_tile(&self, url: &str) -> Result<TileData, LoaderError> {
        let bytes = if let Some(bytes) = self.fetcher.preloaded(url) {
            bytes
        } else {
            self.fetcher.fetch_bytes(url).await.map_err(|e| match e {
                FetchError::Network(msg) => LoaderError::Network(msg),
                FetchError::Http(code) => LoaderError::Http(code),
            })?
        };
        parse_tile_data(&bytes).map_err(LoaderError::Parse)
    }

    pub fn update_tiles(&mut self, positions: Vec<f32>, current_day: usize) -> HashSet<TileKey> {
        let mut needed = HashSet::new();
        let edge_threshold = 0.1;

        for position in positions.chunks_exact(2) {
            let lon = normalize_lon(position[0]) as f64;
            let lat = position[1] as f64;

            let lon_idx = ((lon - self.min_lon as f64) / self.tile_size as f64).floor() as i32;
            let lat_idx = ((lat - self.min_lat as f64) / self.tile_size as f64).floor() as i32;

            let mut add_tile = |lon_idx: i32, lat_idx: i32| {
                if lon_idx >= 0 && lon_idx < 36 && lat_idx >= 0 && lat_idx < 17 {
                    needed.insert(TileKey {
                        lon_idx: lon_idx as usize,
                        lat_idx: lat_idx as usize,
                        day: current_day,
                    });
                }
            };

            add_tile(lon_idx, lat_idx);

            let lon_mod = ((lon % 10.0) + 10.0) % 10.0;
            let lat_mod = ((lat % 10.0) + 10.0) % 10.0;

            if lon_mod < edge_threshold {
                add_tile(lon_idx - 1, lat_idx);
            } else if lon_mod > 10.0 - edge_threshold {
                add_tile(lon_idx + 1, lat_idx);
            }

            if lat_mod < edge_threshold {
                add_tile(lon_idx, lat_idx - 1);
            } else if lat_mod > 10.0 - edge_threshold {
                add_tile(lon_idx, lat_idx + 1);
            }

            if (lon_mod < edge_threshold || lon_mod > 10.0 - edge_threshold)
                && (lat_mod < edge_threshold || lat_mod > 10.0 - edge_threshold)
            {
                let lon_dir = if lon_mod < edge_threshold { -1 } else { 1 };
                let lat_dir = if lat_mod < edge_threshold { -1 } else { 1 };
                add_tile(lon_idx + lon_dir, lat_idx + lat_dir);
            }
        }
        self.cache.retain(|k, _| needed.contains(k));
        needed
    }

    pub async fn load_by_date(
        &mut self,
        date: usize,
        tiles: &HashSet<TileKey>,
    ) -> Result<(), LoaderError> {
        for tile in tiles {
            if self.cache.contains_key(tile) || self.pending.contains(tile) {
                continue;
            }

            self.pending.insert(tile.clone());
            let url = self.tile_url(date, tile);

            match self.load_tile(&url).await {
                Ok(data) => {
                    self.cache.insert(tile.clone(), RefCell::new(data));
                }
                Err(e) => {
                    self.pending.remove(tile);
                    return Err(e);
                }
            }
            self.pending.remove(tile);
        }
        Ok(())
    }

    pub async fn load_ocean_tiles(&mut self, positions: Vec<f32>, current_date_int: usize) {
        let needed_ocean_tiles = self.update_tiles(positions, current_date_int);

        if let Err(e) = self
            .load_by_date(current_date_int, &needed_ocean_tiles)
            .await
        {
            log::error!("Failed to load ocean tiles: {:?}", e);
        }
    }

    pub fn get_tile_key(&self, lon: f32, lat: f32, day: usize) -> TileKey {
        let lon_idx = ((lon - self.min_lon) / self.tile_size).floor() as i32;
        let lat_idx = ((lat - self.min_lat) / self.tile_size).floor() as i32;

        TileKey {
            lon_idx: lon_idx.max(0).min(35) as usize,
            lat_idx: lat_idx.max(0).min(16) as usize,
            day,
        }
    }

    pub fn get_cell_index(
        &self,
        lon: f32,
        lat: f32,
        n_lon: usize,
        n_lat: usize,
        step: f32,
        tile_min_lon: f32,
        tile_min_lat: f32,
    ) -> (usize, usize) {
        let lon_cell = ((lon - tile_min_lon) / step).floor() as i32;
        let lat_cell = ((lat - tile_min_lat) / step).floor() as i32;

        let lon_cell = lon_cell.max(0).min(n_lon as i32 - 2) as usize;
        let lat_cell = lat_cell.max(0).min(n_lat as i32 - 2) as usize;

        (lon_cell, lat_cell)
    }
}

/// fractional coordinates reused by fields on the same grid
#[derive(Clone, Copy)]
struct GridSample {
    output_index: usize,
    x: f32,
    y: f32,
}

fn sample_spatial(
    grid: &[f32],
    width: usize,
    height: usize,
    sample: &GridSample,
    nearest: bool,
) -> f32 {
    let (mut x, mut y) = (sample.x, sample.y);
    if !x.is_finite() || !y.is_finite() {
        return f32::NAN;
    }
    if nearest {
        x = x.clamp(0.0, (width - 1) as f32);
        y = y.clamp(0.0, (height - 1) as f32);
    } else if x < 0.0 || y < 0.0 || x > (width - 1) as f32 || y > (height - 1) as f32 {
        return f32::NAN;
    }
    let col = (x.floor() as usize).min(width - 2);
    let row = (y.floor() as usize).min(height - 2);
    bilerp(
        grid,
        x - col as f32,
        y - row as f32,
        row * width + col,
        width,
    )
}

/// expand nonfinite cells by one step using a snapshot of the whole slice
fn expand_nan_neighbors(grid: &mut [f32], width: usize, height: usize, scratch: &mut Vec<f32>) {
    scratch.clear();
    scratch.extend_from_slice(grid);
    for row in 0..height {
        for col in 0..width {
            let idx = row * width + col;
            if scratch[idx].is_finite() {
                continue;
            }
            let mut maximum = f32::NEG_INFINITY;
            for y in row.saturating_sub(1)..=(row + 1).min(height - 1) {
                for x in col.saturating_sub(1)..=(col + 1).min(width - 1) {
                    let value = scratch[y * width + x];
                    if value.is_finite() {
                        maximum = maximum.max(value);
                    }
                }
            }
            grid[idx] = if maximum.is_finite() {
                maximum
            } else {
                f32::NAN
            };
        }
    }
}

/// preserve successful samples and retry only failures after each expansion
fn interpolate_batch(
    grid: &mut [f32],
    width: usize,
    height: usize,
    samples: &[GridSample],
    scratch: &mut Vec<f32>,
) -> Vec<f32> {
    if width < 2 || height < 2 || grid.len() != width * height {
        return vec![f32::NAN; samples.len()];
    }
    let mut values: Vec<_> = samples
        .iter()
        .map(|sample| sample_spatial(grid, width, height, sample, false))
        .collect();
    let mut missing: Vec<_> = values
        .iter()
        .enumerate()
        .filter_map(|(i, value)| (!value.is_finite()).then_some(i))
        .collect();
    if missing.is_empty() {
        return values;
    }
    if !grid.iter().any(|v| v.is_finite()) {
        values.fill(f32::NAN);
        return values;
    }
    for _ in 0..10 {
        expand_nan_neighbors(grid, width, height, scratch);
        missing.retain(|&i| {
            values[i] = sample_spatial(grid, width, height, &samples[i], true);
            !values[i].is_finite()
        });
        if missing.is_empty() {
            break;
        }
    }
    values
}

/// spatially sample surface slices before interpolating between times
fn interpolate_time_batch(
    data: &mut [f32],
    width: usize,
    height: usize,
    times: usize,
    depths: usize,
    time: f32,
    samples: &[GridSample],
    scratch: &mut Vec<f32>,
) -> Vec<f32> {
    let cells = width * height;
    if times == 0
        || depths == 0
        || cells == 0
        || !time.is_finite()
        || data.len() < times * depths * cells
    {
        return vec![f32::NAN; samples.len()];
    }
    let time = time.clamp(0.0, (times - 1) as f32);
    let before = time.floor() as usize;
    let fraction = time - before as f32;
    let offset = before * depths * cells;
    let mut values = interpolate_batch(
        &mut data[offset..offset + cells],
        width,
        height,
        samples,
        scratch,
    );
    // an exact timestamp does not require the next slice
    if fraction == 0.0 {
        return values;
    }
    let offset = (before + 1) * depths * cells;
    let after = interpolate_batch(
        &mut data[offset..offset + cells],
        width,
        height,
        samples,
        scratch,
    );
    for (value, next) in values.iter_mut().zip(after) {
        *value = lerp(*value, next, fraction);
    }
    values
}

#[cfg(test)]
mod interpolation_tests {
    use super::*;
    use crate::fetch::BoxFuture;

    fn sample(x: f32, y: f32) -> GridSample {
        GridSample {
            output_index: 0,
            x,
            y,
        }
    }

    #[test]
    fn finite_values_and_zeros_do_not_expand_the_grid() {
        let mut grid = [2.0, -2.0, 2.0, -2.0];
        let mut scratch = Vec::new();
        let values = interpolate_batch(
            &mut grid,
            2,
            2,
            &[sample(0.5, 0.5), sample(0.0, 0.0)],
            &mut scratch,
        );
        assert_eq!(values, [0.0, 2.0]);
        assert_eq!(grid, [2.0, -2.0, 2.0, -2.0]);
        assert!(scratch.is_empty());
    }

    #[test]
    fn dilation_uses_a_snapshot_and_the_largest_finite_neighbor() {
        let mut grid = [f32::NAN, -5.0, f32::INFINITY, -2.0];
        let values = interpolate_batch(&mut grid, 2, 2, &[sample(0.5, 0.5)], &mut Vec::new());
        assert_eq!(values, [-2.75]);
        assert_eq!(grid, [-2.0, -5.0, -2.0, -2.0]);
        let mut grid = [2.0, f32::NAN, f32::NAN, 0.0, f32::NAN, f32::NAN];
        expand_nan_neighbors(&mut grid, 3, 2, &mut Vec::new());
        assert_eq!(grid[1], 2.0);
        assert!(grid[2].is_nan());
        assert_eq!(grid[3], 0.0);
    }

    #[test]
    fn batch_order_does_not_change_results_or_expanded_values() {
        let mut original = vec![f32::NAN; 8 * 2];
        original[0] = 2.0;
        original[8] = 2.0;
        original[7] = 8.0;
        original[15] = 8.0;
        let samples = [sample(0.25, 0.5), sample(3.5, 0.5), sample(6.75, 0.5)];
        let mut forward_grid = original.clone();
        let forward = interpolate_batch(&mut forward_grid, 8, 2, &samples, &mut Vec::new());
        let mut reverse_grid = original;
        let reversed: Vec<_> = samples.iter().copied().rev().collect();
        let reverse = interpolate_batch(&mut reverse_grid, 8, 2, &reversed, &mut Vec::new());
        assert_eq!(forward, reverse.into_iter().rev().collect::<Vec<_>>());
        assert_eq!(forward_grid, reverse_grid);
        assert_eq!(forward, [2.0, 5.0, 8.0]);
    }

    #[test]
    fn retries_stop_at_ten_and_later_batches_reuse_expansion() {
        let mut grid = vec![f32::NAN; 30 * 2];
        grid[0] = 4.0;
        grid[30] = 4.0;
        let samples = [sample(9.5, 0.5), sample(10.5, 0.5)];
        let values = interpolate_batch(&mut grid, 30, 2, &samples, &mut Vec::new());
        assert_eq!(values[0], 4.0);
        assert!(values[1].is_nan());
        assert!(grid[11].is_nan());
        let values = interpolate_batch(&mut grid, 30, 2, &samples, &mut Vec::new());
        assert_eq!(values, [4.0, 4.0]);
    }

    #[test]
    fn missing_grids_stay_missing_and_retries_clamp_to_the_edge() {
        let mut missing = [f32::NAN; 4];
        let mut scratch = Vec::new();
        assert!(
            interpolate_batch(&mut missing, 2, 2, &[sample(0.5, 0.5)], &mut scratch)[0].is_nan()
        );
        assert!(scratch.is_empty());
        let mut grid = [1.0, 2.0, 3.0, 4.0];
        assert_eq!(
            interpolate_batch(&mut grid, 2, 2, &[sample(-1.0, 0.5)], &mut scratch),
            [2.0]
        );
    }

    #[test]
    fn temporal_sampling_is_spatial_first_and_isolates_depth_slices() {
        let mut data = [
            f32::NAN,
            2.0,
            4.0,
            6.0,
            99.0,
            99.0,
            99.0,
            99.0,
            10.0,
            10.0,
            10.0,
            10.0,
            88.0,
            88.0,
            88.0,
            88.0,
        ];
        let values = interpolate_time_batch(
            &mut data,
            2,
            2,
            2,
            2,
            0.5,
            &[sample(0.5, 0.5)],
            &mut Vec::new(),
        );
        assert_eq!(values, [7.25]);
        assert_eq!(data[0], 6.0);
        assert_eq!(&data[4..8], &[99.0; 4]);
        assert_eq!(&data[12..], &[88.0; 4]);
    }

    #[test]
    fn exact_and_final_timestamps_do_not_require_another_slice() {
        let mut data = [0.0, 0.0, 0.0, 0.0, f32::NAN, f32::NAN, f32::NAN, f32::NAN];
        assert_eq!(
            interpolate_time_batch(
                &mut data,
                2,
                2,
                2,
                1,
                0.0,
                &[sample(0.5, 0.5)],
                &mut Vec::new()
            ),
            [0.0]
        );
        let mut data = [f32::NAN, f32::NAN, f32::NAN, f32::NAN, 2.0, 2.0, 2.0, 2.0];
        assert_eq!(
            interpolate_time_batch(
                &mut data,
                2,
                2,
                2,
                1,
                24.0,
                &[sample(0.5, 0.5)],
                &mut Vec::new()
            ),
            [2.0]
        );
        assert!(data[0].is_nan());
    }

    #[test]
    fn matches_scipy_reference_for_mixed_missing_values_and_edges() {
        let mut grid: Vec<_> = (0..30)
            .map(|i| {
                if i % 4 == 0 || i % 7 == 0 {
                    f32::NAN
                } else {
                    ((i * 7) % 19 - 9) as f32 / 4.0
                }
            })
            .collect();
        let samples = [
            sample(0.5, 0.5),
            sample(2.3, 1.7),
            sample(-0.4, 2.0),
            sample(5.2, 3.0),
            sample(5.0, 4.0),
            sample(0.0, 0.0),
            sample(4.7, 0.2),
            sample(2.1, 4.0),
            sample(3.0, 3.0),
        ];
        // expected values from the supplied algorithm using SciPy float32 arrays
        let expected = [
            -0.1875,
            1.0350000858306885,
            1.5,
            0.0,
            1.0,
            -0.5,
            1.1800000667572021,
            0.6749998331069946,
            2.25,
        ];
        let values = interpolate_batch(&mut grid, 6, 5, &samples, &mut Vec::new());
        for (value, expected) in values.iter().zip(expected) {
            assert!((value - expected).abs() < 1e-6);
        }
    }

    #[test]
    fn padded_header_preserves_origins_nans_and_legacy_tiles() {
        let mut legacy = Vec::new();
        for value in [2u32, 2, 1] {
            legacy.extend(value.to_le_bytes());
        }
        legacy.extend(0.0f32.to_le_bytes());
        for _ in 0..25 {
            for value in [f32::NAN, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0] {
                legacy.extend(f16::from_f32(value).to_bits().to_le_bytes());
            }
        }
        let old = parse_tile_data(&legacy).unwrap();
        assert_eq!(old.current_origin, None);
        let mut padded = b"PRT2".to_vec();
        for value in [109.916664f32, 19.916666, 109.75, 19.75] {
            padded.extend(value.to_le_bytes());
        }
        padded.extend(legacy);
        let new = parse_tile_data(&padded).unwrap();
        assert_eq!(new.current_origin, Some((109.916664, 19.916666)));
        assert_eq!(new.wind_origin, Some((109.75, 19.75)));
        assert!(new.u[0].is_nan());
        assert_eq!(new.v, old.v);
        assert!(parse_tile_data(b"PRT2").is_err());
    }

    #[test]
    fn one_cell_halos_interpolate_continuously_across_tile_edges() {
        let mut loader = DataLoader::new("", -180.0, -80.0, UnusedFetcher);
        for lon_idx in [28, 29] {
            let origin = (
                -180.0 + lon_idx as f32 * 10.0 - 1.0 / 12.0,
                20.0 - 1.0 / 12.0,
            );
            let wind_origin = (-180.0 + lon_idx as f32 * 10.0 - 0.25, 19.75);
            let field = |width: usize, step: f32, origin: (f32, f32)| -> Vec<f32> {
                (0..width * width)
                    .map(|i| {
                        (origin.0 + (i % width) as f32 * step) * 0.01
                            + (origin.1 + (i / width) as f32 * step) * 0.02
                    })
                    .collect()
            };
            let currents = field(122, 1.0 / 12.0, origin);
            let winds = field(42, 0.25, wind_origin);
            loader.cache.insert(
                TileKey {
                    lon_idx,
                    lat_idx: 10,
                    day: 1,
                },
                RefCell::new(TileData {
                    current_origin: Some(origin),
                    wind_origin: Some(wind_origin),
                    u: currents.clone(),
                    v: currents,
                    wind_u: winds.clone(),
                    wind_v: winds.clone(),
                    sst_k: winds,
                    depths: vec![0.0],
                    n_lon: 122,
                    n_lat: 122,
                    n_lon_wind: 42,
                    n_lat_wind: 42,
                    n_hours: 1,
                    n_steps: 1,
                }),
            );
        }
        let positions = [
            (109.999, 20.01, 0.0),
            (110.0, 20.01, 0.0),
            (110.001, 20.01, 0.0),
        ];
        let values = loader.get_velocities_wind_slice(&positions, 1, 0.0);
        for ((u, v, wind_u, wind_v), (lon, lat, _)) in values.into_iter().zip(positions) {
            let expected = lon * 0.01 + lat * 0.02;
            assert!((u / meters_per_degree_lon(1.0, lat) - expected).abs() < 1e-6);
            assert!((v / meters_per_degree_lat(1.0, lat) - expected).abs() < 1e-6);
            assert!((wind_u - expected).abs() < 1e-6);
            assert!((wind_v - expected).abs() < 1e-6);
        }
    }

    #[test]
    fn parses_python_zstd_tiles_without_changing_origins_nans_or_zeros() {
        // fixture compressed by Python zstandard at level 3
        let bytes: &[u8] = &[
            40, 181, 47, 253, 96, 180, 0, 181, 1, 0, 148, 2, 80, 82, 84, 50, 85, 213, 219, 66, 85,
            85, 159, 65, 0, 128, 219, 66, 0, 0, 158, 65, 2, 0, 0, 0, 1, 0, 126, 0, 0, 0, 192, 0,
            66, 0, 68, 0, 69, 0, 70, 0, 71, 3, 0, 125, 51, 85, 197, 121, 227, 213, 225, 1,
        ];
        let tile = parse_tile_data(bytes).unwrap();
        assert_eq!(tile.current_origin, Some((109.916664, 19.916666)));
        assert_eq!(tile.wind_origin, Some((109.75, 19.75)));
        assert_eq!(tile.n_hours, 25);
        assert!(tile.u[0].is_nan());
        assert_eq!(tile.u[1], 0.0);
        assert_eq!(tile.u[2], -2.0);
        assert_eq!(tile.v[0], 4.0);
        assert!(parse_tile_data(&[0x28, 0xb5, 0x2f, 0xfd]).is_err());
        assert!(parse_tile_data(&bytes[..bytes.len() - 3]).is_err());
    }

    struct UnusedFetcher;
    impl TileFetcher for UnusedFetcher {
        fn fetch_bytes<'a>(&'a self, _url: &'a str) -> BoxFuture<'a, Result<Vec<u8>, FetchError>> {
            Box::pin(async { panic!("test tiles are already cached") })
        }
    }

    #[test]
    fn loader_batches_currents_wind_and_sst_into_output_order() {
        let mut loader = DataLoader::new("", 0.0, 0.0, UnusedFetcher);
        let field = || vec![f32::NAN, 2.0, 4.0, 6.0, 10.0, 10.0, 10.0, 10.0];
        for lon_idx in [0, 1] {
            loader.cache.insert(
                TileKey {
                    lon_idx,
                    lat_idx: 0,
                    day: 1,
                },
                RefCell::new(TileData {
                    current_origin: None,
                    wind_origin: None,
                    u: field(),
                    v: field(),
                    wind_u: field(),
                    wind_v: field(),
                    sst_k: field(),
                    depths: vec![0.0],
                    n_lon: 2,
                    n_lat: 2,
                    n_lon_wind: 2,
                    n_lat_wind: 2,
                    n_hours: 2,
                    n_steps: 2,
                }),
            );
        }
        let positions = [
            (10.0 + 1.0 / 24.0, 1.0 / 24.0, 0.0),
            (1.0 / 24.0, 1.0 / 24.0, 0.0),
        ];
        let values = loader.sample_groups(
            loader.group_positions_slice(&positions, 1),
            2,
            0.5,
            true,
            true,
            true,
        );
        for value in values {
            assert!((value[0] - meters_per_degree_lon(7.25, positions[0].1)).abs() < 1e-8);
            assert!((value[1] - meters_per_degree_lat(7.25, positions[0].1)).abs() < 1e-8);
            assert!((value[2] - value[3]).abs() < 1e-6);
            assert_eq!(value[3], value[4]);
        }
        for tile in loader.cache.values() {
            let tile = tile.borrow();
            for data in [&tile.u, &tile.v, &tile.wind_u, &tile.wind_v, &tile.sst_k] {
                assert_eq!(data[0], 6.0);
            }
        }
    }
}

pub fn parse_tile_data(bytes: &[u8]) -> Result<TileData, String> {
    let decoded;
    let bytes = if bytes.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
        let mut decoder = ruzstd::decoding::StreamingDecoder::new(bytes)
            .map_err(|e| format!("Invalid zstd tile header: {e}"))?;
        let mut output = Vec::new();
        decoder
            .read_to_end(&mut output)
            .map_err(|e| format!("Failed to decompress zstd tile: {e}"))?;
        decoded = output;
        decoded.as_slice()
    } else {
        bytes
    };
    let (bytes, current_origin, wind_origin) = if bytes.starts_with(b"PRT2") {
        if bytes.len() < 20 {
            return Err("File too short for padded tile header".to_string());
        }
        let value = |offset| f32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
        let current = (value(4), value(8));
        let wind = (value(12), value(16));
        if ![current.0, current.1, wind.0, wind.1]
            .iter()
            .all(|v| v.is_finite())
        {
            return Err("Invalid padded grid origin".to_string());
        }
        (&bytes[20..], Some(current), Some(wind))
    } else {
        (bytes, None, None)
    };
    if bytes.len() < 12 {
        return Err("File too short for header".to_string());
    }

    let n_lon = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    let n_lat = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    let n_depths = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;

    let mut depths = Vec::with_capacity(n_depths);
    let mut offset = 12;
    for _ in 0..n_depths {
        let depth_val = f32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]);
        depths.push(depth_val);
        offset += 4;
    }

    let n_cells = n_lon * n_lat;
    let n_hours = 25;

    let mut u = Vec::with_capacity(n_hours * n_depths * n_cells);
    let mut v = Vec::with_capacity(n_hours * n_depths * n_cells);

    for _ in 0..n_hours {
        for _ in 0..n_depths {
            let u_f16 = &bytes[offset..offset + n_cells * 2];
            offset += n_cells * 2;
            u.extend(
                u_f16
                    .chunks_exact(2)
                    .map(|c| f16::from_bits(u16::from_le_bytes([c[0], c[1]])).to_f32()),
            );

            let v_f16 = &bytes[offset..offset + n_cells * 2];
            offset += n_cells * 2;
            v.extend(
                v_f16
                    .chunks_exact(2)
                    .map(|c| f16::from_bits(u16::from_le_bytes([c[0], c[1]])).to_f32()),
            );
        }
    }

    // Check if wind data is present (need at least 12 bytes for wind header)
    if offset + 12 > bytes.len() {
        // No wind data — return with empty wind/SST vectors
        return Ok(TileData {
            current_origin,
            wind_origin,
            u,
            v,
            wind_u: Vec::new(),
            wind_v: Vec::new(),
            sst_k: Vec::new(),
            n_lon,
            n_lat,
            n_lon_wind: 0,
            n_lat_wind: 0,
            depths,
            n_hours,
            n_steps: 0,
        });
    }

    // Parse wind header
    let n_lon_wind = u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ]) as usize;
    let n_lat_wind = u32::from_le_bytes([
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ]) as usize;
    let n_steps = u32::from_le_bytes([
        bytes[offset + 8],
        bytes[offset + 9],
        bytes[offset + 10],
        bytes[offset + 11],
    ]) as usize;

    offset += 12;

    let n_cells_wind = n_lon_wind * n_lat_wind;
    let wind_bytes_needed = n_steps * n_cells_wind * 2 * 3; // u + v + sst, 2 bytes each

    // Check if we have enough bytes for wind data
    if offset + wind_bytes_needed > bytes.len() {
        // Incomplete wind data — return without wind
        return Ok(TileData {
            current_origin,
            wind_origin,
            u,
            v,
            wind_u: Vec::new(),
            wind_v: Vec::new(),
            sst_k: Vec::new(),
            n_lon,
            n_lat,
            n_lon_wind: 0,
            n_lat_wind: 0,
            depths,
            n_hours,
            n_steps: 0,
        });
    }

    let mut wind_u = Vec::with_capacity(n_steps * n_cells_wind);
    let mut wind_v = Vec::with_capacity(n_steps * n_cells_wind);
    let mut sst_k = Vec::with_capacity(n_steps * n_cells_wind);

    for _ in 0..n_steps {
        let wind_u_f16 = &bytes[offset..offset + n_cells_wind * 2];
        offset += n_cells_wind * 2;
        wind_u.extend(
            wind_u_f16
                .chunks_exact(2)
                .map(|c| f16::from_bits(u16::from_le_bytes([c[0], c[1]])).to_f32()),
        );

        let wind_v_f16 = &bytes[offset..offset + n_cells_wind * 2];
        offset += n_cells_wind * 2;
        wind_v.extend(
            wind_v_f16
                .chunks_exact(2)
                .map(|c| f16::from_bits(u16::from_le_bytes([c[0], c[1]])).to_f32()),
        );

        let sst_f16 = &bytes[offset..offset + n_cells_wind * 2];
        offset += n_cells_wind * 2;
        sst_k.extend(
            sst_f16
                .chunks_exact(2)
                .map(|c| f16::from_bits(u16::from_le_bytes([c[0], c[1]])).to_f32()),
        );
    }

    Ok(TileData {
        current_origin,
        wind_origin,
        u,
        v,
        wind_u,
        wind_v,
        sst_k,
        n_lon,
        n_lat,
        n_lon_wind,
        n_lat_wind,
        depths,
        n_hours,
        n_steps,
    })
}
