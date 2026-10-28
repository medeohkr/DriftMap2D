// cache.rs groups surface fields that share a spatial grid and timeline

/// field arrays stored in time, row, column order
pub struct GridData<const N: usize> {
    pub width: usize,
    pub height: usize,
    pub sample_count: usize,
    pub fields: [Vec<f32>; N],
}

impl<const N: usize> GridData<N> {
    pub(super) fn sample_field(
        &mut self,
        field: usize,
        time: f32,
        samples: &[super::interpolation::GridSample],
        scratch: &mut Vec<f32>,
    ) -> Vec<f32> {
        super::interpolation::interpolate_time_batch(
            &mut self.fields[field],
            self.width,
            self.height,
            self.sample_count,
            time,
            samples,
            scratch,
        )
    }

    pub(super) fn new(width: usize, height: usize, sample_count: usize) -> Result<Self, String> {
        if width < 2 || height < 2 || sample_count == 0 {
            return Err("invalid surface grid dimensions".into());
        }
        let count = super::format::checked_product(&[width, height, sample_count])?;
        Ok(Self {
            width,
            height,
            sample_count,
            fields: std::array::from_fn(|_| Vec::with_capacity(count)),
        })
    }
}
