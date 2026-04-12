//! Protobuf schema parsing, canonical form, and fingerprinting.

use sha2::{Digest, Sha256};

use crate::error::KoraError;

// -- Functions --

/// Parse a Protobuf `.proto` definition and compute its canonical form and SHA-256 fingerprint.
///
/// Validates the input by parsing it with `protox_parse`. Computes a canonical
/// form by normalizing whitespace, and a SHA-256 fingerprint of the canonical form.
///
/// # Errors
///
/// Returns `KoraError::InvalidSchema` when the input is not valid Protobuf syntax.
pub fn parse(raw: &str) -> Result<(String, String), KoraError> {
    protox_parse::parse("schema.proto", raw)
        .map_err(|e| KoraError::InvalidSchema(e.to_string()))?;

    let canonical = canonical_proto(raw);

    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    let fingerprint = format!("{:x}", hasher.finalize());

    Ok((canonical, fingerprint))
}

/// Normalize a proto definition into a canonical form.
///
/// Trims whitespace and collapses consecutive whitespace into single spaces
/// for deterministic fingerprinting.
fn canonical_proto(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<&str>>().join(" ")
}
