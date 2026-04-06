//! Unit tests for schema parsing, canonical form, and fingerprinting.

use kora::schema::{self, SchemaFormat};

const VALID_AVRO: &str = r#"{
    "type": "record",
    "name": "Test",
    "fields": [{"name": "id", "type": "int"}]
}"#;

// --- SchemaFormat ---

#[test]
fn default_format_is_avro() {
    assert_eq!(SchemaFormat::from_optional(None).unwrap(), SchemaFormat::Avro);
}

#[test]
fn explicit_avro_format() {
    assert_eq!(
        SchemaFormat::from_optional(Some("AVRO")).unwrap(),
        SchemaFormat::Avro,
    );
}

#[test]
fn avro_format_is_case_insensitive() {
    assert_eq!(SchemaFormat::from_optional(Some("avro")).unwrap(), SchemaFormat::Avro);
    assert_eq!(SchemaFormat::from_optional(Some("Avro")).unwrap(), SchemaFormat::Avro);
}

#[test]
fn unsupported_format_errors() {
    let err = SchemaFormat::from_optional(Some("XML")).unwrap_err();
    assert!(err.to_string().contains("Unsupported schema type"));
}

// --- Avro parsing ---

#[test]
fn parse_valid_avro() {
    let result = schema::parse(SchemaFormat::Avro, VALID_AVRO);
    assert!(result.is_ok());
}

#[test]
fn parse_invalid_avro() {
    let result = schema::parse(SchemaFormat::Avro, r#"{"not": "a schema"}"#);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid schema"));
}

#[test]
fn canonical_form_is_stable() {
    let a = schema::parse(SchemaFormat::Avro, VALID_AVRO).unwrap();
    let b = schema::parse(SchemaFormat::Avro, VALID_AVRO).unwrap();
    assert_eq!(a.canonical_form, b.canonical_form);
    assert!(!a.canonical_form.is_empty());
}

#[test]
fn fingerprint_is_stable() {
    let a = schema::parse(SchemaFormat::Avro, VALID_AVRO).unwrap();
    let b = schema::parse(SchemaFormat::Avro, VALID_AVRO).unwrap();
    assert_eq!(a.fingerprint, b.fingerprint);
    assert!(!a.fingerprint.is_empty());
}

#[test]
fn different_schemas_have_different_fingerprints() {
    let a = schema::parse(SchemaFormat::Avro, VALID_AVRO).unwrap();
    let b = schema::parse(
        SchemaFormat::Avro,
        r#"{"type":"record","name":"Other","fields":[{"name":"x","type":"string"}]}"#,
    )
    .unwrap();
    assert_ne!(a.fingerprint, b.fingerprint);
}
