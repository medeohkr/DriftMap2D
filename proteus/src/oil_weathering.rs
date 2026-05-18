// ============================================================
// oil_weathering.rs
// Evaporation and emulsification following NOAA ADIOS / OpenDrift.
// ============================================================

use crate::oil_library::{OilProperties, OilType};
use crate::particles::Particles;
macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

// ============================================================
// CONSTANTS
// ============================================================

const K0Y: f32 = 2.024e-06;
const DROP_MAX: f32 = 1.0e-5;
const C_EVAP: f32 = 0.0025;
const MOONEY_K: f32 = 1.54;
const MOONEY_SAFE_CAP: f32 = 0.63;
const H_MIN: f32 = 1.0e-4;
const H_MAX: f32 = 1.0e-3;
const RHO_WATER: f32 = 1025.0;
const C_TEMP_VAPOR: f32 = 0.05;
const C_TEMP_VISCOSITY: f32 = 0.07;
const T_REF: f32 = 15.0;
const WIND_FACTOR_FRESH: f32 = 0.03;
const WIND_FACTOR_MOUSSE: f32 = 0.005;
const C_DIFF_FRESH: f32 = 1.0;
const C_DIFF_MOUSSE: f32 = 0.1;
const T_WEEK: f32 = 604_800.0;
const Y_OVERSHOOT: f32 = 1.582;

// ============================================================
// MASS TRANSFER COEFFICIENT
// ============================================================

#[inline]
pub fn mass_transfer_coeff(wind_speed: f32) -> f32 {
    if wind_speed < 10.0 {
        C_EVAP * wind_speed.powf(0.78)
    } else {
        0.06 * C_EVAP * wind_speed * wind_speed
    }
}

// ============================================================
// TEMPERATURE HELPERS
// ============================================================

#[inline]
pub fn viscosity_at_temperature(mu_ref_cp: f32, sst: f32) -> f32 {
    mu_ref_cp * (C_TEMP_VISCOSITY * (T_REF - sst)).exp()
}

#[inline]
pub fn y_w_final_at_temperature(props: &OilProperties, sst: f32) -> f32 {
    let mu_temp = viscosity_at_temperature(props.dynamic_viscosity_cp, sst);
    let y_w = 0.16 * mu_temp.ln() + 0.15;
    y_w.min(props.y_w_final_max).min(MOONEY_SAFE_CAP)
}

// ============================================================
// PARTICLE AREA
// ============================================================

#[inline]
pub fn particle_area(mass: f32, rho_bulk: f32, mu_bulk_cp: f32, mu_initial_cp: f32) -> f32 {
    let volume = mass / rho_bulk;
    let h_min = H_MIN * (mu_bulk_cp / mu_initial_cp).sqrt();
    let h_min = h_min.clamp(H_MIN, H_MAX);
    (volume / h_min).min(1.0e6)
}

// ============================================================
// EVAPORATION
// ============================================================

pub fn update_evaporation(
    f_evap: &mut f32,
    mass: &mut f32,
    initial_mass: f32,
    props: &OilProperties,
    wind_speed: f32,
    sst: f32,
    rho_bulk: f32,
    mu_bulk_cp: f32,
    dt: f32,
) {
    if *f_evap >= props.f_evap_max {
        return;
    }

    let k_evap = mass_transfer_coeff(wind_speed);
    let area = particle_area(*mass, rho_bulk, mu_bulk_cp, props.dynamic_viscosity_cp);
    let volume = *mass / rho_bulk;
    let d_theta = k_evap * area * dt / volume.max(1.0e-12);
    let p_ratio = (C_TEMP_VAPOR * (sst - T_REF)).exp();
    let remaining = 1.0 - *f_evap / props.f_evap_max;
    let df_evap = props.henry_effective * d_theta * p_ratio * remaining;

    // DIAGNOSTIC
    // log!("evap: k={:.6} area={:.1} vol={:.6} d_theta={:.2} p_ratio={:.3} remaining={:.3} df_evap={:.6} dt={:.1}",
    //     k_evap, area, volume, d_theta, p_ratio, remaining, df_evap, dt);

    let new_f_evap = (*f_evap + df_evap).min(props.f_evap_max);
    let actual_df = new_f_evap - *f_evap;
    *f_evap = new_f_evap;
    let mass_lost = initial_mass * actual_df;
    *mass = (*mass - mass_lost).max(0.0);
}
// ============================================================
// EMULSIFICATION
// ============================================================

