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