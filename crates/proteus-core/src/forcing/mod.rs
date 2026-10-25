// mod.rs coordinates forcing tile fetching, caching, and sampling

mod cache;
mod format;
mod grid;
mod interpolation;
mod norkyst;
mod projection;

pub use cache::{TileData, TileKey};
pub use format::parse_tile_data;

use crate::fetch::{FetchError, TileFetcher};
use crate::utils::{meters_per_degree_lat, meters_per_degree_lon, normalize_lon};
use grid::GridSample;
use interpolation::interpolate_time_batch;
#[cfg(test)]
use interpolation::{expand_nan_neighbors, interpolate_batch};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use thiserror::Error;

const TIME_EPSILON: f32 = 0.000_01;

/// generic F allows the same loader to work in WASM
/// (GlooFetcher) and in the CLI (ReqwestFetcher)
pub struct DataLoader<F: TileFetcher> {
    norkyst: Option<norkyst::Norkyst>,
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
            norkyst: None,
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

    /// load native NorKyst tiles using the projection in their manifest
    pub fn new_norkyst(base_url: &str, fetcher: F) -> Self {
        let mut loader = Self::new(base_url.trim_end_matches('/'), 0.0, 0.0, fetcher);
        loader.norkyst = Some(norkyst::Norkyst::default());
        loader
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
            let key = if let Some(native) = &self.norkyst {
                let Some(key) = native.key(lon, lat, day) else {
                    continue;
                };
                key
            } else {
                self.get_tile_key(lon, lat, day)
            };
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
        if let Some(native) = &self.norkyst {
            return native.sample(groups, count, hour, want_current, want_wind, want_sst);
        }
        let mut results = vec![[f32::NAN; 5]; count];
        let mut scratch = Vec::new();
        for (mut key, group) in groups {
            let sample_hour = if hour > 24.0 + TIME_EPSILON {
                let Ok(next) = next_date(key.day) else {
                    continue;
                };
                key.day = next;
                hour - 24.0
            } else {
                hour
            };
            let Some(cached) = self.cache.get(&key) else {
                continue;
            };
            let mut tile = cached.borrow_mut();
            if !sample_hour.is_finite() || sample_hour < 0.0 || sample_hour > 24.0 + TIME_EPSILON {
                for &(index, _, _, _) in &group {
                    results[index].fill(f32::NAN);
                }
                continue;
            }
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
                        if sample_hour <= (times - 1) as f32 + TIME_EPSILON {
                            sample_hour.min((times - 1) as f32)
                        } else {
                            f32::NAN
                        },
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
                        if *n_steps > 0 && sample_hour <= (*n_steps - 1) as f32 * 6.0 + TIME_EPSILON
                        {
                            (sample_hour / 6.0).min((*n_steps - 1) as f32)
                        } else {
                            f32::NAN
                        },
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
        positions: impl ExactSizeIterator<Item = (f32, f32, f32)>,
        day: usize,
        hour: f32,
    ) -> Vec<(f32, f32, f32, f32)> {
        let count = positions.len();
        self.sample_groups(
            self.group_positions(
                positions
                    .enumerate()
                    .map(|(i, (lon, lat, depth))| (i, lon, lat, depth)),
                day,
            ),
            count,
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

    pub fn get_wind_sst(
        &self,
        positions: impl ExactSizeIterator<Item = (f32, f32, f32)>,
        day: usize,
        hour: f32,
    ) -> Vec<(f32, f32, f32)> {
        let count = positions.len();
        self.sample_groups(
            self.group_positions(
                positions
                    .enumerate()
                    .map(|(i, (lon, lat, depth))| (i, lon, lat, depth)),
                day,
            ),
            count,
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
        parse_tile_data(&bytes).map_err(|error| LoaderError::Parse(format!("{url}: {error}")))
    }

    pub fn update_tiles(&mut self, positions: Vec<f32>, current_day: usize) -> HashSet<TileKey> {
        let needed = self.tiles_for_positions(&positions, current_day);
        self.cache.retain(|key, _| needed.contains(key));
        needed
    }

    fn tiles_for_positions(&self, positions: &[f32], current_day: usize) -> HashSet<TileKey> {
        if let Some(native) = &self.norkyst {
            return positions
                .chunks_exact(2)
                .filter_map(|p| native.key(p[0], p[1], current_day))
                .collect();
        }
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
        needed
    }

    pub async fn load_by_date(
        &mut self,
        date: usize,
        tiles: &HashSet<TileKey>,
    ) -> Result<(), LoaderError> {
        if self.norkyst.is_some() {
            return Err(LoaderError::Parse(
                "use try_load_ocean_tiles for native forcing".into(),
            ));
        }
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
        if let Err(e) = self.try_load_ocean_tiles(positions, current_date_int).await {
            log::error!("Failed to load ocean tiles: {:?}", e);
        }
    }

    /// return forcing errors to callers that must stop an invalid simulation
    pub async fn try_load_ocean_tiles(
        &mut self,
        positions: Vec<f32>,
        date: usize,
    ) -> Result<(), LoaderError> {
        if let Some(native) = &mut self.norkyst {
            return native
                .load(&self.fetcher, &self.base_url, &positions, date, 0.0, 0.0)
                .await;
        }
        let needed = self.update_tiles(positions, date);
        self.load_by_date(date, &needed).await
    }

    /// return the physical grid cell area used by the diffusion scheme
    pub fn cell_area_m2(&self, lat: f32) -> f32 {
        if let Some(native) = &self.norkyst {
            native.cell_area_m2(lat)
        } else {
            (1.0 / 144.0) * (111_120.0f32.powi(2) * lat.to_radians().cos())
        }
    }

    /// preload every date needed by intermediate integration stages
    pub async fn prepare_forcing(
        &mut self,
        positions: Vec<f32>,
        date: usize,
        hour: f32,
        duration_hours: f32,
    ) -> Result<(), LoaderError> {
        if let Some(native) = &mut self.norkyst {
            return native
                .load(
                    &self.fetcher,
                    &self.base_url,
                    &positions,
                    date,
                    hour,
                    duration_hours,
                )
                .await;
        }
        validate_hours(hour, duration_hours)?;
        let current = self.tiles_for_positions(&positions, date);
        let next = if hour + duration_hours > 24.0 + TIME_EPSILON {
            let date = next_date(date)?;
            Some((date, self.tiles_for_positions(&positions, date)))
        } else {
            None
        };
        self.cache.retain(|key, _| {
            current.contains(key) || next.as_ref().is_some_and(|(_, keys)| keys.contains(key))
        });
        self.load_by_date(date, &current).await?;
        if let Some((date, keys)) = next {
            self.load_by_date(date, &keys).await?;
        }
        self.validate_forcing_interval(date, hour, duration_hours)
    }

    /// reject advances beyond the final available forecast sample
    pub fn validate_forcing_interval(
        &self,
        date: usize,
        hour: f32,
        duration_hours: f32,
    ) -> Result<(), LoaderError> {
        if let Some(native) = &self.norkyst {
            native.validate_interval(date, hour, duration_hours)
        } else {
            validate_hours(hour, duration_hours)?;
            let end = hour + duration_hours;
            self.validate_global_day(date, end.min(24.0))?;
            if end > 24.0 + TIME_EPSILON {
                self.validate_global_day(next_date(date)?, end - 24.0)?;
            }
            Ok(())
        }
    }

    fn validate_global_day(&self, date: usize, end: f32) -> Result<(), LoaderError> {
        let mut found = false;
        for (key, cached) in self.cache.iter().filter(|(key, _)| key.day == date) {
            found = true;
            let tile = cached.borrow();
            let mut last = tile.n_hours.saturating_sub(1) as f32;
            if tile.n_steps > 0 {
                last = last.min((tile.n_steps - 1) as f32 * 6.0);
            }
            if end > last + TIME_EPSILON {
                return Err(LoaderError::NotFound(format!(
                    "advance exceeds global time coverage for {date} tile={}_{}: last sample at hour {last}", key.lon_idx, key.lat_idx
                )));
            }
        }
        if !found && !self.cache.is_empty() {
            return Err(LoaderError::NotFound(format!(
                "global forcing is not loaded for {date}"
            )));
        }
        Ok(())
    }
}

fn validate_hours(hour: f32, duration: f32) -> Result<(), LoaderError> {
    if !hour.is_finite()
        || !duration.is_finite()
        || !(0.0..24.0).contains(&hour)
        || !(0.0..=24.0).contains(&duration)
    {
        return Err(LoaderError::NotFound("invalid forcing interval".into()));
    }
    Ok(())
}

fn next_date(date: usize) -> Result<usize, LoaderError> {
    chrono::NaiveDate::from_ymd_opt(
        (date / 10000) as i32,
        (date / 100 % 100) as u32,
        (date % 100) as u32,
    )
    .and_then(|date| date.succ_opt())
    .map(|date| date.format("%Y%m%d").to_string().parse().unwrap())
    .ok_or_else(|| LoaderError::NotFound("invalid forcing date".into()))
}

#[cfg(test)]
#[path = "tests.rs"]
mod interpolation_tests;
