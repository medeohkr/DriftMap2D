// format.rs decodes native tiles and checks their headers against the manifest

use super::{
    cache::Tile,
    metadata::{Grid, FIELDS},
};
use serde::Deserialize;

use crate::forcing::{
    format::{checked_product, decompress, is_zstd, Reader},
    GridData,
};

#[derive(Deserialize)]
struct Header {
    tile_index: [usize; 2],
    shape: [usize; 3], // time, rows, columns
    core_shape: [usize; 2],
    origin_m: [f64; 2],
    timestamps: Vec<i64>,
    fields: Vec<String>,
}

impl Tile {
    pub fn parse(
        bytes: &[u8],
        grid: &Grid,
        index: [usize; 2],
        timestamps: &[i64],
    ) -> Result<Self, String> {
        if !is_zstd(bytes) {
            return Err("native tile must be zstd compressed".into());
        }
        let decoded = decompress(bytes, 8_000_000)?;
        let mut reader = Reader::new(&decoded);
        if reader.take(4)? != b"NKT1" {
            return Err("invalid native tile version".into());
        }
        let length = reader.u32()?;
        let header: Header = serde_json::from_slice(reader.take(length)?)
            .map_err(|e| format!("invalid native header: {e}"))?;
        let col = index[0].checked_mul(120).ok_or("invalid tile column")?;
        let row = index[1].checked_mul(120).ok_or("invalid tile row")?;
        if col >= grid.grid_shape[1] || row >= grid.grid_shape[0] {
            return Err("tile outside grid".into());
        }
        let origin = [
            grid.grid_origin_m[0] + (col as f64 - 1.0) * 800.0,
            grid.grid_origin_m[1] + (row as f64 - 1.0) * 800.0,
        ];
        let count = checked_product(&[timestamps.len(), 122, 122])?;
        if header.tile_index != index
            || header.shape != [timestamps.len(), 122, 122]
            || header.core_shape
                != [
                    120.min(grid.grid_shape[0] - row),
                    120.min(grid.grid_shape[1] - col),
                ]
            || header.origin_m != origin
            || header.timestamps != timestamps
            || header.fields != FIELDS
            || reader.remaining() != checked_product(&[count, 5, 2])?
        {
            return Err("native tile disagrees with manifest or has a truncated payload".into());
        }
        let mut grid = GridData::new(122, 122, timestamps.len())?;
        for field in &mut grid.fields {
            reader.read_field(field, count)?;
        }
        Ok(Self {
            timestamps: header.timestamps,
            grid,
        })
    }
}
