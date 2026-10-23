// pub fn meters_per_degree_lat(value: f32, _lat: f32) -> f32 {
//     value / 111_120.0
// }

// pub fn meters_per_degree_lon(value: f32, lat: f32) -> f32 {
//     value / (111_120.0 * lat.to_radians().cos())
// }
/// WGS84 ellipsoid parameters
const WGS84_A: f32 = 6_378_137.0;         // semi-major axis, meters
const WGS84_E2: f32 = 6.694_379_990_14e-3; // first eccentricity squared

/// Meters per degree of latitude at the given latitude (WGS84).
pub fn meters_per_degree_lat(value: f32, lat: f32) -> f32 {
    let phi = lat.to_radians();
    let sin_phi = phi.sin();
    let denom = (1.0 - WGS84_E2 * sin_phi * sin_phi).powf(1.5);
    let m = WGS84_A * (1.0 - WGS84_E2) / denom;   // meridional radius
    value / (m * std::f32::consts::PI / 180.0)
}

/// Meters per degree of longitude at the given latitude (WGS84).
pub fn meters_per_degree_lon(value: f32, lat: f32) -> f32 {
    let phi = lat.to_radians();
    let sin_phi = phi.sin();
    let denom = (1.0 - WGS84_E2 * sin_phi * sin_phi).sqrt();
    let n = WGS84_A / denom;                      // prime vertical radius
    let r = n * phi.cos();                        // radius of the parallel
    value / (r * std::f32::consts::PI / 180.0)
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

pub fn bilerp(data: &[f32], frac_lon: f32, frac_lat: f32, idx: usize, row_stride: usize) -> f32 {
    let a0 = data[idx];
    let b0 = data[idx + 1];
    let a1 = data[idx + row_stride];
    let b1 = data[idx + row_stride + 1];

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
