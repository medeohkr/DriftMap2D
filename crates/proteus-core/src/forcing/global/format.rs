// format.rs decodes global tile headers and keeps only surface fields

use super::Tile;
use crate::forcing::{
    format::{checked_product, decompress, Reader},
    GridData,
};

const MAX_TILE_BYTES: usize = 64 * 1024 * 1024;

struct Header {
    width: usize,
    height: usize,
    sample_count: usize,
    stored_layers: usize,
    surface_layer: usize,
    current_origin: Option<(f32, f32)>,
    wind_origin: Option<(f32, f32)>,
}

impl Header {
    fn parse(reader: &mut Reader<'_>) -> Result<Self, String> {
        let (mut current_origin, mut wind_origin, mut sample_count) = (None, None, 25);
        if reader.starts_with(b"PRT2") || reader.starts_with(b"PRT3") {
            let counted = reader.take(4)? == b"PRT3";
            let origin = (reader.f32()?, reader.f32()?);
            let wind = (reader.f32()?, reader.f32()?);
            if ![origin.0, origin.1, wind.0, wind.1]
                .iter()
                .all(|v| v.is_finite())
            {
                return Err("invalid global grid origin".into());
            }
            current_origin = Some(origin);
            wind_origin = Some(wind);
            if counted {
                sample_count = reader.u32()?;
            }
        } else if reader.starts_with(b"PRT") {
            return Err("unsupported global tile version".into());
        }
        if !(2..=25).contains(&sample_count) {
            return Err("invalid global sample count".into());
        }
        let width = reader.u32()?;
        let height = reader.u32()?;
        let stored_layers = reader.u32()?;
        if width < 2 || height < 2 || stored_layers == 0 {
            return Err("invalid global grid dimensions".into());
        }
        if stored_layers > reader.remaining() / 4 {
            return Err("truncated legacy layer header".into());
        }
        // depth exists only in the legacy file layout, not in the runtime model
        let mut surface_layer = 0;
        let mut shallowest = f32::INFINITY;
        for layer in 0..stored_layers {
            let depth = reader.f32()?;
            if !depth.is_finite() {
                return Err("invalid legacy layer depth".into());
            }
            if depth.abs() < shallowest {
                shallowest = depth.abs();
                surface_layer = layer;
            }
        }
        Ok(Self {
            width,
            height,
            sample_count,
            stored_layers,
            surface_layer,
            current_origin,
            wind_origin,
        })
    }
}

impl Tile {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let decoded = decompress(bytes, MAX_TILE_BYTES)?;
        let mut reader = Reader::new(&decoded);
        let header = Header::parse(&mut reader)?;
        let cells = checked_product(&[header.width, header.height])?;
        let payload_bytes =
            checked_product(&[cells, header.sample_count, header.stored_layers, 4])?;
        if payload_bytes > reader.remaining() {
            return Err("truncated global current payload".into());
        }
        let mut currents = GridData::new(header.width, header.height, header.sample_count)?;
        for _ in 0..header.sample_count {
            for layer in 0..header.stored_layers {
                for field in &mut currents.fields {
                    if layer == header.surface_layer {
                        reader.read_field(field, cells)?;
                    } else {
                        reader.take(checked_product(&[cells, 2])?)?;
                    }
                }
            }
        }
        let atmosphere = read_atmosphere(&mut reader)?;
        if reader.remaining() != 0 {
            return Err("unexpected bytes after global fields".into());
        }
        Ok(Self {
            currents,
            atmosphere,
            current_origin: header.current_origin,
            wind_origin: header.wind_origin,
        })
    }
}

fn read_atmosphere(reader: &mut Reader<'_>) -> Result<Option<GridData<3>>, String> {
    if reader.remaining() == 0 {
        return Ok(None);
    }
    let width = reader.u32()?;
    let height = reader.u32()?;
    let sample_count = reader.u32()?;
    let values = checked_product(&[width, height, sample_count])?;
    if values == 0 {
        return Ok(None);
    }
    if checked_product(&[values, 6])? != reader.remaining() {
        return Err("invalid global atmospheric payload size".into());
    }
    let mut grid = GridData::new(width, height, sample_count)?;
    let cells = checked_product(&[width, height])?;
    for _ in 0..sample_count {
        for field in &mut grid.fields {
            reader.read_field(field, cells)?;
        }
    }
    Ok(Some(grid))
}
