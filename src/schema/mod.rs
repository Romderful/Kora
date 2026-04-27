//! Schema format handling — parsing, canonical form, fingerprinting, and compatibility.

pub mod avro;
pub mod json_schema;
pub mod protobuf;

use sha2::{Digest, Sha256};

use crate::error::KoraError;

// -- Types --

/// Supported schema formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaFormat {
    /// Apache Avro schema format.
    Avro,
    /// JSON Schema format.
    Json,
    /// Protocol Buffers format.
    Protobuf,
}

/// Parsed and validated schema with computed metadata.
#[derive(Debug)]
pub struct ParsedSchema {
    /// The canonical form of the schema (for deduplication).
    pub canonical_form: String,
    /// Hex-encoded fingerprint of the canonical form (Rabin for Avro, SHA-256 for JSON/Protobuf).
    pub fingerprint: String,
    /// Hex-encoded SHA-256 fingerprint of the raw schema text (for non-normalized dedup).
    pub raw_fingerprint: String,
}

/// Compatibility check direction resolved from the configured mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatDirection {
    /// New schema must read old data.
    Backward,
    /// Old schema must read new data.
    Forward,
    /// Both directions.
    Full,
    /// No check — always compatible.
    None,
}

impl CompatDirection {
    /// Resolve from a compatibility level string (e.g. `"BACKWARD_TRANSITIVE"` → `Backward`).
    /// Transitive vs non-transitive affects which versions to check, not the direction.
    #[must_use]
    pub fn from_level(level: &str) -> Self {
        if level.starts_with("BACKWARD") {
            Self::Backward
        } else if level.starts_with("FORWARD") {
            Self::Forward
        } else if level.starts_with("FULL") {
            Self::Full
        } else {
            Self::None
        }
    }
}

/// Result of a schema compatibility check.
#[derive(Debug)]
pub struct CompatibilityResult {
    /// Whether the schemas are compatible under the given mode.
    pub is_compatible: bool,
    /// Incompatibility details.
    pub messages: Vec<String>,
}

// -- Parsing --

impl SchemaFormat {
    /// Known schema types advertised by the registry (matches Confluent).
    pub const KNOWN_TYPES: &[&str] = &["AVRO", "JSON", "PROTOBUF"];

    /// Parse a format string, defaulting to Avro when `None`.
    ///
    /// # Errors
    ///
    /// Returns `KoraError::InvalidSchema` for unrecognized formats.
    pub fn from_optional(schema_type: Option<&str>) -> Result<Self, KoraError> {
        match schema_type.map(str::to_ascii_uppercase).as_deref() {
            None | Some("AVRO") => Ok(Self::Avro),
            Some("JSON") => Ok(Self::Json),
            Some("PROTOBUF") => Ok(Self::Protobuf),
            Some(other) => Err(KoraError::InvalidSchema(format!(
                "Unsupported schema type: {other}"
            ))),
        }
    }

    /// Wire-format name used in database and API responses.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Avro => "AVRO",
            Self::Json => "JSON",
            Self::Protobuf => "PROTOBUF",
        }
    }
}

/// Parse and validate a raw schema string.
///
/// Each format parser computes the canonical form and its format-specific fingerprint
/// (Rabin for Avro, SHA-256 for JSON/Protobuf). This function adds the raw fingerprint
/// (SHA-256 of the unmodified input text) used for non-normalized dedup.
///
/// # Errors
///
/// Returns `KoraError::InvalidSchema` if the schema is malformed.
pub fn parse(format: SchemaFormat, raw: &str) -> Result<ParsedSchema, KoraError> {
    let (canonical_form, fingerprint) = match format {
        SchemaFormat::Avro => avro::parse(raw),
        SchemaFormat::Json => json_schema::parse(raw),
        SchemaFormat::Protobuf => protobuf::parse(raw),
    }?;

    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    let raw_fingerprint = hex::encode(hasher.finalize());

    Ok(ParsedSchema {
        canonical_form,
        fingerprint,
        raw_fingerprint,
    })
}

// -- Compatibility --

/// Check compatibility between a new schema and an existing schema.
///
/// # Errors
///
/// Returns `KoraError::InvalidSchema` if either schema is malformed.
pub fn check_compatibility(
    format: SchemaFormat,
    new_schema: &str,
    existing_schema: &str,
    direction: CompatDirection,
) -> Result<CompatibilityResult, KoraError> {
    if direction == CompatDirection::None {
        return Ok(CompatibilityResult {
            is_compatible: true,
            messages: Vec::new(),
        });
    }

    match format {
        SchemaFormat::Avro => avro::check_compatibility(new_schema, existing_schema, direction),
        SchemaFormat::Json => {
            json_schema::check_compatibility(new_schema, existing_schema, direction)
        }
        SchemaFormat::Protobuf => {
            protobuf::check_compatibility(new_schema, existing_schema, direction)
        }
    }
}

/// Async wrapper around [`check_compatibility`] that offloads CPU-bound diff
/// work to a blocking thread, preventing Tokio worker starvation under load.
///
/// # Errors
///
/// Returns `KoraError::InvalidSchema` if either schema is malformed, or
/// `KoraError::BackendDataStore` if the blocking task panics.
pub async fn check_compatibility_async(
    format: SchemaFormat,
    new_schema: String,
    existing_schema: String,
    direction: CompatDirection,
) -> Result<CompatibilityResult, KoraError> {
    tokio::task::spawn_blocking(move || {
        check_compatibility(format, &new_schema, &existing_schema, direction)
    })
    .await
    .map_err(|e| KoraError::BackendDataStore(e.to_string()))?
}

/// Run a directional compatibility check using a format-specific diff function.
///
/// The `diff_fn` compares two schemas (old, new) and returns `(is_compatible, messages)`.
/// This function handles the `BACKWARD/FORWARD/FULL` direction logic so that each
/// format only needs to provide its diff implementation.
///
/// # Errors
///
/// Propagates any error from `diff_fn`.
pub fn check_with_direction(
    new_schema: &str,
    existing_schema: &str,
    direction: CompatDirection,
    diff_fn: impl Fn(&str, &str) -> Result<(bool, Vec<String>), KoraError>,
) -> Result<CompatibilityResult, KoraError> {
    match direction {
        CompatDirection::Backward => {
            let (ok, msgs) = diff_fn(existing_schema, new_schema)?;
            Ok(CompatibilityResult {
                is_compatible: ok,
                messages: msgs,
            })
        }
        CompatDirection::Forward => {
            let (ok, msgs) = diff_fn(new_schema, existing_schema)?;
            Ok(CompatibilityResult {
                is_compatible: ok,
                messages: msgs,
            })
        }
        CompatDirection::Full => {
            let (bw_ok, mut msgs) = diff_fn(existing_schema, new_schema)?;
            let (fw_ok, fw_msgs) = diff_fn(new_schema, existing_schema)?;
            msgs.extend(fw_msgs);
            Ok(CompatibilityResult {
                is_compatible: bw_ok && fw_ok,
                messages: msgs,
            })
        }
        CompatDirection::None => Ok(CompatibilityResult {
            is_compatible: true,
            messages: Vec::new(),
        }),
    }
}