#[inline]
pub fn water_uptake_coefficient(wind_speed: f32) -> f32 {
    6.0 * K0Y * wind_speed * wind_speed / DROP_MAX
}
fn emulsification_rate_scale(props: &OilProperties) -> f32 {
    // Base rate * viscosity factor
    // ln(mu) captures the order-of-magnitude difference between diesel (3 cP) and IFO (25000 cP)
    let base = 1.5e-7;
    let viscosity_factor = (props.dynamic_viscosity_cp.ln() / 13.0_f32.ln()).max(0.5);
    base * viscosity_factor
}
#[inline]
pub fn update_emulsification(
    y_w: &mut f32,
    mu_bulk_cp: &mut f32,
    rho_bulk: &mut f32,
    props: &OilProperties,
    wind_speed: f32,
    sst: f32,
    f_evap: f32,
    age_seconds: f32,
    dt: f32,
) {
    let y_w_final_dynamic = y_w_final_at_temperature(props, sst);
    let y_w_max = y_w_final_dynamic.min(MOONEY_SAFE_CAP);

    // ---- Delayed onset ----
    let emul_has_started = age_seconds >= props.bulltime
        || f_evap >= props.bullwinkle_fraction;

    if !emul_has_started && *y_w <= 0.001 {
        *y_w = 0.0;
        *mu_bulk_cp = weathered_oil_viscosity(props, f_evap);
        *rho_bulk = props.density_kgm3;
        return;
    }

    // ---- Water uptake ----
    if *y_w < y_w_max && wind_speed > 0.1 && emul_has_started {
        let k_emul = water_uptake_coefficient(wind_speed) * emulsification_rate_scale(props);;
        let y_prime = Y_OVERSHOOT * y_w_final_dynamic;
        let dy_w = k_emul * (y_prime - *y_w) * dt;
        *y_w = (*y_w + dy_w).min(y_w_max);
    }

    // ---- De-emulsification (natural water loss) ----
    let k_de = de_emulsification_rate(props);
    if k_de > 0.0 && *y_w > 0.0 {
        let dy_w_de = k_de * *y_w * dt;
        *y_w = (*y_w - dy_w_de).max(0.0);
    }

    // ---- Update viscosity ----
    let mu_oil = weathered_oil_viscosity(props, f_evap);

    if *y_w > 0.001 {
        let exponent = 2.5 * *y_w / (1.0 - MOONEY_K * *y_w);
        *mu_bulk_cp = mu_oil * exponent.exp();
    } else {
        *mu_bulk_cp = mu_oil;
    }

    // ---- Update bulk density ----
    *rho_bulk = *y_w * RHO_WATER + (1.0 - *y_w) * props.density_kgm3;
}

/// Calculate de-emulsification rate constant.
/// Unstable oils lose water over time. Stable oils never drain.
#[inline]
fn de_emulsification_rate(props: &OilProperties) -> f32 {
    if props.is_emulsion_stable {
        return 0.0;
    }
    let stability_margin = (props.asphaltene_wt_pct + props.wax_wt_pct) / 5.0;
    let s_b = stability_margin.min(1.0);
    (1.0 - s_b) / T_WEEK
}
// ============================================================
// VISCOSITY HELPERS
// ============================================================

#[inline]
pub fn weathered_oil_viscosity(props: &OilProperties, f_evap: f32) -> f32 {
    props.dynamic_viscosity_cp * (props.c_evap_viscosity * f_evap).exp()
}

#[inline]
pub fn mousse_viscosity(mu_oil_cp: f32, y_w: f32) -> f32 {
    if y_w <= 0.001 {
        return mu_oil_cp;
    }
    let y_w_safe = y_w.min(MOONEY_SAFE_CAP);
    mu_oil_cp * (2.5 * y_w_safe / (1.0 - MOONEY_K * y_w_safe)).exp()
}

