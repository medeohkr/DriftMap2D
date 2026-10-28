// interpolation.rs samples fields and expands missing grid values

use crate::utils::{bilerp, lerp};

/// fractional coordinates reused by fields on the same grid
#[derive(Clone, Copy)]
pub(super) struct GridSample {
    pub(super) output_index: usize,
    pub(super) x: f32,
    pub(super) y: f32,
}
fn sample_spatial(
    grid: &[f32],
    width: usize,
    height: usize,
    sample: &GridSample,
    nearest: bool,
) -> f32 {
    let (mut x, mut y) = (sample.x, sample.y);
    if !x.is_finite() || !y.is_finite() {
        return f32::NAN;
    }
    if nearest {
        x = x.clamp(0.0, (width - 1) as f32);
        y = y.clamp(0.0, (height - 1) as f32);
    } else if x < 0.0 || y < 0.0 || x > (width - 1) as f32 || y > (height - 1) as f32 {
        return f32::NAN;
    }
    let col = (x.floor() as usize).min(width - 2);
    let row = (y.floor() as usize).min(height - 2);
    bilerp(
        grid,
        x - col as f32,
        y - row as f32,
        row * width + col,
        width,
    )
}

/// expand nonfinite cells by one step using a snapshot of the whole slice
pub(super) fn expand_nan_neighbors(
    grid: &mut [f32],
    width: usize,
    height: usize,
    scratch: &mut Vec<f32>,
) {
    scratch.clear();
    scratch.extend_from_slice(grid);
    for row in 0..height {
        for col in 0..width {
            let idx = row * width + col;
            if scratch[idx].is_finite() {
                continue;
            }
            let mut maximum = f32::NEG_INFINITY;
            for y in row.saturating_sub(1)..=(row + 1).min(height - 1) {
                for x in col.saturating_sub(1)..=(col + 1).min(width - 1) {
                    let value = scratch[y * width + x];
                    if value.is_finite() {
                        maximum = maximum.max(value);
                    }
                }
            }
            grid[idx] = if maximum.is_finite() {
                maximum
            } else {
                f32::NAN
            };
        }
    }
}

/// preserve successful samples and retry only failures after each expansion
pub(super) fn interpolate_batch(
    grid: &mut [f32],
    width: usize,
    height: usize,
    samples: &[GridSample],
    scratch: &mut Vec<f32>,
) -> Vec<f32> {
    if width < 2 || height < 2 || grid.len() != width * height {
        return vec![f32::NAN; samples.len()];
    }
    let mut values: Vec<_> = samples
        .iter()
        .map(|sample| sample_spatial(grid, width, height, sample, false))
        .collect();
    let mut missing: Vec<_> = values
        .iter()
        .enumerate()
        .filter_map(|(i, value)| (!value.is_finite()).then_some(i))
        .collect();
    if missing.is_empty() {
        return values;
    }
    if !grid.iter().any(|v| v.is_finite()) {
        values.fill(f32::NAN);
        return values;
    }
    let initially_missing = missing.len();
    let mut rounds = 0;
    for _ in 0..10 {
        rounds += 1;
        expand_nan_neighbors(grid, width, height, scratch);
        missing.retain(|&i| {
            values[i] = sample_spatial(grid, width, height, &samples[i], true);
            !values[i].is_finite()
        });
        if missing.is_empty() {
            break;
        }
    }
    if rounds > 1 {
        log::debug!(
            "nan expansion rounds={} grid={}x{} samples={} initially_missing={} remaining={}",
            rounds,
            width,
            height,
            samples.len(),
            initially_missing,
            missing.len()
        );
    }

    values
}

/// spatially sample surface slices before interpolating between times
pub(super) fn interpolate_time_batch(
    data: &mut [f32],
    width: usize,
    height: usize,
    times: usize,
    time: f32,
    samples: &[GridSample],
    scratch: &mut Vec<f32>,
) -> Vec<f32> {
    let cells = width * height;
    if times == 0 || cells == 0 || !time.is_finite() || data.len() != times * cells {
        return vec![f32::NAN; samples.len()];
    }
    let time = time.clamp(0.0, (times - 1) as f32);
    let before = time.floor() as usize;
    let fraction = time - before as f32;
    let offset = before * cells;
    let mut values = interpolate_batch(
        &mut data[offset..offset + cells],
        width,
        height,
        samples,
        scratch,
    );
    // an exact timestamp does not require the next slice
    if fraction == 0.0 {
        return values;
    }
    let offset = (before + 1) * cells;
    let after = interpolate_batch(
        &mut data[offset..offset + cells],
        width,
        height,
        samples,
        scratch,
    );
    for (value, next) in values.iter_mut().zip(after) {
        *value = lerp(*value, next, fraction);
    }
    values
}
