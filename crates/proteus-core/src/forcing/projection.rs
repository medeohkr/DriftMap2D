// projection.rs maps geographic positions into a northern polar stereographic grid

use serde::Deserialize;
use std::cell::OnceCell;

#[derive(Clone)]
struct Constants {
    e: f64,
    e2: f64,
    a_m_c: f64,
    t_c: f64,
}

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
    #[serde(skip)]
    constants: OnceCell<Constants>,
}

impl PolarStereographic {
    fn constants(&self) -> &Constants {
        self.constants.get_or_init(|| {
            let e2 = 1.0 - (self.semi_minor_axis / self.semi_major_axis).powi(2);
            let e = e2.sqrt();
            let phi_c = self.standard_parallel.to_radians();
            let m_c = phi_c.cos() / (1.0 - e * e * phi_c.sin().powi(2)).sqrt();
            Constants {
                e,
                e2,
                a_m_c: self.semi_major_axis * m_c,
                t_c: Self::t(phi_c, e),
            }
        })
    }

    fn t(phi: f64, e: f64) -> f64 {
        (std::f64::consts::FRAC_PI_4 - phi / 2.0).tan()
            / ((1.0 - e * phi.sin()) / (1.0 + e * phi.sin())).powf(e / 2.0)
    }

    pub(super) fn scale(&self, lat: f64) -> f64 {
        let lat = lat.min(89.999999);
        let phi = lat.to_radians();
        let e2 = self.constants().e2;
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
        self.constants();
        Ok(())
    }

    pub(super) fn project(&self, lon: f64, lat: f64) -> Option<(f64, f64)> {
        if !lon.is_finite() || !lat.is_finite() || lat <= -90.0 || lat > 90.0 {
            return None;
        }
        let c = self.constants();
        let rho = c.a_m_c * Self::t(lat.to_radians(), c.e) / c.t_c;
        let angle = (lon - self.straight_vertical_longitude_from_pole).to_radians();
        Some((
            self.false_easting + rho * angle.sin(),
            self.false_northing - rho * angle.cos(),
        ))
    }
}
