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
/// One RK4 step for a single particle
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