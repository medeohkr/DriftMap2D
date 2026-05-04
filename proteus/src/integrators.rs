// integrators.rs

pub fn euler_step(
    lon: f32,
    lat: f32,
    depth: f32,
    dt: f32,
    velocity_fn: impl Fn(f32, f32, f32) -> (f32, f32),
) -> (f32, f32) {
    let (u, v) = velocity_fn(lon, lat, depth);
    (lon + dt * u, lat + dt * v)
}

/// Euler step with pre-computed velocity (no function call needed)
pub fn euler_step_batch(
    lon: f32,
    lat: f32,
    u: f32,
    v: f32,
    dt: f32,
) -> (f32, f32) {
    (lon + dt * u, lat + dt * v)
}

pub fn midpoint_step(
    lon: f32,
    lat: f32,
    depth: f32,
    dt: f32,
    velocity_fn: impl Fn(f32, f32, f32) -> (f32, f32),
) -> (f32, f32) {
    // First half step
    let (u1, v1) = velocity_fn(lon, lat, depth);
    let lon_mid = lon + 0.5 * dt * u1;
    let lat_mid = lat + 0.5 * dt * v1;
    
    // Velocity at midpoint
    let (u_mid, v_mid) = velocity_fn(lon_mid, lat_mid, depth);
    
    // Full step using midpoint velocity
    (lon + dt * u_mid, lat + dt * v_mid)
}

/// RK4 step for a single particle
pub fn rk4_step(
    lon: f32,
    lat: f32,
    depth: f32,
    dt: f32,
    velocity_fn: impl Fn(f32, f32, f32) -> (f32, f32),
) -> (f32, f32) {
    // k1
    let (u1, v1) = velocity_fn(lon, lat, depth);
    
    // k2
    let lon2 = lon + 0.5 * dt * u1;
    let lat2 = lat + 0.5 * dt * v1;
    let (u2, v2) = velocity_fn(lon2, lat2, depth);
    
    // k3
    let lon3 = lon + 0.5 * dt * u2;
    let lat3 = lat + 0.5 * dt * v2;
    let (u3, v3) = velocity_fn(lon3, lat3, depth);
    
    // k4
    let lon4 = lon + dt * u3;
    let lat4 = lat + dt * v3;
    let (u4, v4) = velocity_fn(lon4, lat4, depth);
    
    // weighted average
    let u = (u1 + 2.0 * u2 + 2.0 * u3 + u4) / 6.0;
    let v = (v1 + 2.0 * v2 + 2.0 * v3 + v4) / 6.0;
    
    (lon + dt * u, lat + dt * v)
}

// ========== BATCH METHODS ==========

/// Batch midpoint integration for multiple particles
pub fn midpoint_step_batch(
    positions: &[(f32, f32, f32)],  // (lon, lat, depth)
    dt: f32,
    get_velocities: impl Fn(&[(f32, f32, f32)]) -> Vec<(f32, f32)>,
) -> Vec<(f32, f32)> {
    let n = positions.len();
    
    // Step 1: Get k1 velocities at current positions
    let k1 = get_velocities(positions);
    
    // Step 2: Compute midpoint positions
    let midpoint_positions: Vec<(f32, f32, f32)> = positions.iter()
        .enumerate()
        .map(|(i, &(lon, lat, depth))| {
            let (u, v) = k1[i];
            (
                lon + 0.5 * dt * u,
                lat + 0.5 * dt * v,
                depth,
            )
        })
        .collect();
    
    // Step 3: Get velocities at midpoints
    let k_mid = get_velocities(&midpoint_positions);
    
    // Step 4: Full step using midpoint velocities
    positions.iter()
        .enumerate()
        .map(|(i, &(lon, lat, _))| {
            let (u_mid, v_mid) = k_mid[i];
            (lon + dt * u_mid, lat + dt * v_mid)
        })
        .collect()
}

/// Batch RK4 integration for multiple particles
pub fn rk4_step_batch(
    positions: &[(f32, f32, f32)],  // (lon, lat, depth)
    dt: f32,
    get_velocities: impl Fn(&[(f32, f32, f32)]) -> Vec<(f32, f32)>,
) -> Vec<(f32, f32)> {
    let n = positions.len();
    
    // k1: Velocities at initial positions
    let k1 = get_velocities(positions);
    
    // k2: Positions after half step with k1
    let k2_positions: Vec<(f32, f32, f32)> = positions.iter()
        .enumerate()
        .map(|(i, &(lon, lat, depth))| {
            let (u, v) = k1[i];
            (
                lon + 0.5 * dt * u,
                lat + 0.5 * dt * v,
                depth,
            )
        })
        .collect();
    let k2 = get_velocities(&k2_positions);
    
    // k3: Positions after half step with k2
    let k3_positions: Vec<(f32, f32, f32)> = positions.iter()
        .enumerate()
        .map(|(i, &(lon, lat, depth))| {
            let (u, v) = k2[i];
            (
                lon + 0.5 * dt * u,
                lat + 0.5 * dt * v,
                depth,
            )
        })
        .collect();
    let k3 = get_velocities(&k3_positions);
    
    // k4: Positions after full step with k3
    let k4_positions: Vec<(f32, f32, f32)> = positions.iter()
        .enumerate()
        .map(|(i, &(lon, lat, depth))| {
            let (u, v) = k3[i];
            (
                lon + dt * u,
                lat + dt * v,
                depth,
            )
        })
        .collect();
    let k4 = get_velocities(&k4_positions);
    
    // Weighted average and final position
    positions.iter()
        .enumerate()
        .map(|(i, &(lon, lat, _))| {
            let u = (k1[i].0 + 2.0 * k2[i].0 + 2.0 * k3[i].0 + k4[i].0) / 6.0;
            let v = (k1[i].1 + 2.0 * k2[i].1 + 2.0 * k3[i].1 + k4[i].1) / 6.0;
            (lon + dt * u, lat + dt * v)
        })
        .collect()
}