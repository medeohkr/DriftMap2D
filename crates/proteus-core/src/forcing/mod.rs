// mod.rs defines the forcing interface and selects a concrete source

mod cache;
mod format;
pub mod global;
mod interpolation;
pub mod norkyst;
mod projection;

use crate::fetch::TileFetcher;
pub use cache::GridData;
pub use global::GlobalForcing;
pub use norkyst::NorkystForcing;
use thiserror::Error;

const TIME_EPSILON: f32 = 0.000_01;

/// spatial tile indices and the UTC date encoded as YYYYMMDD
#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub struct TileKey {
    pub lon_idx: usize,
    pub lat_idx: usize,
    pub day: usize,
}

/// fields needed by an advection, diffusion, or weathering query
#[derive(Clone, Copy)]
pub enum ForcingFields {
    Currents,
    CurrentsWind,
    WindTemperature,
}

impl ForcingFields {
    fn current(self) -> bool {
        matches!(self, Self::Currents | Self::CurrentsWind)
    }
    fn wind(self) -> bool {
        matches!(self, Self::CurrentsWind | Self::WindTemperature)
    }
    fn sst(self) -> bool {
        matches!(self, Self::WindTemperature)
    }
}

/// environmental data used by the simulation, independent of the source grid
///
/// positions are longitude and latitude. samples preserve input order.
/// currents are degrees per second, wind is metres per second, and temperature
/// is kelvin. unavailable values remain NaN. sampling may expand cached fields.
#[allow(async_fn_in_trait)]
pub trait Forcing {
    /// load the spatial margin and every date required by an integration step
    ///
    /// positions are flattened longitude-latitude pairs. the date is YYYYMMDD,
    /// hour is UTC within that date, and duration is in hours. futures need not
    /// be Send because browser fetches run on the WASM thread.
    async fn prepare_forcing(
        &mut self,
        positions: Vec<f32>,
        date: usize,
        hour: f32,
        duration_hours: f32,
    ) -> Result<(), LoaderError>;

    /// reject steps outside the available timeline
    fn validate_forcing_interval(
        &self,
        date: usize,
        hour: f32,
        duration_hours: f32,
    ) -> Result<(), LoaderError>;

    /// sample current u/v, wind u/v, and temperature, in that order
    fn sample_fields(
        &self,
        positions: impl ExactSizeIterator<Item = (f32, f32)>,
        day: usize,
        hour: f32,
        fields: ForcingFields,
    ) -> Vec<[f32; 5]>;

    /// physical grid cell area for the diffusion scheme, in square metres
    fn cell_area_m2(&self, lat: f32) -> f32;

    fn get_velocities_wind(
        &self,
        positions: impl ExactSizeIterator<Item = (f32, f32)>,
        day: usize,
        hour: f32,
    ) -> Vec<(f32, f32, f32, f32)> {
        self.sample_fields(positions, day, hour, ForcingFields::CurrentsWind)
            .into_iter()
            .map(|v| (v[0], v[1], v[2], v[3]))
            .collect()
    }

    fn get_velocities_wind_slice(
        &self,
        positions: &[(f32, f32)],
        day: usize,
        hour: f32,
    ) -> Vec<(f32, f32, f32, f32)> {
        self.get_velocities_wind(positions.iter().copied(), day, hour)
    }

    fn get_velocities(&self, positions: &[(f32, f32)], day: usize, hour: f32) -> Vec<(f32, f32)> {
        self.sample_fields(
            positions.iter().copied(),
            day,
            hour,
            ForcingFields::Currents,
        )
        .into_iter()
        .map(|v| (v[0], v[1]))
        .collect()
    }

    fn get_wind_sst(
        &self,
        positions: impl ExactSizeIterator<Item = (f32, f32)>,
        day: usize,
        hour: f32,
    ) -> Vec<(f32, f32, f32)> {
        self.sample_fields(positions, day, hour, ForcingFields::WindTemperature)
            .into_iter()
            .map(|v| (v[2], v[3], v[4]))
            .collect()
    }
}

/// runtime source selection with static dispatch and source-owned caches
pub enum ForcingKind<F: TileFetcher> {
    Global(GlobalForcing<F>),
    Norkyst(NorkystForcing<F>),
}

impl<F: TileFetcher> ForcingKind<F> {
    pub fn global(base_url: &str, min_lon: f32, min_lat: f32, fetcher: F) -> Self {
        Self::Global(GlobalForcing::new(base_url, min_lon, min_lat, fetcher))
    }

    pub fn norkyst(base_url: &str, fetcher: F) -> Self {
        Self::Norkyst(NorkystForcing::new(base_url, fetcher))
    }
}

impl<F: TileFetcher> Forcing for ForcingKind<F> {
    async fn prepare_forcing(
        &mut self,
        positions: Vec<f32>,
        date: usize,
        hour: f32,
        duration_hours: f32,
    ) -> Result<(), LoaderError> {
        match self {
            Self::Global(source) => {
                source
                    .prepare_forcing(positions, date, hour, duration_hours)
                    .await
            }
            Self::Norkyst(source) => {
                source
                    .prepare_forcing(positions, date, hour, duration_hours)
                    .await
            }
        }
    }

    fn validate_forcing_interval(
        &self,
        date: usize,
        hour: f32,
        duration_hours: f32,
    ) -> Result<(), LoaderError> {
        match self {
            Self::Global(source) => source.validate_forcing_interval(date, hour, duration_hours),
            Self::Norkyst(source) => source.validate_forcing_interval(date, hour, duration_hours),
        }
    }

    fn sample_fields(
        &self,
        positions: impl ExactSizeIterator<Item = (f32, f32)>,
        day: usize,
        hour: f32,
        fields: ForcingFields,
    ) -> Vec<[f32; 5]> {
        match self {
            Self::Global(source) => source.sample_fields(positions, day, hour, fields),
            Self::Norkyst(source) => source.sample_fields(positions, day, hour, fields),
        }
    }

    fn cell_area_m2(&self, lat: f32) -> f32 {
        match self {
            Self::Global(source) => source.cell_area_m2(lat),
            Self::Norkyst(source) => source.cell_area_m2(lat),
        }
    }
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
mod tests;
