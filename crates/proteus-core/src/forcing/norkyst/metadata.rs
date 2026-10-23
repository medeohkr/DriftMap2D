// metadata.rs validates native grid geometry and available forcing dates

use super::super::projection::PolarStereographic;
use chrono::NaiveDate;
use serde::Deserialize;

pub(super) const FIELDS: [&str; 5] = ["u", "v", "wind_u", "wind_v", "temperature_k"];

#[derive(Deserialize)]
pub(super) struct Grid {
    pub format: String,
    pub core_cells: usize,
    pub halo_cells: usize,
    pub tile_shape: [usize; 2],
    pub grid_shape: [usize; 2],
    pub grid_origin_m: [f64; 2],
    pub grid_spacing_m: [f64; 2],
    pub projection: PolarStereographic,
    pub fields: Vec<String>,
    pub field_units: Vec<String>,
    pub vector_basis: String,
    pub dtype: String,
    pub array_order: String,
    pub compression: String,
}

impl Grid {
    pub fn validate(&self) -> Result<(), String> {
        self.projection.validate()?;
        if self.format != "NKT1"
            || self.core_cells != 120
            || self.halo_cells != 1
            || self.tile_shape != [122, 122]
            || self.grid_shape.iter().any(|&v| v < 2 || v > 100_000)
            || !self.grid_origin_m.iter().all(|v| v.is_finite())
            || self.grid_spacing_m != [800.0, 800.0]
            || self.fields != FIELDS
            || self.field_units != ["m/s", "m/s", "m/s", "m/s", "K"]
            || self.vector_basis != "east_north"
            || self.dtype != "<f2"
            || self.array_order != "field,time,y,x"
            || self.compression != "zstd"
        {
            return Err("unsupported NorKyst grid or field layout".into());
        }
        Ok(())
    }

    pub fn position(&self, lon: f32, lat: f32) -> Option<(f64, f64)> {
        let (x, y) = self.projection.project(lon as f64, lat as f64)?;
        let x = (x - self.grid_origin_m[0]) / self.grid_spacing_m[0];
        let y = (y - self.grid_origin_m[1]) / self.grid_spacing_m[1];
        (x >= 0.0
            && y >= 0.0
            && x <= (self.grid_shape[1] - 1) as f64
            && y <= (self.grid_shape[0] - 1) as f64)
            .then_some((x, y))
    }
}

#[derive(Deserialize)]
pub(super) struct Day {
    pub date: String,
    pub timestamps: Vec<i64>,
}

#[derive(Deserialize)]
pub(super) struct Manifest {
    pub grid: Grid,
    pub days: Vec<Day>,
}

impl Manifest {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let manifest: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        manifest.grid.validate()?;
        if manifest.days.is_empty() {
            return Err("empty NorKyst timeline".into());
        }
        let mut dates = std::collections::HashSet::new();
        for day in &manifest.days {
            let date =
                NaiveDate::parse_from_str(&day.date, "%Y-%m-%d").map_err(|e| e.to_string())?;
            let midnight = date.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
            if !dates.insert(date)
                || day.timestamps.len() < 2
                || day.timestamps.len() > 25
                || day.timestamps[0] != midnight
                || day
                    .timestamps
                    .windows(2)
                    .any(|p| p[1].checked_sub(p[0]) != Some(3600))
            {
                return Err("invalid NorKyst day timestamps".into());
            }
        }
        Ok(manifest)
    }

    pub fn day(&self, date: usize) -> Option<&Day> {
        let name = format!(
            "{:04}-{:02}-{:02}",
            date / 10000,
            date / 100 % 100,
            date % 100
        );
        self.days.iter().find(|d| d.date == name)
    }
}
