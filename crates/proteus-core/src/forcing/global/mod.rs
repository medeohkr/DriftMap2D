// mod.rs loads and samples the global longitude-latitude forcing grids

mod cache;
mod format;
mod grid;
mod preload;

use super::interpolation::GridSample;
#[cfg(test)]
use super::interpolation::{expand_nan_neighbors, interpolate_batch};
use super::preload::PreloadPlan;
use super::sampling::SamplingScratch;
use super::{
    next_date, validate_hours, Forcing, ForcingFields, LoaderError, TileKey, TIME_EPSILON,
};
use crate::arrays::{Coordinates, FieldBuffers, FieldOutput};
use crate::fetch::{FetchError, TileFetcher};
use crate::utils::{meters_per_degree_lat, meters_per_degree_lon, normalize_lon};
pub use cache::Tile;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

/// generic F allows the same loader to work in WASM
/// (GlooFetcher) and in the CLI (ReqwestFetcher)
pub struct GlobalForcing<F: TileFetcher> {
    min_lon: f32,
    min_lat: f32,
    step: f32,
    step_wind: f32,
    tile_size: f32,
    base_url: String,
    fetcher: F,

    cache: HashMap<TileKey, RefCell<Tile>>,
    pending: HashSet<TileKey>, // ensures tiles mid-download aren't requested
    sampling: RefCell<SamplingScratch>,
    preload: PreloadPlan,
}

impl<F: TileFetcher> GlobalForcing<F> {
    fn sample_positions_into(
        &self,
        positions: impl ExactSizeIterator<Item = (f32, f32)>,
        day: usize,
        hour: f32,
        fields: ForcingFields,
        output: &mut impl FieldOutput,
    ) {
        let count = positions.len();
        let mut scratch = self.sampling.borrow_mut();
        scratch.clear_groups();
        for (i, (lon, lat)) in positions.enumerate() {
            let lon = normalize_lon(lon);
            let key = self.get_tile_key(lon, lat, day);
            scratch.groups.entry(key).or_default().push((i, lon, lat));
        }
        scratch.groups.retain(|_, group| !group.is_empty());
        self.sample_groups_into(
            &mut scratch,
            count,
            hour,
            fields.current(),
            fields.wind(),
            fields.sst(),
            output,
        );
    }
    pub fn new(base_url: &str, min_lon: f32, min_lat: f32, fetcher: F) -> Self {
        Self {
            min_lon,
            min_lat,
            step: 1.0 / 12.0,
            step_wind: 1.0 / 4.0,
            tile_size: 10.0,
            base_url: base_url.trim_end_matches('/').to_string(),
            fetcher,
            cache: HashMap::new(),
            pending: HashSet::new(),
            sampling: RefCell::new(SamplingScratch::default()),
            preload: PreloadPlan::default(),
        }
    }

    /// group positions from a generic iterator, avoiding extra vec allocation
    #[cfg(test)]
    fn group_positions<I>(&self, iter: I, day: usize) -> HashMap<TileKey, Vec<(usize, f32, f32)>>
    where
        I: IntoIterator<Item = (usize, f32, f32)>,
    {
        let mut groups: HashMap<TileKey, Vec<(usize, f32, f32)>> = HashMap::new();
        for (i, lon, lat) in iter {
            let lon = normalize_lon(lon);
            let key = self.get_tile_key(lon, lat, day);
            groups
                .entry(key)
                .or_insert_with(Vec::new)
                .push((i, lon, lat));
        }
        groups
    }

    #[cfg(test)]
    fn group_positions_slice(
        &self,
        positions: &[(f32, f32)],
        day: usize,
    ) -> HashMap<TileKey, Vec<(usize, f32, f32)>> {
        self.group_positions(
            positions
                .iter()
                .enumerate()
                .map(|(i, &(lon, lat))| (i, lon, lat)),
            day,
        )
    }

