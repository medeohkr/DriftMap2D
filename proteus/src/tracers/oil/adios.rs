use serde::Deserialize;
// === MAIN STRUCT ===

#[derive(Debug, Clone, Deserialize)]
pub struct AdiosOil {
    pub metadata: AdiosMetadata,
    #[serde(default)]
    pub sub_samples: Vec<AdiosSubSample>,
}

// === METADATA (Name and API) ===

#[derive(Debug, Clone, Deserialize)]
pub struct AdiosMetadata {
    pub product_type: String,
    #[serde(rename = "API")]
    pub api: f32
}

// === SUBSAMPLE ===

#[derive(Debug, Clone, Deserialize)]
pub struct AdiosSubSample {
    pub metadata: AdiosSampleMetadata,
    #[serde(default)]
    pub physical_properties: Option<AdiosPhysicalProperties>,
    #[serde(default, rename = "SARA")]
    pub sara: Option<AdiosSara>,
    #[serde(default)]
    pub distillation_data: Option<AdiosDistillationData>,
}

// === SAMPLE METADATA ===

#[derive(Debug, Clone, Deserialize)]
pub struct AdiosSampleMetadata {
    pub fraction_evaporated: AdiosFraction,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdiosFraction {
    #[serde(default)]
    pub value: f32,
}

// === PHYSICAL PROPERTIES ===

#[derive(Debug, Clone, Deserialize)]
pub struct AdiosPhysicalProperties {
    #[serde(default)]
    pub densities: Vec<AdiosDensityMeasurement>,
    #[serde(default)]
    pub kinematic_viscosities: Vec<AdiosViscosityMeasurement>,
    #[serde(default)]
    pub dynamic_viscosities: Vec<AdiosViscosityMeasurement>,
    #[serde(default)]
    pub interfacial_tension_water: Vec<AdiosTensionMeasurement>,
    #[serde(default)]
    pub interfacial_tension_seawater: Vec<AdiosTensionMeasurement>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdiosDensityMeasurement {
    pub density: AdiosDensityValue,
    pub ref_temp: AdiosTemperature,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdiosDensityValue {
    #[serde(default)]
    pub value: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdiosViscosityMeasurement {
    pub viscosity: AdiosViscosityValue,
    pub ref_temp: AdiosTemperature,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdiosViscosityValue {
    #[serde(default)]
    pub value: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdiosTensionMeasurement {
    pub tension: AdiosTensionValue,
    pub ref_temp: AdiosTemperature,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdiosTensionValue {
    #[serde(default)]
    pub value: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdiosTemperature {
    #[serde(default)]
    pub value: f32,
}

// === SARA ===

#[derive(Debug, Clone, Deserialize)]
pub struct AdiosSara {
    pub saturates: Option<AdiosSaraComponent>,
    pub aromatics: Option<AdiosSaraComponent>,
    pub resins: Option<AdiosSaraComponent>,
    pub asphaltenes: Option<AdiosSaraComponent>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdiosSaraComponent {
    #[serde(default)]
    pub value: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdiosDistillationData {
    #[serde(default)]
    pub cuts: Vec<AdiosDistillationCut>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdiosDistillationCut {
    pub fraction: AdiosFraction,
    pub vapor_temp: AdiosTemperature,
}

// === HELPERS ===

impl AdiosOil {
    // get unevaporated sample
    pub fn fresh_sample(&self) -> &AdiosSubSample {
        self.sub_samples.iter().find(|s| {
            let frac = s.metadata.fraction_evaporated.value;
            frac == 0.0 || frac < 0.001 // margin for rounding errors
        }).expect("No Subsample Found")
    }
    // get the raw density measurements now, interpolate later
    pub fn densities(&self) -> Vec<(f32, f32)> {
        let props = self.fresh_sample().physical_properties.as_ref()
            .expect("Physical properties missing");

        let mut data: Vec<(f32, f32)> = props.densities.iter()
            .map(|d| (d.ref_temp.value, d.density.value * 1000.0))
            .collect();
        data.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        data
    }

    pub fn viscosities(&self) -> Vec<(f32, f32)> {
        let props = self.fresh_sample().physical_properties.as_ref()
            .expect("Physical properties missing");

        // prioritize dynamic viscosities
        if !props.dynamic_viscosities.is_empty() {
            let mut data: Vec<(f32, f32)> = props.dynamic_viscosities.iter()
                .map(|v| (v.ref_temp.value, v.viscosity.value))
                .collect();
            data.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            return data;
        }

        // fall back to kinematic viscosity
        let mut data: Vec<(f32, f32)> = props.kinematic_viscosities.iter()
            .map(|v| {
                let temp = v.ref_temp.value;
                let kinematic_cst = v.viscosity.value;
                let density_gml = lerp(&self.densities(), temp);
                (temp, kinematic_cst * density_gml)
            })
            .collect();
        data.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        data
    }

    pub fn interfacial_tension(&self) -> Vec<(f32, f32)> {
        let props = self.fresh_sample().physical_properties.as_ref()
            .expect("Physical properties missing");

        // prioritize seawater interfacial tension
        if !props.interfacial_tension_seawater.is_empty() {
            return props.interfacial_tension_seawater.iter()
                .map(|v| (v.ref_temp.value, v.tension.value))
                .collect();
        }

        // fall back to water interfacial tension
        if !props.interfacial_tension_water.is_empty() {
            return props.interfacial_tension_water.iter()
                .map(|v| (v.ref_temp.value, v.tension.value))
                .collect();
        }

        vec![(15.0, 0.001 * (39.0 - 0.2571 * self.metadata.api))]
    }
    
    // SARA fractions
    pub fn saturate_fraction(&self) -> Option<f32> {
        let sample = self.fresh_sample();
        let sara = sample.sara.as_ref()?;
        Some(sara.saturates.as_ref().map(|s| s.value / 100.0).unwrap_or(0.0))
    }

    pub fn aromatic_fraction(&self) -> Option<f32> {
        let sample = self.fresh_sample();
        let sara = sample.sara.as_ref()?;
        Some(sara.aromatics.as_ref().map(|a| a.value / 100.0).unwrap_or(0.0))
    }
    pub fn asphaltene_fraction(&self) -> Option<f32> {
        let sample = self.fresh_sample();
        let sara = sample.sara.as_ref()?;
        Some(sara.asphaltenes.as_ref().map(|a| a.value / 100.0).unwrap_or(0.0))
    }

    pub fn resin_fraction(&self) -> Option<f32> {
        let sample = self.fresh_sample();
        let sara = sample.sara.as_ref()?;
        Some(sara.resins.as_ref().map(|r| r.value / 100.0).unwrap_or(0.0))
    }

    fn estimate_resin_fraction(&self) -> f32 {
        let density = lerp(&self.densities(), 15.0);
        let viscosity = lerp(&self.viscosities(), 15.0);
        let a = 10.0 * (0.001 * density).exp();
        let b = 10.0 * (1000.0 * density * viscosity).ln();
        let f_res = 0.033 * a + 0.00087 * b - 0.74;
        f_res.clamp(0.0, 1.0)
    }

    fn estimate_asphaltene_fraction(&self) -> f32 {
        let density = lerp(&self.densities(), 15.0);
        let viscosity = lerp(&self.viscosities(), 15.0);
        let a: f32 = 10.0 * (0.001 * density).exp();
        let b: f32 = 10.0 * (1000.0 * density * viscosity).ln();
        let f_asph = 0.000014 * a.powi(3) + 0.000004 * b.powi(2) - 0.18;
        f_asph.clamp(0.0, 1.0)
    }

    pub fn sara_fractions(&self) -> (f32, f32, f32, f32) {
        let asphaltene = self.asphaltene_fraction().unwrap_or(self.estimate_asphaltene_fraction());
        let resin = self.resin_fraction().unwrap_or(self.estimate_resin_fraction());
        
        // estimate saturates and aromatics if not available
        let (sat, arom) = if let (Some(s), Some(a)) = (
            self.saturate_fraction(),
            self.aromatic_fraction(),
        ) {
            (s, a)
        } else {
            // estimate from API
            let api = self.metadata.api;
            let sat = if api > 30.0 { 0.5 } else { 0.3 };
            let arom = 1.0 - sat - resin - asphaltene;
            (sat, arom)
        };
        
        (sat, arom, resin, asphaltene)
    }

    // get vec of distillation cuts
    pub fn distillation_cuts(&self) -> Option<Vec<(f32, f32)>> {
        let cuts = self.fresh_sample().distillation_data.as_ref()?.cuts.as_slice(); // takes cuts as a slice from distillation_data ref
        Some(cuts.iter().map(|cut| {
            (cut.fraction.value / 100.0, cut.vapor_temp.value)
        }).collect())
    }

    // adios2d distillation cut estimation from api
    pub fn distillation_cuts_from_api(&self, n_cuts: usize) -> Vec<(f32, f32)> {
        let api = self.metadata.api;
        let t0 = 457.0 - 3.34 * api;
        let tg = 1357.0 - 247.7 * api.ln();
        let mut cuts = Vec::with_capacity(n_cuts);
        for i in 0..n_cuts {
            let fraction = (i + 1) as f32 / n_cuts as f32;
            let temp = t0 + (tg * (i + 1) as f32) / n_cuts as f32;
            cuts.push((fraction, temp));
        }
        cuts
    }

    pub fn molecular_weights(&self) -> Vec<f32> {
        let cuts = match self.distillation_cuts() {
            Some(c) => c,
            None => self.distillation_cuts_from_api(10)
        };
        
        let mut mw = Vec::with_capacity(cuts.len() * 4);
        
        for (_, bp_c) in &cuts {
            let bp_k = bp_c + 273.15;
            mw.push(1000.0 / saturate_mol_wt(bp_k));
            mw.push(1000.0 / aromatic_mol_wt(bp_k));
            mw.push(1000.0 / resin_mol_wt(bp_k));
            mw.push(1000.0 / asphaltene_mol_wt(bp_k));
        }
        mw
    }

    pub fn initial_mass_components(&self, total_mass: f32) -> Vec<f32> {
        let cuts = match self.distillation_cuts() {
            Some(c) => c,
            None => self.distillation_cuts_from_api(10),
        };
        
        let (f_sat, f_arom, f_res, f_asph) = self.sara_fractions();
        
        let mut components = Vec::with_capacity(cuts.len() * 4);
        
        for i in 0..cuts.len() {
            // mass fraction for this cut (cumulative difference)
            let cut_fraction = if i == 0 {
                cuts[i].0
            } else {
                cuts[i].0 - cuts[i - 1].0
            };
            let cut_mass = total_mass * cut_fraction;
            
            // distribute cut mass across SARA fractions
            components.push(cut_mass * f_sat);
            components.push(cut_mass * f_arom);
            components.push(cut_mass * f_res);
            components.push(cut_mass * f_asph);
        }

        components
    }
    
    // adios2 bullwinkle_fraction estimation
    pub fn bullwinkle_fraction(&self) -> f32 {
        let api = self.metadata.api;
        let f_asph: f32 = self.asphaltene_fraction().unwrap_or(self.estimate_asphaltene_fraction());
        let bullwinkle_fraction: f32;
        let t_g = 1356.7 - 247.36 * api.ln();
        let t_bp = 532.98 - 3.1295 * api;
        let bull_adios1 = ((483.0 - t_bp) / t_g).clamp(0.0, 0.4);

        if f_asph > 0.0 {
            bullwinkle_fraction = (0.20219 - 0.168 * (f_asph).log10()).clamp(0.0, 0.0303)
        } else if api < 26.0 {
            bullwinkle_fraction = 0.08
        } else if api > 50.0 {
            bullwinkle_fraction = 0.303
        } else {
            bullwinkle_fraction = -1.038 - 0.78935 * (1.0 / api).log10()
        }

        0.5 * (bullwinkle_fraction + bull_adios1)
    }
}

// interpolation helper
pub fn lerp(props: &[(f32, f32)], target: f32) -> f32 { 
    if target <= props[0].0 {
        return props[0].1;
    }
    if target >= props.last().unwrap().0 {
        return props.last().unwrap().1;
    }
    
    for i in 0..props.len() - 1 {
        if target >= props[i].0 && target <= props[i + 1].0 {
            let t0 = props[i].0;
            let t1 = props[i + 1].0;
            let v0 = props[i].1;
            let v1 = props[i + 1].1;
            let frac = (target - t0) / (t1 - t0);
            return v0 + frac * (v1 - v0);
        }
    }
    
    props.last().unwrap().1
}

// molecular weight for saturates from boiling point (g/mol)
fn saturate_mol_wt(boiling_point_k: f32) -> f32 {
    let t = boiling_point_k.clamp(0.1, 1069.9);
    let val = 6.98291 - (1070.0 - t).ln();
    (49.677 * val).powf(1.5)
}

// molecular weight for aromatics from boiling point (g/mol)
fn aromatic_mol_wt(boiling_point_k: f32) -> f32 {
    let t = boiling_point_k.clamp(0.1, 1014.9);
    let val = 6.911 - (1015.0 - t).ln();
    (44.504 * val).powf(1.5)
}

// fixed molecular weight (g/mol) for resins
fn resin_mol_wt(_boiling_point_k: f32) -> f32 {
    800.0
}

// foxed molecular weight (g/mol) for asphaltenes
fn asphaltene_mol_wt(_boiling_point_k: f32) -> f32 {
    1000.0
}

pub fn boiling_points(distillation_cuts: Vec<(f32, f32)>) -> Vec<f32> {
    distillation_cuts.iter().flat_map(
        |(_, bp_c)| std::iter::repeat(*bp_c + 273.15).take(4)
    ).collect()
}