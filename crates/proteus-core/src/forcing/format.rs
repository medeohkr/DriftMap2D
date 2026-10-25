// format.rs decompresses and parses forcing tile payloads

use super::TileData;
use half::f16;
use std::io::Read;

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
    let (bytes, current_origin, wind_origin, n_hours) =
        if bytes.starts_with(b"PRT2") || bytes.starts_with(b"PRT3") {
            let counted = bytes.starts_with(b"PRT3");
            let header_size = if counted { 24 } else { 20 };
            if bytes.len() < header_size {
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
            let hours = if counted {
                u32::from_le_bytes(bytes[20..24].try_into().unwrap()) as usize
            } else {
                25
            };
            if !(2..=25).contains(&hours) {
                return Err(format!("Invalid current sample count: {hours}"));
            }
            (&bytes[header_size..], Some(current), Some(wind), hours)
        } else {
            (bytes, None, None, 25)
        };
    if bytes.len() < 12 {
        return Err("File too short for header".to_string());
    }

    let n_lon = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    let n_lat = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    let n_depths = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;

    if n_lon < 2 || n_lat < 2 || n_depths == 0 {
        return Err(format!(
            "Invalid current grid dimensions: {n_lon} x {n_lat} x {n_depths}"
        ));
    }
    if n_depths > (bytes.len() - 12) / 4 {
        return Err("Truncated depth header or incompatible tile format".to_string());
    }
    let n_cells = n_lon
        .checked_mul(n_lat)
        .ok_or("Current grid dimensions overflow")?;
    let current_values = n_hours
        .checked_mul(n_depths)
        .and_then(|n| n.checked_mul(n_cells))
        .ok_or("Current payload dimensions overflow")?;
    let depth_end = 12 + n_depths * 4;
    let current_bytes = current_values
        .checked_mul(4)
        .ok_or("Current payload size overflow")?;
    if current_bytes > bytes.len() - depth_end {
        return Err(format!(
            "Truncated current payload: expected {current_bytes} bytes for {n_hours} samples, found {}",
            bytes.len() - depth_end
        ));
    }

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

    let mut u = Vec::with_capacity(current_values);
    let mut v = Vec::with_capacity(current_values);

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
    if offset == bytes.len() {
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

    if bytes.len() - offset < 12 {
        return Err("Truncated wind header".to_string());
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

    let n_cells_wind = n_lon_wind
        .checked_mul(n_lat_wind)
        .ok_or("Wind grid dimensions overflow")?;
    let wind_values = n_steps
        .checked_mul(n_cells_wind)
        .ok_or("Wind payload dimensions overflow")?;
    let wind_bytes_needed = wind_values
        .checked_mul(6)
        .ok_or("Wind payload size overflow")?;

    // Check if we have enough bytes for wind data
    if wind_bytes_needed > bytes.len() - offset {
        return Err(format!(
            "Truncated wind payload: expected {wind_bytes_needed} bytes, found {}",
            bytes.len() - offset
        ));
    }
    if wind_values == 0 {
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

    if n_lon_wind < 2 || n_lat_wind < 2 {
        return Err("Invalid wind grid dimensions".to_string());
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
