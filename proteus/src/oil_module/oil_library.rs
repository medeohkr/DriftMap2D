// oil_library.rs
// Based on NOAA ADIOS oil database
// Values to verify against official source

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OilProperties {
    pub id: String,
    pub name: String,
    pub api_gravity: f32,          // degrees API
    pub density_kg_m3: f32,        // kg/m³ at 15°C
    pub viscosity_cp: f32,          // centipoise at 15°C
    pub windage_factor: f32,        // percent of wind speed (0.01-0.05)
    pub evaporation_rate: f32,      // fraction per day initial
    pub max_evaporation: f32,       // maximum fraction that can evaporate
    pub max_water_content: f32,     // emulsification capacity (0-1)
    pub beaching_probability: f32,  // relative stickiness (0-1)
    pub spreading_rate: f32,        // relative to medium crude (0.5-1.5)
    pub pour_point_c: f32,          // temperature where oil solidifies
    pub flash_point_c: f32,         // fire hazard temperature
}

impl OilProperties {
    pub fn evaporation_fraction(&self, days: f32) -> f32 {
        // Exponential decay model
        self.max_evaporation * (1.0 - (-self.evaporation_rate * days).exp())
    }
    
    pub fn viscosity_at_temp(&self, temp_c: f32) -> f32 {
        // Simplified temperature correction
        // Real oils have complex viscosity-temperature curves
        let temp_diff = (15.0 - temp_c).max(0.0);
        self.viscosity_cp * (1.0 + 0.1 * temp_diff)
    }
    
    pub fn density_at_temp(&self, temp_c: f32) -> f32 {
        // Thermal expansion ~0.00064 per °C
        let expansion = 0.00064 * (temp_c - 15.0);
        self.density_kg_m3 * (1.0 - expansion)
    }
    
    pub fn emulsified_viscosity(&self, water_fraction: f32) -> f32 {
        // Mooney's equation for emulsion viscosity
        let water_frac = water_fraction.min(self.max_water_content);
        self.viscosity_cp * (1.0 / (1.0 - water_frac)).powf(2.5)
    }
}

// ============= OIL LIBRARY =============

pub struct OilLibrary {
    oils: std::collections::HashMap<String, OilProperties>,
}

impl OilLibrary {
    pub fn new() -> Self {
        let mut oils = std::collections::HashMap::new();
        
        // 1. ARABIAN LIGHT (Medium Crude)
        // Source: ADIOS, Saudi Aramco
        // Global benchmark, ~30% of spills
        oils.insert("arabian_light".to_string(), OilProperties {
            id: "arabian_light".to_string(),
            name: "Arabian Light".to_string(),
            api_gravity: 33.5,
            density_kg_m3: 858.0,
            viscosity_cp: 12.0,
            windage_factor: 0.028,
            evaporation_rate: 0.050,
            max_evaporation: 0.35,
            max_water_content: 0.65,
            beaching_probability: 0.60,
            spreading_rate: 1.0,
            pour_point_c: -15.0,
            flash_point_c: 65.0,
        });
        
        // 2. BONNY LIGHT (Light Crude)
        // Source: ADIOS, Nigeria
        // Common in West Africa, US Gulf imports
        oils.insert("bonny_light".to_string(), OilProperties {
            id: "bonny_light".to_string(),
            name: "Bonny Light".to_string(),
            api_gravity: 36.2,
            density_kg_m3: 845.0,
            viscosity_cp: 5.0,
            windage_factor: 0.038,
            evaporation_rate: 0.090,
            max_evaporation: 0.60,
            max_water_content: 0.50,
            beaching_probability: 0.30,
            spreading_rate: 1.3,
            pour_point_c: -18.0,
            flash_point_c: 60.0,
        });
        
        // 3. IFO 380 (Heavy Fuel Oil / Bunker)
        // Source: ADIOS, International Bunker Industry
        // Most common shipping fuel
        oils.insert("ifo_380".to_string(), OilProperties {
            id: "ifo_380".to_string(),
            name: "IFO 380".to_string(),
            api_gravity: 20.0,
            density_kg_m3: 950.0,
            viscosity_cp: 3500.0,
            windage_factor: 0.012,
            evaporation_rate: 0.015,
            max_evaporation: 0.10,
            max_water_content: 0.80,
            beaching_probability: 0.90,
            spreading_rate: 0.5,
            pour_point_c: 15.0,
            flash_point_c: 80.0,
        });
        
        // 4. MARINE DIESEL / MGO (Refined)
        // Source: ADIOS
        // Common in small spills, fishing vessels
        oils.insert("marine_diesel".to_string(), OilProperties {
            id: "marine_diesel".to_string(),
            name: "Marine Diesel / MGO".to_string(),
            api_gravity: 38.0,
            density_kg_m3: 835.0,
            viscosity_cp: 3.0,
            windage_factor: 0.045,
            evaporation_rate: 0.15,
            max_evaporation: 0.85,
            max_water_content: 0.20,
            beaching_probability: 0.10,
            spreading_rate: 1.5,
            pour_point_c: -20.0,
            flash_point_c: 55.0,
        });
        
        // 5. VENEZUELAN HEAVY (Extra Heavy)
        // Source: ADIOS
        // Common in Americas, Orinoco belt
        oils.insert("venezuelan_heavy".to_string(), OilProperties {
            id: "venezuelan_heavy".to_string(),
            name: "Venezuelan Heavy".to_string(),
            api_gravity: 16.0,
            density_kg_m3: 960.0,
            viscosity_cp: 8000.0,
            windage_factor: 0.008,
            evaporation_rate: 0.003,
            max_evaporation: 0.05,
            max_water_content: 0.75,
            beaching_probability: 0.95,
            spreading_rate: 0.3,
            pour_point_c: 25.0,
            flash_point_c: 100.0,
        });
        
        Self { oils }
    }
    
    pub fn get(&self, id: &str) -> Option<&OilProperties> {
        self.oils.get(id)
    }
    
    pub fn get_default(&self) -> &OilProperties {
        self.get("arabian_light").unwrap()
    }
    
    pub fn list_all(&self) -> Vec<&OilProperties> {
        self.oils.values().collect()
    }
    
}
