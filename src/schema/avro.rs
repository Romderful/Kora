//! Avro schema parsing, canonical form, and fingerprinting.

use apache_avro::rabin::Rabin;
use apache_avro::Schema;

use crate::error::KoraError;
use crate::schema::ParsedSchema;

/// Parse an Avro schema string and compute its canonical form and fingerprint.
///
/// # Errors
///
/// Returns `KoraError::InvalidSchema` when the input is not valid Avro JSON.
pub fn parse(raw: &str) -> Result<ParsedSchema, KoraError> {
    let schema =
        Schema::parse_str(raw).map_err(|e| KoraError::InvalidSchema(e.to_string()))?;

    let canonical = schema.canonical_form();
    let fingerprint = schema.fingerprint::<Rabin>().to_string();

    Ok(ParsedSchema {
        canonical_form: canonical,
        fingerprint,
    })
}
