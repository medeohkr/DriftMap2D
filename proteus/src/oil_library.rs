#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OilType {
    ArabianLight,
    BonnyLight,
    IFO380,
    VenezuelanHeavy,
    MarineDiesel,
}

pub struct OilProperties {
    pub name: &'static str,
    pub api: f32,
    pub dynamic_viscosity_cp: f32,    // at ~15°C, in centipoise
    pub density_kgm3: f32,            // at ~15°C
    pub asphaltene_wt_pct: f32,
    pub wax_wt_pct: f32,
    pub y_w_final_max: f32,           // maximum water fraction for stable emulsions
    pub is_emulsion_stable: bool,
    pub f_evap_max: f32,              // maximum evaporative mass fraction
    pub c_evap_viscosity: f32,        // μ_weathered = μ_initial * exp(C * f_evap)
    pub k0y: f32,                     // ADIOS water uptake rate constant
}

impl OilType {
    pub fn properties(&self) -> OilProperties {
        match self {
            OilType::ArabianLight => OilProperties {
                name: "Arabian Light",
                api: 32.1,
                dynamic_viscosity_cp: 13.0,
                density_kgm3: 864.0,
                asphaltene_wt_pct: 3.6,
                wax_wt_pct: 2.7,
                y_w_final_max: 0.56,
                is_emulsion_stable: true,
                f_evap_max: 0.40,
                c_evap_viscosity: 7.0,
                k0y: 2.024e-06,
            },
            OilType::BonnyLight => OilProperties {
                name: "Bonny Light",
                api: 36.7,
                dynamic_viscosity_cp: 6.0,
                density_kgm3: 840.53,
                asphaltene_wt_pct: 0.8,            // estimated from literature
                wax_wt_pct: 8.0,
                y_w_final_max: 0.15,
                is_emulsion_stable: false,
                f_evap_max: 0.55,
                c_evap_viscosity: 5.0,
                k0y: 2.024e-06,
            },
            OilType::IFO380 => OilProperties {
                name: "IFO 380 LS",
                api: 11.3,
                dynamic_viscosity_cp: 25_000.0,
                density_kgm3: 990.0,
                asphaltene_wt_pct: 6.6,
                wax_wt_pct: 5.8,
                y_w_final_max: 0.85,
                is_emulsion_stable: true,
                f_evap_max: 0.08,
                c_evap_viscosity: 1.5,
                k0y: 2.024e-06,
            },
            OilType::VenezuelanHeavy => OilProperties {
                name: "Bachequero Heavy",
                api: 14.0,
                dynamic_viscosity_cp: 10_000.0,
                density_kgm3: 971.66,
                asphaltene_wt_pct: 11.0,           // estimated: Venezuelan heavy range 9–14%
                wax_wt_pct: 2.0,                   // estimated: low wax typical for heavy Venezuelan
                y_w_final_max: 0.80,
                is_emulsion_stable: true,
                f_evap_max: 0.06,
                c_evap_viscosity: 2.0,
                k0y: 2.024e-06,
            },
            OilType::MarineDiesel => OilProperties {
                name: "Marine Diesel",
                api: 36.4,
                dynamic_viscosity_cp: 3.5,
                density_kgm3: 842.0,
                asphaltene_wt_pct: 0.0,
                wax_wt_pct: 0.05,
                y_w_final_max: 0.0,
                is_emulsion_stable: false,
                f_evap_max: 0.80,
                c_evap_viscosity: 3.0,
                k0y: 2.024e-06,
            },
        }
    }
}