    /// sample each tile as a batch before converting current units
    fn sample_groups_into(
        &self,
        scratch: &mut SamplingScratch,
        count: usize,
        hour: f32,
        want_current: bool,
        want_wind: bool,
        want_sst: bool,
        results: &mut impl FieldOutput,
    ) {
        results.reset(count);
        let SamplingScratch {
            groups,
            samples,
            values,
            interpolation,
        } = scratch;
        for (key, group) in groups.iter() {
            let mut key = key.clone();
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
                for &(index, _, _) in group {
                    for field in 0..5 {
                        results.set(field, index, f32::NAN);
                    }
                }
                continue;
            }
            let (min_lon, min_lat) = self.tile_origin(&key);
            let prepare_samples =
                |step: f32, origin: Option<(f32, f32)>, samples: &mut Vec<GridSample>| {
                    let (min_lon, min_lat) = origin.unwrap_or((min_lon, min_lat));
                    samples.clear();
                    samples.extend(group.iter().map(|&(output_index, lon, lat)| GridSample {
                        output_index,
                        x: (lon - min_lon) / step,
                        y: (lat - min_lat) / step,
                    }));
                };
            if want_current {
                prepare_samples(self.step, tile.current_origin, samples);
                let grid = &mut tile.currents;
                let last = (grid.sample_count - 1) as f32;
                let time = if sample_hour <= last + TIME_EPSILON {
                    sample_hour.min(last)
                } else {
                    f32::NAN
                };
                for component in 0..2 {
                    grid.sample_field_into(component, time, samples, values, interpolation);
                    for (sample, value) in samples.iter().zip(values.iter().copied()) {
                        results.set(component, sample.output_index, value);
                    }
                }
            }
            if want_wind || want_sst {
                prepare_samples(self.step_wind, tile.wind_origin, samples);
                if let Some(grid) = &mut tile.atmosphere {
                    let last = (grid.sample_count - 1) as f32;
                    let time = if sample_hour <= last * 6.0 + TIME_EPSILON {
                        (sample_hour / 6.0).min(last)
                    } else {
                        f32::NAN
                    };
                    for (field, requested) in
                        [want_wind, want_wind, want_sst].into_iter().enumerate()
                    {
                        if !requested {
                            continue;
                        }
                        grid.sample_field_into(field, time, samples, values, interpolation);
                        for (sample, value) in samples.iter().zip(values.iter().copied()) {
                            results.set(field + 2, sample.output_index, value);
                        }
                    }
                }
            }
            if want_current {
                for &(idx, _, lat) in group {
                    results.set(0, idx, meters_per_degree_lon(results.get(0, idx), lat));
                    results.set(1, idx, meters_per_degree_lat(results.get(1, idx), lat));
                }
            }
        }
    }

    #[cfg(test)]
    fn sample_groups(
        &self,
        groups: HashMap<TileKey, Vec<(usize, f32, f32)>>,
        count: usize,
        hour: f32,
        current: bool,
        wind: bool,
        sst: bool,
    ) -> Vec<[f32; 5]> {
        let mut scratch = SamplingScratch {
            groups,
            ..Default::default()
        };
        let mut values = Vec::new();
        self.sample_groups_into(&mut scratch, count, hour, current, wind, sst, &mut values);
        values
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

    async fn load_tile(&self, url: &str) -> Result<Tile, LoaderError> {
        let bytes = if let Some(bytes) = self.fetcher.preloaded(url) {
            bytes
        } else {
            self.fetcher.fetch_bytes(url).await.map_err(|e| match e {
                FetchError::Network(msg) => LoaderError::Network(msg),
                FetchError::Http(code) => LoaderError::Http(code),
            })?
        };
        Tile::parse(&bytes).map_err(|error| LoaderError::Parse(format!("{url}: {error}")))
    }

    fn tiles_for_positions(&self, positions: &[f32], current_day: usize) -> HashSet<TileKey> {
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

    async fn load_by_date(
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

    fn validate_global_day(&self, date: usize, end: f32) -> Result<(), LoaderError> {
        let mut found = false;
        for (key, cached) in self.cache.iter().filter(|(key, _)| key.day == date) {
            found = true;
            let tile = cached.borrow();
            let mut last = tile.currents.sample_count.saturating_sub(1) as f32;
            if let Some(atmosphere) = &tile.atmosphere {
                last = last.min((atmosphere.sample_count - 1) as f32 * 6.0);
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

impl<F: TileFetcher> Forcing for GlobalForcing<F> {
    fn cell_area_m2(&self, lat: f32) -> f32 {
        (1.0 / 144.0) * (111_120.0f32.powi(2) * lat.to_radians().cos())
    }

    async fn prepare_forcing(
        &mut self,
        positions: Vec<f32>,
        date: usize,
        hour: f32,
        duration_hours: f32,
    ) -> Result<(), LoaderError> {
        self.prepare_forcing_slice(&positions, date, hour, duration_hours)
            .await
    }

    async fn prepare_forcing_slice(
        &mut self,
        positions: &[f32],
        date: usize,
        hour: f32,
        duration_hours: f32,
    ) -> Result<(), LoaderError> {
        validate_hours(hour, duration_hours)?;
        let current = self.tiles_for_positions(positions, date);
        self.update_preload(&current, date);
        let next = if hour + duration_hours > 24.0 + TIME_EPSILON {
            let date = next_date(date)?;
            Some((
                date,
                current
                    .iter()
                    .map(|key| TileKey {
                        day: date,
                        ..key.clone()
                    })
                    .collect::<HashSet<_>>(),
            ))
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

    fn validate_forcing_interval(
        &self,
        date: usize,
        hour: f32,
        duration_hours: f32,
    ) -> Result<(), LoaderError> {
        validate_hours(hour, duration_hours)?;
        let end = hour + duration_hours;
        self.validate_global_day(date, end.min(24.0))?;
        if end > 24.0 + TIME_EPSILON {
            self.validate_global_day(next_date(date)?, end - 24.0)?;
        }
        Ok(())
    }

    fn sample_fields(
        &self,
        positions: impl ExactSizeIterator<Item = (f32, f32)>,
        day: usize,
        hour: f32,
        fields: ForcingFields,
    ) -> Vec<[f32; 5]> {
        let mut output = Vec::new();
        self.sample_fields_into(positions, day, hour, fields, &mut output);
        output
    }

    fn sample_fields_into(
        &self,
        positions: impl ExactSizeIterator<Item = (f32, f32)>,
        day: usize,
        hour: f32,
        fields: ForcingFields,
        output: &mut Vec<[f32; 5]>,
    ) {
        self.sample_positions_into(positions, day, hour, fields, output);
    }

    fn sample_soa_into(
        &self,
        positions: Coordinates<'_>,
        day: usize,
        hour: f32,
        fields: ForcingFields,
        output: &mut FieldBuffers,
    ) {
        self.sample_positions_into(positions.iter(), day, hour, fields, output);
    }
}

#[cfg(test)]
mod tests;
