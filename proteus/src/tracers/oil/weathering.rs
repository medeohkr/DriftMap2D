use super::OilTracer;
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

fn evap_decay_constant(
    wind_speed: f32,
    sea_water_temperature_k: f32,
    area: f32,
    mass_components: &[f32],
    vapor_pressures: &[f32],
    molecular_weights: &[f32],
) -> Vec<f32> {
    // mass transfer coefficient (m/s)
    let k = mass_transport_coeff(wind_speed);
    
    // convert molecular weight from g/mol to kg/mol
    let mw_kg: Vec<f32> = molecular_weights.iter()
        .map(|&mw| mw / 1000.0)
        .collect();
    
    // sum of mass components / molecular weight (total moles)
    // sum_mi_mw = Σ(m_i / M_i)
    let sum_mi_mw: f32 = mass_components.iter()
        .zip(&mw_kg)
        .map(|(&mass, &mw)| mass / mw)
        .sum();
    
    // decay constant for each component
    // decay = -(area * K) / (R * T * Σ(m_i / M_i)) * vp_i
    let factor = -(area * k) / (GAS_CONSTANT * sea_water_temperature_k * sum_mi_mw);
    
    vapor_pressures.iter()
        .map(|&vp| factor * vp)
        .collect()
}

fn mass_transport_coeff(wind_speed: f32) -> f32 {
    if wind_speed > 10.0 {
        return 0.06 * C_EVAP * wind_speed.powi(2)
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
    total_initial_mass: &f32,
    total_mass: &mut f32,
    f_evap: &mut f32,
    area: f32,
    wind_speed: f32,
    sst_k: f32,
    molecular_weights: &[f32],
    distillation_cuts: &[(f32, f32)],  // (fraction, boiling_point_celsius)
    dt: f32,
) -> f32 {
    if *total_mass <= 0.0 || area <= 0.0 || mass_components.is_empty() {
        return 0.0;
    }
    
    // compute vapor pressures for each cut on the fly
    let boiling_points_k: Vec<f32> = distillation_cuts.iter()
        .flat_map(|(_, bp_c)| std::iter::repeat(*bp_c + 273.15).take(4))//repeat each boiling point 4 times for the 4 components
        .collect();
    
    let vapor_pressures = vapor_pressure(&boiling_points_k, sst_k);
    
    // calculate decay constants for each component
    let mut decay = evap_decay_constant(
        wind_speed,
        sst_k,
        area,
        mass_components,
        &vapor_pressures,
        molecular_weights,
    );
    decay.extend(std::iter::repeat(0.0).take(4)); // extend decay vector for 4 residue components with zero decay
    
    let mut total_evaporated = 0.0;
    let mut new_total_mass = 0.0;
    
    for i in 0..mass_components.len() {
        let old_mass = mass_components[i];
        let new_mass = old_mass * (decay[i] * dt).exp();
        mass_components[i] = new_mass;
        new_total_mass += new_mass;
        total_evaporated += old_mass - new_mass;
    }
    
    *total_mass = new_total_mass;
    *f_evap = 1.0 - ( new_total_mass / total_initial_mass );
    
    total_evaporated
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
    // A(t+dt) = A(t) + k * dt * exp(-k / S_max * (age - start_time))
    let area_increment = k_emul * dt * (-k_emul / s_max * age_since_start).exp();
    *interfacial_area += area_increment;
    *interfacial_area = interfacial_area.min(s_max);
    
    // update water fraction from interfacial area
    // y_w = (A * d_max) / (6.0 + A * d_max)
    *y_w = (*interfacial_area * DROP_MAX) / (6.0 + *interfacial_area * DROP_MAX);
    *y_w = y_w.min(y_max);
    true
}

pub fn step_particle_weathering(
    particle: &mut OilData,
    oil: &OilTracer,
    wind_speed: f32,
    sst_celsius: f32,
    dt: f32,
) {  // Returns (mu_bulk_cp, rho_bulk)
    // 1. Temperature-dependent oil properties
    let initial_density = lerp(oil.density_kgm3.clone(),sst_celsius);
    let initial_viscosity = lerp(oil.dynamic_viscosity_cp.clone(),sst_celsius);

    let (oil_density, oil_viscosity) = update_density_viscosity(
        particle.y_w,
        particle.f_evap,
        initial_density,
        initial_viscosity,
        sea_water_density(sst_celsius)
    );

    let area = particle.total_mass / oil_density / 1e-3;
    // 2. Evaporation
    if particle.age < 24.0 * 3600.0 {
        update_evaporation(
            &mut particle.mass_components,
            &particle.total_initial_mass,
            &mut particle.total_mass,
            &mut particle.f_evap,
            area,
            wind_speed,
            sst_celsius + 273.15,
            &oil.molecular_weights,
            &oil.distillation_cuts,
            dt,
        );
    }

    
    // 3. Emulsification
    let mut emulsifying = false;
    if particle.f_evap >= oil.bullwinkle_fraction {
        if particle.emulsification_start_age == -1.0 {
            particle.emulsification_start_age = particle.age;
        }
        emulsifying = true;
    }
    log!("bullwinkle_fraction: {}, f_evap: {}, emulsifying: {}", oil.bullwinkle_fraction, particle.f_evap, emulsifying);
    if emulsifying {
        let age_since_start = particle.age - particle.emulsification_start_age;
        update_emulsification(
            &mut particle.y_w,
            &mut particle.interfacial_area,
            oil_viscosity,
            age_since_start,
            wind_speed,
            dt,
        );
    }
    // 4. Age
    particle.age += dt;
}