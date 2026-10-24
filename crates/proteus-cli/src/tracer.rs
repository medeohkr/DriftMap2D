use anyhow::{bail, ensure, Context, Result};
use proteus_core::tracers::{GenericProperties, LeewayProperties, OilPropertiesJson};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

/// resolve catalog selections while accepting existing full tracer properties
pub fn resolve(kind: &str, text: &str, config_dir: &Path, catalog_dir: Option<&Path>) -> Result<String> {
    let value: Value = serde_json::from_str(text).context("invalid tracer_json")?;
    let spec = value.as_object().context("tracer_json must be an object")?;
    let selector = spec.contains_key("oil_id") || spec.contains_key("object_id");
    let mut properties = if selector {
        ensure!(spec.keys().all(|k| matches!(k.as_str(), "oil_id" | "object_id" | "overrides")),
            "catalog selections accept only oil_id or object_id and overrides");
        ensure!(!(spec.contains_key("oil_id") && spec.contains_key("object_id")),
            "select either oil_id or object_id");
        let (field, filename) = match kind {
            "oil" => ("oil_id", "oil_catalog.json"),
            "sar" => ("object_id", "object_catalog.json"),
            _ => bail!("catalog selection requires tracer_type oil or sar"),
        };
        let id = spec.get(field).and_then(Value::as_str)
            .with_context(|| format!("{field} must be a string for tracer_type {kind}"))?;
        let path = locate_catalog(filename, config_dir, catalog_dir)?;
        let catalog: Value = serde_json::from_slice(&std::fs::read(&path)
            .with_context(|| format!("failed to read {}", path.display()))?)
            .with_context(|| format!("invalid catalog {}", path.display()))?;
        let properties = if kind == "oil" { oil(&catalog, id)? } else { object(&catalog, id)? };
        log::info!("resolved {field}={id} from {}", path.display());
        properties
    } else {
        ensure!(!spec.contains_key("overrides"), "overrides requires oil_id or object_id");
        spec.clone()
    };
    if let Some(overrides) = spec.get("overrides") {
        for (key, value) in overrides.as_object().context("overrides must be an object")? {
            ensure!(properties.contains_key(key), "unknown {kind} override {key}");
            properties.insert(key.clone(), value.clone());
        }
    }
    let value = Value::Object(properties);
    validate(kind, &value).context("invalid tracer properties")?;
    Ok(serde_json::to_string(&value)?)
}

/// find project catalogs without depending on the working directory
fn locate_catalog(filename: &str, config_dir: &Path, explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(dir) = explicit {
        return Ok(config_dir.join(dir).join(filename));
    }
    for ancestor in config_dir.ancestors() {
        let path = ancestor.join("catalogs").join(filename);
        if path.is_file() { return Ok(path); }
    }
    let executable = std::env::current_exe()?;
    for ancestor in executable.parent().unwrap().ancestors() {
        let path = ancestor.join("catalogs").join(filename);
        if path.is_file() { return Ok(path); }
    }
    bail!("{filename} not found in catalogs directories near the config or executable. run python scripts/build_catalogs.py or set catalog_dir")
}

fn oil(catalog: &Value, id: &str) -> Result<Map<String, Value>> {
    let oils = catalog["oils"].as_array().context("oil catalog is missing oils")?;
    let matches: Vec<_> = oils.iter().filter(|oil| oil["oil_id"].as_str() == Some(id)).collect();
    ensure!(matches.len() == 1, "oil_id {id} must match exactly one catalog oil, found {}", matches.len());
    let record = matches[0];
    let mut properties = Map::new();
    for key in ["product_type", "density_kgm3", "dynamic_viscosity_cp", "interfacial_tension_n_m",
        "sara_mass_fractions", "boiling_points_c", "molecular_weights_kg_mol", "component_mass_fractions",
        "bullwinkle_fraction", "bulltime", "emulsion_water_fraction_max"] {
        properties.insert(key.into(), record.get(key).with_context(|| format!("catalog oil {id} is missing {key}"))?.clone());
    }
    properties.insert("api".into(), record.get("api_gravity").context("catalog oil is missing api_gravity")?.clone());
    properties.insert("emulsion_water_fraction_max_by_temperature".into(),
        record.get("emulsion_water_fraction_max_by_temperature").cloned().unwrap_or(json!([])));
    properties.insert("wind_factor".into(), json!(0.035));
    properties.insert("wind_deflection".into(), Value::Null);
    Ok(properties)
}

