//! Protobuf schema parsing, canonical form, fingerprinting, and compatibility.

mod diff;

use sha2::{Digest, Sha256};

use super::{CompatDirection, CompatibilityResult};
use crate::error::KoraError;

// -- Parsing --

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

// -- Compatibility --

/// Check compatibility between two Protobuf schemas.
///
/// Uses Confluent-compatible diff rules.
///
/// # Errors
///
/// Returns `KoraError::InvalidSchema` if either schema is malformed.
pub fn check_compatibility(
    new_schema: &str,
    existing_schema: &str,
    direction: CompatDirection,
) -> Result<CompatibilityResult, KoraError> {
    super::check_with_direction(new_schema, existing_schema, direction, diff::check)
}

/// Check compatibility with dependency resolution for imported types.
///
/// Dependencies are `(filename, proto_content)` pairs representing imported files.
///
/// # Errors
///
/// Returns `KoraError::InvalidSchema` if any schema or dependency is malformed.
pub fn check_compatibility_with_deps(
    new_schema: &str,
    existing_schema: &str,
    direction: CompatDirection,
    old_deps: &[(String, String)],
    new_deps: &[(String, String)],
) -> Result<CompatibilityResult, KoraError> {
    super::check_with_direction(new_schema, existing_schema, direction, |old, new| {
        diff::check_with_deps(old, new, old_deps, new_deps)
    })
}

// -- Helpers --

/// Normalize a proto definition into a canonical form.
fn canonical_proto(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<&str>>().join(" ")
}
