pub fn meters_per_degree_lat(value: f32) -> f32 {
    value / 111_120.0
}

pub fn meters_per_degree_lon(value: f32, lat: f32) -> f32 {
    value / (111_120.0 * lat.to_radians().cos())
}

pub fn normalize_lon(lon: f32) -> f32 {
    let mut lon = lon;
    while lon < -180.0 {
        lon += 360.0;
    }
    while lon >= 180.0 {
        lon -= 360.0;
    }
    lon
}

pub fn lerp(a: f32, b: f32, frac: f32) -> f32 {
    a + frac * (b - a)
}

pub fn bilerp(data: &[f32], frac_lon: f32, frac_lat: f32, idx: usize, next_row: usize) -> f32 {
    let a0 = data[idx];
    let b0 = data[idx + 1];
    let a1 = data[idx + next_row];
    let b1 = data[idx + next_row + 1];

    lerp(lerp(a0, b0, frac_lon), lerp(a1, b1, frac_lon), frac_lat)
}

pub fn find_depth_indices(depths: &[f32], target_depth: f32) -> (usize, f32) {
    if target_depth <= depths[0] {
        return (0, 0.0);
    }

    if target_depth >= depths[depths.len() - 1] {
        return (depths.len() - 1, 0.0);
    }

    for i in 0..depths.len() - 1 {
        if target_depth >= depths[i] && target_depth <= depths[i + 1] {
            let t = (target_depth - depths[i]) / (depths[i + 1] - depths[i]);
            return (i, t);
        }
    }

    (0, 0.0)
}
