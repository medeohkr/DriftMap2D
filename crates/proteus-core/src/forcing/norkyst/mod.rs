// mod.rs loads native projected forcing and reuses the shared interpolator

mod cache;
mod format;
mod metadata;
mod preload;

use super::preload::{tile_url, PreloadPlan};
use super::sampling::SamplingScratch;
use super::{interpolation::GridSample, Forcing, ForcingFields, LoaderError, TileKey};
use crate::arrays::{Coordinates, FieldBuffers, FieldOutput};
use crate::{
    fetch::{FetchError, TileFetcher},
    utils::{meters_per_degree_lat, meters_per_degree_lon, normalize_lon},
};
use cache::Tile;
use metadata::Manifest;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
};

pub struct NorkystForcing<F: TileFetcher> {
    fetcher: F,
    base_url: String,
    manifest: Option<Manifest>,
    tiles: HashMap<TileKey, RefCell<Tile>>,
    sampling: RefCell<SamplingScratch<NativeSample>>,
    prepared: Vec<preload::PreparedPosition>,
    bounds: Vec<Option<preload::Bounds>>,
    preload: PreloadPlan,
}

#[derive(Default)]
struct NativeSample {
    output_index: usize,
    lat: f32,
    x: f32,
    y: f32,
}

async fn fetch<F: TileFetcher>(fetcher: &F, url: &str) -> Result<Vec<u8>, LoaderError> {
    if let Some(bytes) = fetcher.preloaded(url) {
        return Ok(bytes);
    }
    fetcher.fetch_bytes(url).await.map_err(|e| match e {
        FetchError::Network(message) => LoaderError::Network(format!("{url}: {message}")),
        FetchError::Http(code) => LoaderError::Http(code),
    })
}

