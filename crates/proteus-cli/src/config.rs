use serde::{Deserialize, Deserializer};
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct Config {
    /// Simulation start date, "YYYY-MM-DD HH:MM"
    pub start_date: String,

    /// Tracer type: "generic", "oil", or "sar"
    pub tracer_type: String,

    /// Tracer-specific config (passed through as JSON to the tracer constructor)
    #[serde(default = "default_json_object", deserialize_with = "de_string_or_object")]
    pub tracer_json: String,

    /// Releases spec (see below)
    pub releases: ReleasesSpec,

    pub particles: usize,
    pub time_step_minutes: f32,

    #[serde(default = "default_advection")]
    pub advection: String,

    #[serde(default = "default_diffusion")]
    pub diffusion: String,

    #[serde(default = "default_diffusion_coeffs")]
    pub diffusion_coeffs: Vec<f32>,

    pub steps: u32,

    #[serde(default = "default_base_url")]
    pub tile_url: String,

    #[serde(default = "default_output")]
    pub output: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ReleasesSpec {
    /// Full array of release objects, e.g. [{ "lon": -63.5, ... }]
    Inline(Vec<serde_json::Value>),
    /// Or a path to a separate releases file (relative to the config)
    Path(String),
}

fn default_json_object() -> String { "{\"wind_factor\": 0, \"wind_deflection\": 0}".into() }
fn de_string_or_object<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        // If it's already a string, keep it as-is.
        Value::String(s) => Ok(s),
        // Otherwise, re-serialize the parsed value back to a string.
        other => serde_json::to_string(&other).map_err(serde::de::Error::custom),
    }
}
fn default_advection() -> String { "rk4".into() }
fn default_diffusion() -> String { "constant".into() }
fn default_diffusion_coeffs() -> Vec<f32> { vec![0.5, 2.0] }
fn default_base_url() -> String { "tiles".into() }
fn default_output() -> String { "centroids.json".into() }