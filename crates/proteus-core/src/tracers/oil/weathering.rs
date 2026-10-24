use super::OilData;
use super::OilProperties;

const K0Y: f32 = 2.024e-6; // 0.000002024
const DROP_MIN: f32 = 1.0e-6;
const DROP_MAX: f32 = 1.0e-5;
const C_EVAP: f32 = 2.5e-3; // 0.0025
const GAS_CONSTANT: f32 = 8.314;
const ATMOS_PRESSURE: f32 = 101325.0;
const D_ZB: f32 = 0.97;
const R_CAL: f32 = 1.987;
const VISC_F_REF: f32 = 0.84;
const VISC_CURVFIT_PARAM: f32 = 1.5e3;

fn lerp(props: &[(f32, f32)], target: f32) -> f32 {
    if target <= props[0].1 {
        return props[0].0;
    }
    if target >= props.last().unwrap().1 {
        return props.last().unwrap().0;
    }

    for i in 0..props.len() - 1 {
        if target >= props[i].1 && target <= props[i + 1].1 {
            let t0 = props[i].1;
            let t1 = props[i + 1].1;
            let v0 = props[i].0;
            let v1 = props[i + 1].0;
            let frac = (target - t0) / (t1 - t0);
            return v0 + frac * (v1 - v0);
        }
    }

    props.last().unwrap().1
}
// adios2 estimation of y_max
pub(super) fn estimate_water_fraction_max(viscosity_points: &[(f32, f32)]) -> f32 {
    // ADIOS fallback uses fresh-oil dynamic viscosity at 15 C, once per oil.
    let viscosity_cp = lerp(viscosity_points, 15.0);
    let viscosity_pas = (viscosity_cp * 1e-3).max(1e-12);

    let value = if viscosity_pas > 0.050 {
        0.9 - 0.0952 * (viscosity_pas / 0.050).ln()
    } else {
        0.9
    };

    value.clamp(0.0, 0.9)
}

// adios2 estimation helper for missing interfacial tension
fn _interfacial_tension_from_api(api: f32) -> f32 {
    0.001 * (39.0 - 0.2571 * api)
}

fn water_uptake_coefficient(wind_speed: f32) -> f32 {
    6.0 * K0Y * wind_speed * wind_speed / DROP_MAX
}