impl<F: TileFetcher> NorkystForcing<F> {
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
        let Some(manifest) = &self.manifest else {
            output.reset(count);
            return;
        };
        for (i, (lon, lat)) in positions.enumerate() {
            let lon = normalize_lon(lon);
            // only reuse preparation coordinates when the input position matches exactly
            let projected = self
                .prepared
                .get(i)
                .filter(|p| p.lon.to_bits() == lon.to_bits() && p.lat.to_bits() == lat.to_bits())
                .map(|p| (p.x, p.y))
                .or_else(|| manifest.grid.position(lon, lat));
            let Some((x, y)) = projected else {
                continue;
            };
            let key = TileKey {
                lon_idx: x as usize / 120,
                lat_idx: y as usize / 120,
                day,
            };
            let sample = NativeSample {
                output_index: i,
                lat,
                x: (x - (key.lon_idx * 120) as f64 + 1.0) as f32,
                y: (y - (key.lat_idx * 120) as f64 + 1.0) as f32,
            };
            scratch.groups.entry(key).or_default().push(sample);
        }
        scratch.groups.retain(|_, group| !group.is_empty());
        self.sample_into(
            &mut scratch,
            count,
            hour,
            fields.current(),
            fields.wind(),
            fields.sst(),
            output,
        );
    }
    pub fn new(base_url: &str, fetcher: F) -> Self {
        Self {
            fetcher,
            base_url: base_url.trim_end_matches('/').to_owned(),
            manifest: None,
            tiles: HashMap::new(),
            sampling: RefCell::new(SamplingScratch::default()),
            prepared: Vec::new(),
            bounds: Vec::new(),
            preload: PreloadPlan::default(),
        }
    }

    #[cfg(test)]
    fn key(&self, lon: f32, lat: f32, day: usize) -> Option<TileKey> {
        let grid = &self.manifest.as_ref()?.grid;
        let (x, y) = grid.position(lon, lat)?;
        Some(TileKey {
            lon_idx: x as usize / 120,
            lat_idx: y as usize / 120,
            day,
        })
    }

    async fn ensure_manifest(&mut self) -> Result<(), LoaderError> {
        if self.manifest.is_none() {
            let url = format!("{}/manifest.json", self.base_url);
            self.manifest = Some(
                Manifest::parse(&fetch(&self.fetcher, &url).await?)
                    .map_err(|e| LoaderError::Parse(format!("{url}: {e}")))?,
            );
        }
        Ok(())
    }

    fn interval_dates(
        &self,
        date: usize,
        hour: f32,
        duration: f32,
    ) -> Result<Vec<usize>, LoaderError> {
        let manifest = self
            .manifest
            .as_ref()
            .ok_or_else(|| LoaderError::NotFound("NorKyst manifest".into()))?;
        let day = manifest
            .day(date)
            .ok_or_else(|| LoaderError::NotFound(format!("NorKyst date {date}")))?;
        if !hour.is_finite()
            || !duration.is_finite()
            || hour < 0.0
            || hour >= 24.0
            || duration < 0.0
            || duration > 24.0
        {
            return Err(LoaderError::NotFound("invalid forcing interval".into()));
        }
        let end = day.timestamps[0] as f64 + (hour as f64 + duration as f64) * 3600.0;
        let mut limit = *day.timestamps.last().unwrap();
        let mut dates = vec![date];
        while end > limit as f64 + 0.01 {
            let next = chrono::DateTime::from_timestamp(limit, 0).unwrap();
            let next_date = next.format("%Y%m%d").to_string().parse().unwrap();
            let Some(next_day) = manifest.day(next_date) else {
                return Err(LoaderError::NotFound(
                    "advance exceeds NorKyst time coverage".into(),
                ));
            };
            let next_limit = *next_day.timestamps.last().unwrap();
            if next_day.timestamps[0] != limit || next_limit <= limit {
                return Err(LoaderError::NotFound(
                    "advance exceeds NorKyst time coverage".into(),
                ));
            }
            dates.push(next_date);
            limit = next_limit;
        }
        Ok(dates)
    }

    fn sample_into(
        &self,
        scratch: &mut SamplingScratch<NativeSample>,
        count: usize,
        hour: f32,
        current: bool,
        wind: bool,
        sst: bool,
        results: &mut impl FieldOutput,
    ) {
        results.reset(count);
        let SamplingScratch {
            groups,
            samples,
            values,
            interpolation,
        } = scratch;
        let Some(manifest) = &self.manifest else {
            return;
        };
        for (key, group) in groups.iter() {
            let Some(day) = manifest.day(key.day) else {
                continue;
            };
            let timestamp = day.timestamps[0] as f64 + hour as f64 * 3600.0;
            if !timestamp.is_finite() {
                continue;
            }
            let tile_key = if timestamp > *day.timestamps.last().unwrap() as f64 + 0.01 {
                let Some(time) = chrono::DateTime::from_timestamp(timestamp.floor() as i64, 0)
                else {
                    continue;
                };
                let target = time.format("%Y%m%d").to_string().parse().unwrap();
                TileKey {
                    day: target,
                    ..key.clone()
                }
            } else {
                key.clone()
            };
            let Some(tile) = self.tiles.get(&tile_key) else {
                continue;
            };
            let mut tile = tile.borrow_mut();
            if !timestamp.is_finite()
                || timestamp < tile.timestamps[0] as f64
                || timestamp > *tile.timestamps.last().unwrap() as f64 + 0.004
            {
                continue;
            }
            let before = tile
                .timestamps
                .partition_point(|&t| t as f64 <= timestamp)
                .saturating_sub(1);
            let fraction = if before + 1 < tile.timestamps.len() {
                (timestamp - tile.timestamps[before] as f64)
                    / (tile.timestamps[before + 1] - tile.timestamps[before]) as f64
            } else {
                0.0
            };
            samples.clear();
            samples.extend(group.iter().map(|p| GridSample {
                output_index: p.output_index,
                x: p.x,
                y: p.y,
            }));
            for field in 0..5 {
                if !match field {
                    0 | 1 => current,
                    2 | 3 => wind,
                    _ => sst,
                } {
                    continue;
                }
                tile.grid.sample_field_into(
                    field,
                    before as f32 + fraction as f32,
                    samples,
                    values,
                    interpolation,
                );
                for (sample, value) in samples.iter().zip(values.iter().copied()) {
                    results.set(field, sample.output_index, value);
                }
            }
            if current {
                for p in group {
                    let i = p.output_index;
                    results.set(0, i, meters_per_degree_lon(results.get(0, i), p.lat));
                    results.set(1, i, meters_per_degree_lat(results.get(1, i), p.lat));
                }
            }
        }
    }
}

impl<F: TileFetcher> Forcing for NorkystForcing<F> {
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
        self.ensure_manifest().await?;
        let dates = self.interval_dates(date, hour, duration_hours)?;
        let needed = self.plan_positions(positions, date, &dates)?;
        let manifest = self.manifest.as_ref().unwrap();
        let grid = &manifest.grid;
        self.tiles.retain(|k, _| needed.contains(k));
        for key in needed {
            if self.tiles.contains_key(&key) {
                continue;
            }
            let url = tile_url(&self.base_url, &key);
            let bytes = fetch(&self.fetcher, &url).await?;
            let day = manifest.day(key.day).unwrap();
            let tile = Tile::parse(&bytes, grid, [key.lon_idx, key.lat_idx], &day.timestamps)
                .map_err(|e| LoaderError::Parse(format!("{url}: {e}")))?;
            self.tiles.insert(key, RefCell::new(tile));
        }
        Ok(())
    }

    fn validate_forcing_interval(
        &self,
        date: usize,
        hour: f32,
        duration_hours: f32,
    ) -> Result<(), LoaderError> {
        self.interval_dates(date, hour, duration_hours).map(|_| ())
    }

    fn cell_area_m2(&self, lat: f32) -> f32 {
        let Some(manifest) = &self.manifest else {
            return f32::NAN;
        };
        let scale = manifest.grid.projection.scale(lat as f64);
        (800.0 * 800.0 / scale.powi(2)) as f32
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
