//! JSON Schema parsing, canonical form, and fingerprinting.

use sha2::{Digest, Sha256};

use crate::error::KoraError;

// -- Functions --

/// Parse a JSON Schema string and compute its canonical form and SHA-256 fingerprint.
///
/// Validates the input is a valid JSON Schema using meta-validation,
/// computes a deterministic canonical form (sorted keys), and generates
/// a SHA-256 fingerprint of the canonical form.
///
/// # Errors
///
/// Returns `KoraError::InvalidSchema` when the input is not valid JSON
/// or not a valid JSON Schema.
pub fn parse(raw: &str) -> Result<(String, String), KoraError> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| KoraError::InvalidSchema(e.to_string()))?;

    if !value.is_object() {
        return Err(KoraError::InvalidSchema(
            "JSON Schema must be a JSON object".to_string(),
        ));
    }

    if !jsonschema::meta::is_valid(&value) {
        return Err(KoraError::InvalidSchema(
            "Invalid JSON Schema".to_string(),
        ));
    }

    let canonical = canonical_json(&value);

    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    let fingerprint = format!("{:x}", hasher.finalize());

    Ok((canonical, fingerprint))
}

/// Produce a deterministic JSON string with sorted object keys.
fn canonical_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let entries: Vec<String> = keys
                .iter()
                .map(|k| format!("{}:{}", serde_json::to_string(k).unwrap_or_default(), canonical_json(&map[*k])))
                .collect();
            format!("{{{}}}", entries.join(","))
        }
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(canonical_json).collect();
            format!("[{}]", items.join(","))
        }
        _ => serde_json::to_string(value).unwrap_or_default(),
    }
}