fn mass_transport_coeff(wind_speed: f32) -> f32 {
    if wind_speed >= 10.0 {
        0.06 * C_EVAP * wind_speed.powi(2)
    } else {
        C_EVAP * wind_speed.powf(0.78)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracers::{OilTracer, Tracer};

    fn oil(bullwinkle: f32, bulltime: f32) -> OilTracer {
        let json = serde_json::json!({
            "wind_factor": 0.0, "wind_deflection": 0.0,
            "product_type": "test", "api": 30.0,
            "density_kgm3": [[900.0, 15.0]],
            "dynamic_viscosity_cp": [[9.0, 15.0]],
            "interfacial_tension_n_m": [[0.03, 15.0]],
            "sara_mass_fractions": {"asphaltenes": 0.03},
            "boiling_points_c": [100.0, 400.0],
            "molecular_weights_kg_mol": [0.1, 0.3],
            "component_mass_fractions": [0.5, 0.5],
            "bullwinkle_fraction": bullwinkle, "bulltime": bulltime
        });
        let mut oil = OilTracer::new(&json.to_string(), 1, 1000.0);
        oil.push();
        oil
    }

    #[test]
    fn transport_uses_high_wind_branch_at_ten() {
        assert!((mass_transport_coeff(10.0) - 0.015).abs() < 1e-7);
    }

    #[test]
    fn viscosity_multiplier_uses_kinematic_units() {
        // 9 cP / 900 kg/m³ = 1e-5 m²/s; NOAA kv1 = 4.7434165.
        let (_, viscosity) = update_density_viscosity(0.0, 0.2, 900.0, 9.0, 1025.0);
        let expected = 9.0 * (4.7434165_f32 * 0.2).exp();
        assert!((viscosity - expected).abs() < 1e-4);
    }

    #[test]
    fn evaporation_continues_then_stops_after_one_day() {
        let mut oil = oil(1.0, -999.0);
        oil.step(&[0], &[5.0], &[15.0], 3600.0);
        let first_mass = oil.data.total_mass[0];
        oil.step(&[0], &[5.0], &[15.0], 3600.0);
        assert!(oil.data.total_mass[0] < first_mass);
        oil.data.age[0] = 82800.0;
        let before_boundary = oil.data.total_mass[0];
        oil.step(&[0], &[5.0], &[15.0], 3600.0);
        assert!(oil.data.total_mass[0] < before_boundary);
        let after_boundary = oil.data.total_mass[0];
        oil.step(&[0], &[5.0], &[15.0], 3600.0);
        assert_eq!(oil.data.total_mass[0], after_boundary);
    }

    #[test]
    fn emulsification_waits_until_next_step_after_evaporation_threshold() {
        let mut oil = oil(1e-6, -999.0);
        oil.step(&[0], &[5.0], &[15.0], 3600.0);
        assert!(oil.data.total_mass[0] < oil.data.total_initial_mass);
        assert!(oil.data.f_evap[0] > 1e-6);
        assert_eq!(oil.data.y_w[0], 0.0);
        oil.step(&[0], &[5.0], &[15.0], 3600.0);
        assert!(oil.data.y_w[0] > 0.0);
        assert_eq!(oil.data.emulsification_start_age[0], 0.0);
    }

    #[test]
    fn reported_fraction_uses_current_mass_and_age_advances_after_weathering() {
        let mut oil = oil(1.0, -999.0);
        oil.step(&[0], &[5.0], &[15.0], 3600.0);
        assert!(oil.data.f_evap[0] > 0.0);
        assert_eq!(oil.data.f_evap[0], 1.0 - oil.data.total_mass[0] / oil.data.total_initial_mass);
        assert_eq!(oil.data.age[0], 3600.0);
        oil.step(&[0], &[5.0], &[15.0], 3600.0);
        assert_eq!(oil.data.f_evap[0], 1.0 - oil.data.total_mass[0] / oil.data.total_initial_mass);
        assert_eq!(oil.data.age[0], 7200.0);

        // The diagnostic must refresh even when evaporation has stopped.
        oil.data.age[0] = 90000.0;
        let expected = 1.0 - oil.data.total_mass[0] / oil.data.total_initial_mass;
        oil.step(&[0], &[5.0], &[15.0], 3600.0);
        assert_eq!(oil.data.f_evap[0], expected);
    }

    #[test]
    fn emulsification_requires_positive_threshold_or_reached_bulltime() {
        let mut disabled = oil(0.0, -999.0);
        disabled.step(&[0], &[5.0], &[15.0], 3600.0);
        assert_eq!(disabled.data.y_w[0], 0.0);

        let mut timed = oil(1.0, 3600.0);
        timed.step(&[0], &[5.0], &[15.0], 3600.0);
        assert_eq!(timed.data.y_w[0], 0.0);
        timed.step(&[0], &[5.0], &[15.0], 3600.0);
        assert!(timed.data.y_w[0] > 0.0);
        assert_eq!(timed.data.emulsification_start_age[0], 3600.0);
    }

    #[test]
    fn configured_water_cap_is_constant_and_respected() {
        let mut oil = oil(0.0, 0.0);
        oil.properties.emulsion_water_fraction_max = 0.8;
        oil.properties.asphaltenes_frac = 0.0;
        for _ in 0..48 {
            oil.step(&[0], &[10.0], &[15.0], 3600.0);
        }
        assert!(oil.data.y_w[0] > 0.7);
        assert!(oil.data.y_w[0] <= 0.8);
        assert_eq!(oil.properties.emulsion_water_fraction_max, 0.8);
    }

    #[test]
    fn zero_water_cap_disables_emulsification() {
        let mut oil = oil(0.0, 0.0);
        oil.properties.emulsion_water_fraction_max = 0.0;
        oil.step(&[0], &[10.0], &[15.0], 3600.0);
        assert_eq!(oil.data.y_w[0], 0.0);
        assert_eq!(oil.data.interfacial_area[0], 0.0);
    }

    #[test]
    fn legacy_water_cap_uses_fresh_oil_at_fifteen_celsius() {
        let oil = oil(0.0, 0.0);
        assert_eq!(oil.properties.emulsion_water_fraction_max, 0.9);
        let cap = estimate_water_fraction_max(&[(50.0, 10.0), (150.0, 20.0)]);
        let expected = 0.9 - 0.0952 * 2.0_f32.ln();
        assert!((cap - expected).abs() < 1e-6);
    }

    #[test]
    fn norwegian_water_cap_interpolates_and_clamps_at_endpoints() {
        for (temperature, cap) in [(0.0, 0.46), (10.0, 0.485), (20.0, 0.51)] {
            let mut oil = oil(0.0, 0.0);
            oil.properties.emulsion_water_fraction_max_by_temperature = vec![(0.46, 5.0), (0.51, 15.0)];
            for _ in 0..72 {
                oil.step(&[0], &[20.0], &[temperature], 3600.0);
            }
            assert!((oil.data.y_w[0] - cap).abs() < 1e-6);
        }
    }

    #[test]
    fn norwegian_cap_only_overrides_when_all_samples_are_below_gnome() {
        let mut oil = oil(0.0, 0.0);
        oil.push();
        oil.properties.emulsion_water_fraction_max = 0.5;
        oil.properties.emulsion_water_fraction_max_by_temperature = vec![(0.4, 5.0), (0.6, 15.0)];
        for _ in 0..72 {
            oil.step(&[0, 1], &[20.0, 20.0], &[5.0, 15.0], 3600.0);
        }
        assert!((oil.data.y_w[0] - 0.5).abs() < 1e-6);
        assert!((oil.data.y_w[1] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn cooling_reduces_an_existing_emulsion_to_the_new_cap() {
        let mut water = 0.7;
        let mut area = 6.0 / DROP_MIN;
        update_emulsification(&mut water, &mut area, 0.46, 3600.0, 10.0, 3600.0);
        assert_eq!(water, 0.46);
    }
}

fn vapor_pressure(boiling_points: &[f32], sst_celsius: f32) -> Vec<f32> {
    boiling_points
        .iter()
        .map(|&boiling_point| {
            let d_s = 8.75 + 1.987 * boiling_point.ln();
            let c_2i = 0.19 * boiling_point - 18.0;

            let var = 1.0 / (boiling_point - c_2i) - 1.0 / (sst_celsius + 273.15 - c_2i);
            let ln_pi_po =
                d_s * (boiling_point - c_2i).powi(2) / (D_ZB * R_CAL * boiling_point) * var;
            ln_pi_po.exp() * ATMOS_PRESSURE
        })
        .collect()
}

pub fn update_density_viscosity(
    y_w: f32,
    f_evap: f32,
    oil_density_kgm3: f32,
    oil_viscosity_cp: f32,
    sea_water_density: f32,
) -> (f32, f32) {
    // Emulsion density (weighted average)
    let density = y_w * sea_water_density + (1.0 - y_w) * oil_density_kgm3;

    // The NOAA coefficient expects kinematic viscosity in m²/s, not cP.
    let kinematic_viscosity = oil_viscosity_cp * 1e-3 / oil_density_kgm3;
    let kv1 = (kinematic_viscosity.sqrt() * VISC_CURVFIT_PARAM).clamp(1.0, 10.0);
    let evap_factor = (kv1 * f_evap).exp();

    // Emulsification factor (Mooney equation)
    let fw_d_fref = y_w / VISC_F_REF;
    let emul_factor = if (1.187 - fw_d_fref) > 0.0 {
        (1.0 + (fw_d_fref / (1.187 - fw_d_fref))).powf(2.49)
    } else {
        1.0 // Shouldn't happen in practice (y_w < 0.84)
    };

    let viscosity = oil_viscosity_cp * evap_factor * emul_factor;

    (density, viscosity)
}

pub fn sea_water_density(sst_celsius: f32) -> f32 {
    1025.0 - 0.2 * (sst_celsius - 15.0)
}

fn evap_decay_constants(
    wind_speeds: &[f32],
    sst_celsius: &[f32],
    areas: &[f32],
    mass_components: &[f32],
    n_components: usize,
    molecular_weights: &[f32],
    boiling_points: &[f32],
    evaporating_indices: &[(usize, usize)],
) -> Vec<f32> {
    let mut decay = Vec::with_capacity(evaporating_indices.len() * n_components);

    for &(pos, idx) in evaporating_indices.iter() {
        let k = mass_transport_coeff(wind_speeds[pos]);
        let sum_moles: f32 = mass_components[(idx * n_components)..((idx + 1) * n_components)]
            .iter()
            .zip(molecular_weights)
            .map(|(&mass, &mw)| mass / mw)
            .sum();
        let factor = -(areas[pos] * k) / (GAS_CONSTANT * (sst_celsius[pos] + 273.15) * sum_moles);
        let vp = vapor_pressure(boiling_points, sst_celsius[pos]);
        for &vp_val in &vp {
            decay.push(factor * vp_val);
        }
    }

    decay
}

pub fn update_evaporation(
    mass_components: &mut [f32],
    n_components: usize,
    total_mass: &mut [f32],
    area: &[f32],
    wind_speeds: &[f32],
    sst_celsius: &[f32],
    molecular_weights: &[f32],
    boiling_points: &[f32],
    evaporating_indices: &[(usize, usize)],
    dt: f32,
) {
    let decay = evap_decay_constants(
        wind_speeds,
        sst_celsius,
        area,
        mass_components,
        n_components,
        molecular_weights,
        boiling_points,
        evaporating_indices,
    );

    for (i, (_, idx)) in evaporating_indices.iter().enumerate() {
        for j in 0..n_components {
            let new_component =
                mass_components[idx * n_components + j] * (decay[i * n_components + j] * dt).exp();
            mass_components[idx * n_components + j] = new_component;
        }
    }

    for &(_, idx) in evaporating_indices.iter() {
        total_mass[idx] = mass_components[idx * n_components..(idx + 1) * n_components]
            .iter()
            .sum();
    }
}

pub fn update_emulsification(
    y_w: &mut f32,
    interfacial_area: &mut f32,
    y_max: f32,
    age_since_start: f32,
    wind_speed: f32,
    dt: f32,
) {
    if y_max <= 0.0 {
        return;
    }

    // maximum interfacial area
    let s_max = (6.0 / DROP_MIN) * (y_max / (1.0 - y_max));

    // water uptake coefficient
    let k_emul = water_uptake_coefficient(wind_speed);

    // update interfacial area
    let area_increment = k_emul * dt * (-k_emul / s_max * age_since_start).exp();
    *interfacial_area += area_increment;
    *interfacial_area = interfacial_area.min(s_max);

    // update water fraction from interfacial area
    *y_w = (*interfacial_area * DROP_MAX) / (6.0 + *interfacial_area * DROP_MAX);
    *y_w = y_w.min(y_max);
}

pub fn step_particle_weathering(
    particles: &mut OilData,
    indices: &[usize],
    oil: &OilProperties,
    wind_speeds: &[f32],
    sst_celsius: &[f32],
    dt: f32,
) {
    let n = indices.len();

    // Use the start-of-step fraction for viscosity and emulsification.
    for &idx in indices {
        particles.f_evap[idx] =
            1.0 - particles.total_mass[idx] / particles.total_initial_mass;
    }

    let initial_densities: Vec<f32> = (0..n)
        .map(|pos| lerp(&oil.density_kgm3, sst_celsius[pos]))
        .collect();

    let initial_viscosities: Vec<f32> = (0..n)
        .map(|pos| lerp(&oil.dynamic_viscosity_cp, sst_celsius[pos]))
        .collect();

    let oil_densities: Vec<f32> = indices
        .iter()
        .enumerate()
        .map(|(pos, &idx)| {
            update_density_viscosity(
                particles.y_w[idx],
                particles.f_evap[idx],
                initial_densities[pos],
                initial_viscosities[pos],
                sea_water_density(sst_celsius[pos]),
            ).0
        })
        .collect();

    let areas: Vec<f32> = indices
        .iter()
        .enumerate()
        .map(|(pos, &idx)| particles.total_mass[idx] / oil_densities[pos] / 1e-3)
        .collect();

    let evaporating_indices: Vec<(usize, usize)> = indices
        .iter()
        .enumerate()
        .filter(|(_, &idx)| particles.age[idx] < 86400.0)
        .map(|(pos, &idx)| (pos, idx))
        .collect();

    // OpenOil evaluates emulsification with the start-of-step fraction too.
    let emulsifying: Vec<bool> = indices
        .iter()
        .map(|&idx| {
            (oil.bullwinkle_fraction > 0.0
                && particles.f_evap[idx] >= oil.bullwinkle_fraction)
                || (oil.bulltime >= 0.0 && particles.age[idx] >= oil.bulltime)
        })
        .collect();

    if !evaporating_indices.is_empty() {
        update_evaporation(
            &mut particles.mass_components,
            particles.n_components,
            &mut particles.total_mass,
            &areas,
            wind_speeds,
            sst_celsius,
            &oil.molecular_weights,
            &oil.boiling_points,
            &evaporating_indices,
            dt,
        );
    }

    // OpenOil applies the Norwegian limits only when all emulsifying samples are below GNOME's limit
    let use_temperature_limits = !oil.emulsion_water_fraction_max_by_temperature.is_empty()
        && emulsifying.iter().enumerate().filter(|(_, active)| **active).all(|(pos, _)|
            lerp(&oil.emulsion_water_fraction_max_by_temperature, sst_celsius[pos])
                < oil.emulsion_water_fraction_max);

    for (pos, &idx) in indices.iter().enumerate() {
        if emulsifying[pos] {
            if particles.emulsification_start_age[idx] == -1.0 {
                // OpenOil uses prescribed bulltime when positive, otherwise
                // the element's bulltime (zero for a fresh release).
                particles.emulsification_start_age[idx] = oil.bulltime.max(0.0);
            }
            let age_since_start = particles.age[idx] - particles.emulsification_start_age[idx];
            update_emulsification(
                &mut particles.y_w[idx],
                &mut particles.interfacial_area[idx],
                if use_temperature_limits {
                    lerp(&oil.emulsion_water_fraction_max_by_temperature, sst_celsius[pos])
                } else {
                    oil.emulsion_water_fraction_max
                },
                age_since_start,
                wind_speeds[pos],
                dt,
            );
        }

        // Refresh the public diagnostic from end-of-step masses. Both CLI and
        // WASM report this value; internal onset decisions used the old value.
        particles.f_evap[idx] =
            1.0 - particles.total_mass[idx] / particles.total_initial_mass;
        // Age advances after both evaporation and emulsification.
        particles.age[idx] += dt;
    }
}
