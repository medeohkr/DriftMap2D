use super::OilProperties;
use super::OilData;
use super::adios::{lerp}
;
macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

const CP_TO_PAS: f32 = 1e-3; // 1000 cP = PaS
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

// adios2 estimation of y_max
fn y_max(viscosity: f32) -> f32 {
    let viscosity_pas = viscosity * CP_TO_PAS;
    if viscosity_pas > 0.050 {
        return 0.9 - 0.0952 * (viscosity_pas / 0.050).ln()
    }
    else {
        return 0.9
    }
}

// adios2 estimation helper for missing interfacial tension
fn interfacial_tension_from_api(api: f32) -> f32 {
    0.001 * (39.0 - 0.2571 * api)
}

fn water_uptake_coefficient(wind_speed: f32) -> f32 {
    6.0 * K0Y * wind_speed * wind_speed / DROP_MAX
}

fn evap_decay_constants(
    wind_speeds: &[f32],
    sst_k: &[f32],
    areas: &[f32],
    mass_components: &[f32],
    n_components: usize,
    molecular_weights: &[f32],
    boiling_points: &[f32],
    indices: &[usize],
) -> Vec<f32> {
    let mut decay = Vec::with_capacity(indices.len() * n_components);

    for &idx in indices {
        let k = mass_transport_coeff(wind_speeds[idx]);
        let sum_moles: f32 = mass_components[idx * n_components..(idx + 1) * n_components]
            .iter()
            .zip(molecular_weights)
            .map(|(&mass, &mw)| mass * mw)
            .sum();
        let factor = -(areas[idx] * k) / (GAS_CONSTANT * sst_k[idx] * sum_moles);
        let vp = vapor_pressure(boiling_points, sst_k[idx]);
        for &vp_val in &vp {
            decay.push(factor * vp_val);
        }
    }

    decay
}

fn mass_transport_coeff(wind_speed: f32) -> f32 {
    if wind_speed > 10.0 {
        0.06 * C_EVAP * wind_speed.powi(2)
    } else {
        C_EVAP * wind_speed.powf(0.78)
    }
}

fn vapor_pressure(boiling_points: &[f32], temp_k: f32) -> Vec<f32> {
    boiling_points.iter()
        .map(|&boiling_point| {
            let d_s = 8.75 + 1.987 * boiling_point.ln();
            let c_2i = 0.19 * boiling_point - 18.0;
            
            let var = 1.0 / (boiling_point - c_2i) - 1.0 / (temp_k - c_2i);
            let ln_pi_po = d_s * (boiling_point - c_2i).powi(2) /
                            (D_ZB * R_CAL * boiling_point) * var;
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
    
    // Evaporation factor
    let kv1 = (oil_viscosity_cp.sqrt() * VISC_CURVFIT_PARAM).clamp(1.0, 10.0);
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

pub fn update_evaporation(
    mass_components: &mut [f32],
    n_components: usize,
    total_initial_mass: f32,
    total_mass: &mut [f32],
    f_evap: &mut [f32],
    area: &[f32],
    wind_speeds: &[f32],
    sst_k: &[f32],
    molecular_weights: &[f32],
    boiling_points: &[f32],
    evaporating_indices: &[usize],
    dt: f32,
) {
    let decay = evap_decay_constants(
        wind_speeds,
        sst_k,
        area,
        mass_components,
        n_components,
        boiling_points,
        molecular_weights,
        evaporating_indices,
    );

    for i in 0..mass_components.len() {
        mass_components[i] = mass_components[i] * (decay[i] * dt).exp()
    }

    for i in 0..wind_speeds.len() {
        total_mass[i] = mass_components[i * n_components..(i + 1) * n_components].iter().sum();
        f_evap[i] = 1.0 - (total_mass[i] / total_initial_mass)
    }
}

pub fn update_emulsification(
    y_w: &mut f32,
    interfacial_area: &mut f32,
    viscosity: f32,
    age_since_start: f32,
    wind_speed: f32,
    dt: f32,
) -> bool {

    let y_max = y_max(viscosity);

    if y_max <= 0.0 {
        return false;
    }

    if *y_w >= y_max {
        return true;
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

    true
}

pub fn step_particle_weathering(
    particles: &mut OilData,
    oil: &OilProperties,
    wind_speeds: &[f32],
    sst_celsius: &[f32],
    dt: f32,
) {
    let n = wind_speeds.len();
    let initial_densities: Vec<f32> = (0..n).map(
        |i| lerp(&oil.density_kgm3,sst_celsius[i])
    ).collect();

    let initial_viscosities: Vec<f32> = (0..n).map(
        |i| lerp(&oil.dynamic_viscosity_cp,sst_celsius[i])
    ).collect();

    let (oil_densities, oil_viscosities): (Vec<f32>, Vec<f32>) = (0..n).map(
        |i| update_density_viscosity(
        particles.y_w[i],
        particles.f_evap[i],
        initial_densities[i],
        initial_viscosities[i],
        sea_water_density(sst_celsius[i])
    )).collect();

    let areas: Vec<f32> = (0..n).map(
        |i| particles.total_mass[i] / oil_densities[i] / 1e-3
    ).collect();

    let evaporating_indices: Vec<usize> = (0..particles.age.len())
        .filter(|&i| particles.age[i] < 24.0 * 3600.0)
        .collect();

    if evaporating_indices.len() > 0 {
        update_evaporation(
            &mut particles.mass_components,
            particles.n_components,
            particles.total_initial_mass,
            &mut particles.total_mass,
            &mut particles.f_evap,
            &areas,
            wind_speeds,
            sst_celsius,
            &oil.molecular_weights,
            &oil.boiling_points,
            &evaporating_indices,
            dt,
        );
    }
    for i in 0..n {
        let mut emulsifying = false;
        if particles.f_evap[i] >= oil.bullwinkle_fraction {
            if particles.emulsification_start_age[i] == -1.0 {
                particles.emulsification_start_age[i] = particles.age[i];
            }
            emulsifying = true;
        }

        if emulsifying {
            let age_since_start = particles.age[i] - particles.emulsification_start_age[i];
            update_emulsification(
                &mut particles.y_w[i],
                &mut particles.interfacial_area[i],
                oil_viscosities[i],
                age_since_start,
                wind_speeds[i],
                dt,
            );
        }
        // 4. Age
        particles.age[i] += dt;
    }
}