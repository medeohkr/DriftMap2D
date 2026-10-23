// projection.rs maps geographic positions into a northern polar stereographic grid

use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub(super) struct PolarStereographic {
    grid_mapping_name: String,
    straight_vertical_longitude_from_pole: f64,
    latitude_of_projection_origin: f64,
    false_northing: f64,
    false_easting: f64,
    semi_major_axis: f64,
    semi_minor_axis: f64,
    standard_parallel: f64,
}

impl PolarStereographic {
    pub(super) fn scale(&self, lat: f64) -> f64 {
        let lat = lat.min(89.999999);
        let phi = lat.to_radians();
        let e2 = 1.0 - (self.semi_minor_axis / self.semi_major_axis).powi(2);
        let m = phi.cos() / (1.0 - e2 * phi.sin().powi(2)).sqrt();
        let (_, y) = self
            .project(self.straight_vertical_longitude_from_pole, lat)
            .unwrap();
        (self.false_northing - y).abs() / (self.semi_major_axis * m)
    }

    pub(super) fn validate(&self) -> Result<(), String> {
        let values = [
            self.straight_vertical_longitude_from_pole,
            self.latitude_of_projection_origin,
            self.false_northing,
            self.false_easting,
            self.semi_major_axis,
            self.semi_minor_axis,
            self.standard_parallel,
        ];
        if !values.iter().all(|v| v.is_finite())
            || self.grid_mapping_name != "polar_stereographic"
            || self.latitude_of_projection_origin != 90.0
            || self.standard_parallel <= 0.0
            || self.standard_parallel >= 90.0
            || self.semi_minor_axis <= 0.0
            || self.semi_minor_axis > self.semi_major_axis
        {
            return Err("unsupported polar stereographic projection".into());
        }
        Ok(())
    }

    pub(super) fn project(&self, lon: f64, lat: f64) -> Option<(f64, f64)> {
        if !lon.is_finite() || !lat.is_finite() || lat <= -90.0 || lat > 90.0 {
            return None;
        }
        let e = (1.0 - (self.semi_minor_axis / self.semi_major_axis).powi(2)).sqrt();
        let t = |phi: f64| {
            (std::f64::consts::FRAC_PI_4 - phi / 2.0).tan()
                / ((1.0 - e * phi.sin()) / (1.0 + e * phi.sin())).powf(e / 2.0)
        };
        let phi_c = self.standard_parallel.to_radians();
        let m_c = phi_c.cos() / (1.0 - e * e * phi_c.sin().powi(2)).sqrt();
        let rho = self.semi_major_axis * m_c * t(lat.to_radians()) / t(phi_c);
        let angle = (lon - self.straight_vertical_longitude_from_pole).to_radians();
        Some((
            self.false_easting + rho * angle.sin(),
            self.false_northing - rho * angle.cos(),
        ))
    }
}
