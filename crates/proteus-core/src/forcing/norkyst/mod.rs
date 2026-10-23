// mod.rs loads native projected forcing and reuses the shared interpolator

mod format;
mod metadata;

use super::{grid::GridSample, interpolation::interpolate_time_batch, LoaderError, TileKey};
use crate::{
    fetch::{FetchError, TileFetcher},
    utils::{meters_per_degree_lat, meters_per_degree_lon},
};
use format::Tile;
use metadata::Manifest;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
};

#[derive(Default)]
pub(super) struct Norkyst {
    manifest: Option<Manifest>,
    tiles: HashMap<TileKey, RefCell<Tile>>,
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

impl Norkyst {
    pub(super) fn cell_area_m2(&self, lat: f32) -> f32 {
        let Some(manifest) = &self.manifest else {
            return f32::NAN;
        };
        let scale = manifest.grid.projection.scale(lat as f64);
        (800.0 * 800.0 / scale.powi(2)) as f32
    }

    pub(super) fn key(&self, lon: f32, lat: f32, day: usize) -> Option<TileKey> {
        let grid = &self.manifest.as_ref()?.grid;
        let (x, y) = grid.position(lon, lat)?;
        Some(TileKey {
            lon_idx: x as usize / 120,
            lat_idx: y as usize / 120,
            day,
        })
    }

    pub(super) async fn load<F: TileFetcher>(
        &mut self,
        fetcher: &F,
        base: &str,
        positions: &[f32],
        date: usize,
        hour: f32,
        duration_hours: f32,
    ) -> Result<(), LoaderError> {
        if self.manifest.is_none() {
            let url = format!("{base}/manifest.json");
            self.manifest = Some(
                Manifest::parse(&fetch(fetcher, &url).await?)
                    .map_err(|e| LoaderError::Parse(format!("{url}: {e}")))?,
            );
        }
        let dates = self.interval_dates(date, hour, duration_hours)?;
        let manifest = self.manifest.as_ref().unwrap();
        let grid = &manifest.grid;
        let mut needed = HashSet::new();
        if positions.len() % 2 != 0 {
            return Err(LoaderError::Parse("incomplete position pair".into()));
        }
        for position in positions.chunks_exact(2) {
            let (x, y) = grid.position(position[0], position[1]).ok_or_else(|| {
                LoaderError::NotFound(format!(
                    "position outside NorKyst domain {}, {}",
                    position[0], position[1]
                ))
            })?;
            // preload a 12 km margin for intermediate integration and diffusion probes
            let left = ((x - 15.0).max(0.0) as usize) / 120;
            let right = ((x + 15.0).min((grid.grid_shape[1] - 1) as f64) as usize) / 120;
            let top = ((y - 15.0).max(0.0) as usize) / 120;
            let bottom = ((y + 15.0).min((grid.grid_shape[0] - 1) as f64) as usize) / 120;
            for lon_idx in left..=right {
                for lat_idx in top..=bottom {
                    for &day in &dates {
                        needed.insert(TileKey {
                            lon_idx,
                            lat_idx,
                            day,
                        });
                    }
                }
            }
        }
        self.tiles.retain(|k, _| needed.contains(k));
        for key in needed {
            if self.tiles.contains_key(&key) {
                continue;
            }
            let url = format!(
                "{base}/{:04}/{:02}/{:02}/{:03}_{:03}.bin",
                key.day / 10000,
                key.day / 100 % 100,
                key.day % 100,
                key.lon_idx,
                key.lat_idx
            );
            let bytes = fetch(fetcher, &url).await?;
            let day = manifest.day(key.day).unwrap();
            let tile = Tile::parse(&bytes, grid, [key.lon_idx, key.lat_idx], &day.timestamps)
                .map_err(|e| LoaderError::Parse(format!("{url}: {e}")))?;
            self.tiles.insert(key, RefCell::new(tile));
        }
        Ok(())
    }

    pub(super) fn validate_interval(
        &self,
        date: usize,
        hour: f32,
        duration_hours: f32,
    ) -> Result<(), LoaderError> {
        self.interval_dates(date, hour, duration_hours).map(|_| ())
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

    pub(super) fn sample(
        &self,
        groups: HashMap<TileKey, Vec<(usize, f32, f32, f32)>>,
        count: usize,
        hour: f32,
        current: bool,
        wind: bool,
        sst: bool,
    ) -> Vec<[f32; 5]> {
        let mut results = vec![[f32::NAN; 5]; count];
        let Some(manifest) = &self.manifest else {
            return results;
        };
        let mut scratch = Vec::new();
        for (key, group) in groups {
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
            let samples: Vec<_> = group
                .iter()
                .filter_map(|&(output_index, lon, lat, _)| {
                    let (x, y) = manifest.grid.position(lon, lat)?;
                    Some(GridSample {
                        output_index,
                        x: (x - (key.lon_idx * 120) as f64 + 1.0) as f32,
                        y: (y - (key.lat_idx * 120) as f64 + 1.0) as f32,
                    })
                })
                .collect();
            let times = tile.timestamps.len();
            for field in 0..5 {
                if !match field {
                    0 | 1 => current,
                    2 | 3 => wind,
                    _ => sst,
                } {
                    continue;
                }
                let values = interpolate_time_batch(
                    &mut tile.fields[field],
                    122,
                    122,
                    times,
                    1,
                    before as f32 + fraction as f32,
                    &samples,
                    &mut scratch,
                );
                for (sample, value) in samples.iter().zip(values) {
                    results[sample.output_index][field] = value;
                }
            }
            if current {
                for &(i, _, lat, _) in &group {
                    results[i][0] = meters_per_degree_lon(results[i][0], lat);
                    results[i][1] = meters_per_degree_lat(results[i][1], lat);
                }
            }
        }
        results
    }
}

#[cfg(test)]
mod tests;