fn object(catalog: &Value, id: &str) -> Result<Map<String, Value>> {
    let objects = catalog["objects"].as_object().context("object catalog is missing objects")?;
    let record = if let Some(record) = objects.get(id) {
        record
    } else {
        let matches: Vec<_> = objects.values().filter(|r| r["name"].as_str() == Some(id)).collect();
        ensure!(matches.len() == 1, "unknown or ambiguous object_id {id}");
        matches[0]
    };
    let mut properties = json!({
        "jibe_probability": 0.04, "random_orientation": true, "capsizing": false,
        "capsize_threshold": 30.0, "capsize_fraction": 0.4, "capsize_sigma": 5.0,
    }).as_object().unwrap().clone();
    for key in ["downwind", "right", "left"] {
        properties.insert(key.into(), record.get(key).with_context(|| format!("catalog object {id} is missing {key}"))?.clone());
    }
    Ok(properties)
}

fn validate(kind: &str, value: &Value) -> Result<()> {
    match kind {
        "generic" => { let _: GenericProperties = serde_json::from_value(value.clone())?; }
        "sar" => {
            let props: LeewayProperties = serde_json::from_value(value.clone())?;
            ensure!((0.0..=1.0).contains(&props.jibe_probability), "jibe_probability must be a fraction from 0 to 1");
            ensure!((0.0..=1.0).contains(&props.capsize_fraction), "capsize_fraction must be a fraction from 0 to 1");
        }
        "oil" => {
            let props: OilPropertiesJson = serde_json::from_value(value.clone())?;
            let n = props.component_mass_fractions.len();
            ensure!(n > 0 && props.boiling_points_c.len() == n && props.molecular_weights_kg_mol.len() == n,
                "oil component arrays must have equal nonzero lengths");
            ensure!(!props.density_kgm3.is_empty() && !props.dynamic_viscosity_cp.is_empty()
                && !props.interfacial_tension_n_m.is_empty(), "oil temperature tables must not be empty");
            if let Some(cap) = props.emulsion_water_fraction_max {
                ensure!((0.0..=0.9).contains(&cap), "invalid maximum water fraction");
            }
            let table = props.emulsion_water_fraction_max_by_temperature;
            ensure!(table.iter().all(|&(cap, temp)| (0.0..=0.9).contains(&cap) && temp.is_finite())
                && table.windows(2).all(|p| p[0].1 < p[1].1), "invalid water fraction temperature limits");
        }
        _ => bail!("unknown tracer_type {kind}"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/cli-tracer-tests")
                .join(format!("{}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed)));
            std::fs::create_dir_all(root.join("catalogs")).unwrap();
            std::fs::create_dir_all(root.join("scenarios")).unwrap();
            let mut record = raw_oil();
            record["oil_id"] = json!("GN00006");
            record["api_gravity"] = record["api"].take();
            let oils = json!({"oils": [record]});
            std::fs::write(root.join("catalogs/oil_catalog.json"), serde_json::to_vec(&oils).unwrap()).unwrap();
            let objects = json!({"objects": {"PIW-1": {"name": "Person in water", "downwind": [0.96, 0, 12],
                "right": [0.54, 0, 9.4], "left": [-0.54, 0, 9.4]}}});
            std::fs::write(root.join("catalogs/object_catalog.json"), serde_json::to_vec(&objects).unwrap()).unwrap();
            Self(root)
        }

        fn resolve(&self, kind: &str, spec: Value) -> Result<Value> {
            Ok(serde_json::from_str(&resolve(kind, &spec.to_string(), &self.0.join("scenarios"), None)?)?)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let target = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/cli-tracer-tests");
            assert!(self.0.starts_with(target));
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn raw_oil() -> Value {
        json!({"api": 30, "product_type": "Crude Oil NOS", "density_kgm3": [[850, 15]],
            "dynamic_viscosity_cp": [[10, 15]], "interfacial_tension_n_m": [[0.03, 15]],
            "sara_mass_fractions": {"asphaltenes": 0.03}, "boiling_points_c": [100, 400],
            "molecular_weights_kg_mol": [0.1, 0.3], "component_mass_fractions": [0.5, 0.5],
            "bullwinkle_fraction": 0.2, "bulltime": -999, "emulsion_water_fraction_max": 0.9,
            "emulsion_water_fraction_max_by_temperature": [[0.46, 5], [0.51, 15]],
            "wind_factor": 0.035, "wind_deflection": null})
    }

    #[test]
    fn oil_selector_preserves_properties_and_applies_overrides() {
        let fixture = Fixture::new();
        let resolved = fixture.resolve("oil", json!({"oil_id": "GN00006", "overrides": {"wind_factor": 0.02}})).unwrap();
        assert_eq!(resolved["api"], 30);
        assert_eq!(resolved["wind_factor"], 0.02);
        assert_eq!(resolved["wind_deflection"], Value::Null);
        assert_eq!(resolved["bulltime"], -999);
        assert_eq!(resolved["emulsion_water_fraction_max_by_temperature"], raw_oil()["emulsion_water_fraction_max_by_temperature"]);
    }

    #[test]
    fn object_selector_accepts_code_and_name_with_fraction_defaults() {
        let fixture = Fixture::new();
        let by_code = fixture.resolve("sar", json!({"object_id": "PIW-1"})).unwrap();
        let by_name = fixture.resolve("sar", json!({"object_id": "Person in water"})).unwrap();
        assert_eq!(by_code, by_name);
        assert_eq!(by_code["jibe_probability"], 0.04);
        assert_eq!(by_code["capsize_fraction"], 0.4);
        let overridden = fixture.resolve("sar", json!({"object_id": "PIW-1", "overrides": {"random_orientation": false}})).unwrap();
        assert_eq!(overridden["random_orientation"], false);
    }

    #[test]
    fn legacy_full_properties_do_not_require_catalogs() {
        let dir = Path::new("missing");
        let generic = r#"{"wind_factor":0.02,"wind_deflection":0}"#;
        assert!(resolve("generic", generic, dir, None).is_ok());
        assert!(resolve("oil", &raw_oil().to_string(), dir, None).is_ok());
    }

    #[test]
    fn relative_catalog_directory_is_relative_to_the_config() {
        let fixture = Fixture::new();
        let resolved = resolve("sar", r#"{"object_id":"PIW-1"}"#, &fixture.0.join("scenarios"), Some(Path::new("../catalogs"))).unwrap();
        assert!(resolved.contains("downwind"));
    }

    #[test]
    fn invalid_ids_selectors_and_overrides_return_errors() {
        let fixture = Fixture::new();
        for (kind, spec) in [
            ("oil", json!({"oil_id": "missing"})),
            ("sar", json!({"object_id": "missing"})),
            ("sar", json!({"oil_id": "GN00006"})),
            ("oil", json!({"oil_id": "GN00006", "object_id": "PIW-1"})),
            ("oil", json!({"oil_id": "GN00006", "overrides": {"wind_fctor": 2}})),
            ("oil", json!({"oil_id": "GN00006", "overrides": {"wind_factor": "fast"}})),
            ("sar", json!({"object_id": "PIW-1", "overrides": {"jibe_probability": 4}})),
        ] {
            assert!(fixture.resolve(kind, spec.clone()).is_err(), "{spec}");
        }
    }
}