// ============================================================
// TRANSPORT DAMPING
// ============================================================

#[inline]
pub fn wind_drift_factor(y_w: f32, y_w_final: f32) -> f32 {
    if y_w_final <= 0.0 {
        return WIND_FACTOR_FRESH;
    }
    let progress = (y_w / y_w_final).clamp(0.0, 1.0);
    WIND_FACTOR_FRESH + progress * (WIND_FACTOR_MOUSSE - WIND_FACTOR_FRESH)
}

#[inline]
pub fn diffusion_damping(y_w: f32, y_w_final: f32) -> f32 {
    if y_w_final <= 0.0 {
        return C_DIFF_FRESH;
    }
    let progress = (y_w / y_w_final).clamp(0.0, 1.0);
    C_DIFF_FRESH + progress * (C_DIFF_MOUSSE - C_DIFF_FRESH)
}

// ============================================================
// BULK PARTICLE UPDATE
// ============================================================

#[inline]
pub fn step_particle_weathering(
    particles: &mut Particles,
    idx: usize,
    wind_speed: f32,
    sst: f32,
    initial_mass: f32,
    dt: f32,
    oil_type: OilType,
) {
    let props = oil_type.properties();

    update_evaporation(
        &mut particles.f_evap[idx],
        &mut particles.mass[idx],
        initial_mass,
        &props,
        wind_speed,
        sst,
        particles.rho_bulk[idx],
        particles.mu_bulk_cp[idx],
        dt,
    );

    update_emulsification(
        &mut particles.y_w[idx],
        &mut particles.mu_bulk_cp[idx],
        &mut particles.rho_bulk[idx],
        &props,
        wind_speed,
        sst,
        particles.f_evap[idx],
        particles.age[idx] * 86400.0,  // age in days → seconds
        dt,
    );
    // log!("Particle 0: mass={:.4}kg, f_evap={:.4}, y_w={:.4}, mu={:.1}cP",
    //     particles.mass[idx],
    //     particles.f_evap[idx],
    //     particles.y_w[idx],
    //     particles.mu_bulk_cp[idx],
    // );
}

// ============================================================
// TESTS
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mass_transfer_coeff_low_wind() {
        let k = mass_transfer_coeff(5.0);
        assert!(k > 0.008 && k < 0.009);
    }

    #[test]
    fn test_mass_transfer_coeff_high_wind() {
        let k_10 = mass_transfer_coeff(10.0);
        let k_15 = mass_transfer_coeff(15.0);
        assert!(k_10 > 0.014 && k_10 < 0.016);
        assert!(k_15 > 0.033 && k_15 < 0.034);
    }

    #[test]
    fn test_mooney_singularity_clamped() {
        let mu = mousse_viscosity(100.0, 0.7);
        assert!(mu.is_finite());
        assert!(mu > 100.0);
        assert!(mu < 1e12);
    }

    #[test]
    fn test_wind_drift_factor_fresh() {
        let factor = wind_drift_factor(0.0, 0.56);
        assert!((factor - 0.03).abs() < 1e-6);
    }

    #[test]
    fn test_wind_drift_factor_mousse() {
        let factor = wind_drift_factor(0.56, 0.56);
        assert!((factor - 0.005).abs() < 1e-6);
    }

    #[test]
    fn test_diffusion_damping_mid() {
        let factor = diffusion_damping(0.28, 0.56);
        assert!(factor > 0.5 && factor < 0.6);
    }

    #[test]
    fn test_viscosity_decreases_with_temperature() {
        let mu_cold = viscosity_at_temperature(100.0, 5.0);
        let mu_warm = viscosity_at_temperature(100.0, 25.0);
        assert!(mu_warm < mu_cold);
    }

    #[test]
    fn test_y_w_final_higher_in_cold_water() {
        use crate::oil_library::OilType;
        let props = OilType::ArabianLight.properties();
        let yw_cold = y_w_final_at_temperature(&props, 5.0);
        let yw_warm = y_w_final_at_temperature(&props, 25.0);
        // Colder water = more viscous oil = higher Y_w_final
        assert!(yw_cold > yw_warm);
    }
}