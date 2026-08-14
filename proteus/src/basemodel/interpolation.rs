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

/// Linear interpolation between two values
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + t * (b - a)
}