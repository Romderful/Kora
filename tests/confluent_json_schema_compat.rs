//! JSON Schema compatibility tests replayed from Confluent Schema Registry.
//!
//! Ensures 100% wire-compatible diff engine behavior by running the exact same
//! test cases as Confluent's Java suite.
//!
//! Coverage:
//! - `SchemaDiffTest.java` — 4 fixture files (251 cases) + 2 inline tests
//! - `CircularRefSchemaDiffTest.java` — 2 generated circular `$ref` schemas
//!
//! Fixture data: `tests/fixtures/confluent/js_diff_*.json`

use std::time::Instant;

use serde::Deserialize;

#[derive(Deserialize)]
struct TestCase {
    description: String,
    original_schema: serde_json::Value,
    update_schema: serde_json::Value,
    #[allow(dead_code)]
    changes: Vec<String>,
    compatible: bool,
}

fn load_cases(filename: &str) -> Vec<TestCase> {
    let path = format!("tests/fixtures/confluent/{filename}");
    let data =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read {path}: {e}"));
    serde_json::from_str(&data).unwrap_or_else(|e| panic!("Failed to parse {path}: {e}"))
}

fn run_cases(filename: &str) {
    let cases = load_cases(filename);
    let mut passed = 0;
    let mut failed = 0;
    let mut failures = Vec::new();

    for (i, case) in cases.iter().enumerate() {
        let old = case.original_schema.to_string();
        let new = case.update_schema.to_string();

        let result = kora::schema::json_schema::check_compatibility(
            &new,
            &old,
            kora::schema::CompatDirection::Backward,
        );

        match result {
            Ok(result) => {
                if result.is_compatible == case.compatible {
                    passed += 1;
                } else {
                    failed += 1;
                    failures.push(format!(
                        "  [{i}] \"{}\": expected compatible={}, got compatible={}. messages={:?}",
                        case.description, case.compatible, result.is_compatible, result.messages
                    ));
                }
            }
            Err(e) => {
                failed += 1;
                failures.push(format!(
                    "  [{i}] \"{}\": diff engine error: {e}",
                    case.description
                ));
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "{filename}: {failed}/{} failed:\n{}",
            passed + failed,
            failures.join("\n")
        );
    }

    assert_eq!(failed, 0, "{filename}: all {passed} cases passed");
}

#[test]
fn confluent_json_schema_diff_examples() {
    run_cases("js_diff_examples.json");
}

#[test]
fn confluent_json_schema_diff_examples_2020_12() {
    run_cases("js_diff_examples_2020_12.json");
}

#[test]
fn confluent_json_schema_combined_examples() {
    run_cases("js_diff_combined.json");
}

#[test]
fn confluent_json_schema_combined_examples_2020_12() {
    run_cases("js_diff_combined_2020_12.json");
}

// -- Confluent SchemaDiffTest inline tests --

/// Confluent `testSchemaAddsProperties`: empty schema → schema with properties is incompatible.
#[test]
fn confluent_schema_adds_properties() {
    let old = "{}";
    let new = r#"{"properties": {}}"#;
    let result = kora::schema::json_schema::check_compatibility(
        new,
        old,
        kora::schema::CompatDirection::Backward,
    );
    assert!(result.is_ok());
    let result = result.unwrap();
    assert!(
        !result.is_compatible,
        "empty schema → schema with properties should be incompatible"
    );
}

/// Confluent `testConnectTypeAsBytes`: connect.type=bytes makes type change compatible.
#[test]
fn confluent_connect_type_as_bytes() {
    let old = r#"{"type":"string","title":"org.apache.kafka.connect.data.Decimal","connect.version":1,"connect.type":"bytes","connect.parameters":{"scale":"2"}}"#;
    let new = r#"{"type":"number","title":"org.apache.kafka.connect.data.Decimal","connect.version":1,"connect.type":"bytes","connect.parameters":{"scale":"2"}}"#;
    let result = kora::schema::json_schema::check_compatibility(
        new,
        old,
        kora::schema::CompatDirection::Backward,
    );
    assert!(result.is_ok());
    let result = result.unwrap();
    assert!(
        result.is_compatible,
        "connect.type=bytes schemas should be compatible regardless of type: {:?}",
        result.messages
    );
}

// -- Confluent CircularRefSchemaDiffTest --

/// Generate a circular JSON Schema with N oneOf branches and M properties per type.
fn circular_schema(branches: usize, props_per_type: usize) -> String {
    let mut defs = serde_json::Map::new();

    defs.insert(
        "ProductOrId".to_string(),
        serde_json::json!({
            "oneOf": [
                {"$ref": "#/$defs/Product"},
                {"type": "string"}
            ]
        }),
    );

    let mut one_of = Vec::new();
    for b in 0..branches {
        let type_name = format!("Type{b}");
        let mut props = serde_json::Map::new();
        for p in 0..props_per_type {
            props.insert(
                format!("prop{p}"),
                serde_json::json!({"$ref": "#/$defs/ProductOrId"}),
            );
        }
        defs.insert(
            type_name.clone(),
            serde_json::json!({
                "type": "object",
                "properties": props
            }),
        );
        one_of.push(serde_json::json!({"$ref": format!("#/$defs/{type_name}")}));
    }

    defs.insert("Product".to_string(), serde_json::json!({"oneOf": one_of}));

    serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$defs": defs,
        "$ref": "#/$defs/Product"
    })
    .to_string()
}

#[test]
fn confluent_circular_ref_2_branches() {
    let schema = circular_schema(2, 1);
    let start = Instant::now();
    let result = kora::schema::json_schema::check_compatibility(
        &schema,
        &schema,
        kora::schema::CompatDirection::Backward,
    );
    let elapsed = start.elapsed();

    assert!(result.is_ok(), "circular ref should not crash");
    assert!(
        result.unwrap().is_compatible,
        "identical schemas should be compatible"
    );
    assert!(
        elapsed.as_secs() < 5,
        "should complete quickly, took {elapsed:?}"
    );
}

#[test]
fn confluent_circular_ref_12_branches_x5_props() {
    let schema = circular_schema(12, 5);
    let start = Instant::now();
    let result = kora::schema::json_schema::check_compatibility(
        &schema,
        &schema,
        kora::schema::CompatDirection::Backward,
    );
    let elapsed = start.elapsed();

    assert!(result.is_ok(), "complex circular ref should not crash");
    assert!(
        result.unwrap().is_compatible,
        "identical schemas should be compatible"
    );
    assert!(
        elapsed.as_secs() < 10,
        "should complete within 10s, took {elapsed:?}"
    );
}
