// format.rs decodes native tiles and checks their headers against the manifest

use super::metadata::{Grid, FIELDS};
use half::f16;
use serde::Deserialize;
use std::io::Read;

#[derive(Deserialize)]
struct Header {
    tile_index: [usize; 2],
    shape: [usize; 3],
    core_shape: [usize; 2],
    origin_m: [f64; 2],
    timestamps: Vec<i64>,
    fields: Vec<String>,
}

pub(super) struct Tile {
    pub timestamps: Vec<i64>,
    pub fields: [Vec<f32>; 5],
}

impl Tile {
    pub fn parse(
        bytes: &[u8],
        grid: &Grid,
        index: [usize; 2],
        timestamps: &[i64],
    ) -> Result<Self, String> {
        let mut decoded = Vec::new();
        // limit decoded size before trusting dimensions from an external header
        ruzstd::decoding::StreamingDecoder::new(bytes)
            .map_err(|e| e.to_string())?
            .take(8_000_001)
            .read_to_end(&mut decoded)
            .map_err(|e| e.to_string())?;
        if decoded.len() > 8_000_000 || decoded.get(..4) != Some(b"NKT1") {
            return Err("invalid or oversized NorKyst tile".into());
        }
        let length = u32::from_le_bytes(
            decoded
                .get(4..8)
                .ok_or("truncated native header")?
                .try_into()
                .unwrap(),
        ) as usize;
        let end = 8usize.checked_add(length).ok_or("native header overflow")?;
        let header: Header =
            serde_json::from_slice(decoded.get(8..end).ok_or("truncated native header")?)
                .map_err(|e| e.to_string())?;
        let col = index[0].checked_mul(120).ok_or("invalid tile column")?;
        let row = index[1].checked_mul(120).ok_or("invalid tile row")?;
        if col >= grid.grid_shape[1] || row >= grid.grid_shape[0] {
            return Err("tile outside grid".into());
        }
        let origin = [
            grid.grid_origin_m[0] + (col as f64 - 1.0) * 800.0,
            grid.grid_origin_m[1] + (row as f64 - 1.0) * 800.0,
        ];
        let count = timestamps.len() * 122 * 122;
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
            || decoded.len() - end != count * 5 * 2
        {
            return Err("native tile disagrees with manifest or has a truncated payload".into());
        }
        let mut fields: [Vec<f32>; 5] = std::array::from_fn(|_| Vec::with_capacity(count));
        for (field, data) in fields
            .iter_mut()
            .zip(decoded[end..].chunks_exact(count * 2))
        {
            field.extend(
                data.chunks_exact(2)
                    .map(|v| f16::from_bits(u16::from_le_bytes([v[0], v[1]])).to_f32()),
            );
        }
        Ok(Self {
            timestamps: header.timestamps,
            fields,
        })
    }
}
