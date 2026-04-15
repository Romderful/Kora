//! Protobuf compatibility tests replayed from Confluent Schema Registry.
//!
//! Ensures 100% wire-compatible diff engine behavior by running the exact same
//! test cases as Confluent's Java suite.
//!
//! Coverage:
//! - `SchemaDiffTest.java` — 1 fixture file (43 cases) + 1 inline test
//!
//! Fixture data: `tests/fixtures/confluent/pb_diff_examples.json`

use serde::Deserialize;

#[derive(Deserialize)]
struct DepEntry {
    name: String,
    dependency: String,
}

#[derive(Deserialize)]
struct TestCase {
    description: String,
    #[allow(dead_code)]
    original_metadata: Option<serde_json::Value>,
    #[allow(dead_code)]
    update_metadata: Option<serde_json::Value>,
    original_schema: String,
    update_schema: String,
    #[allow(dead_code)]
    changes: Vec<String>,
    compatible: bool,
    #[serde(default)]
    original_dependencies: Vec<DepEntry>,
    #[serde(default)]
    update_dependencies: Vec<DepEntry>,
}

fn load_cases() -> Vec<TestCase> {
    let data = std::fs::read_to_string("tests/fixtures/confluent/pb_diff_examples.json")
        .expect("Failed to read protobuf test fixtures");
    serde_json::from_str(&data).expect("Failed to parse protobuf test fixtures")
}

#[test]
fn confluent_protobuf_diff_examples() {
    let cases = load_cases();
    let mut passed = 0;
    let mut failed = 0;
    let mut failures = Vec::new();

    for (i, case) in cases.iter().enumerate() {
        let old_deps: Vec<(String, String)> = case
            .original_dependencies
            .iter()
            .map(|d| (d.name.clone(), d.dependency.clone()))
            .collect();
        let new_deps: Vec<(String, String)> = case
            .update_dependencies
            .iter()
            .map(|d| (d.name.clone(), d.dependency.clone()))
            .collect();

        let result = kora::schema::protobuf::check_compatibility_with_deps(
            &case.update_schema,
            &case.original_schema,
            kora::schema::CompatDirection::Backward,
            &old_deps,
            &new_deps,
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
                    "  [{i}] \"{}\": parse error: {e}",
                    case.description
                ));
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "pb_diff_examples.json: {failed}/{} failed:\n{}",
            passed + failed,
            failures.join("\n")
        );
    }
}

/// Confluent `checkCompatibilityUsingProtoFiles`: TestProto.proto vs TestProto2.proto.
///
/// Field #2 changes name (test_int → test_string2) and type (int32 → string).
#[test]
fn confluent_protobuf_proto_file_compat() {
    let old = r#"syntax = "proto3";
package test1;
message TestMessage {
    string test_string = 1;
    int32 test_int = 2;
}"#;
    let new = r#"syntax = "proto3";
package test2;
message TestMessage {
    string test_string = 1;
    string test_string2 = 2;
}"#;

    let result = kora::schema::protobuf::check_compatibility(
        new,
        old,
        kora::schema::CompatDirection::Backward,
    )
    .expect("should parse both schemas");

    // Confluent expects FIELD_SCALAR_KIND_CHANGED (int32→string) making it incompatible.
    // FIELD_NAME_CHANGED is compatible so it won't appear in error messages.
    assert!(
        !result.is_compatible,
        "field type int32→string should be incompatible: {:?}",
        result.messages
    );
    assert!(
        result
            .messages
            .iter()
            .any(|m| m.contains("field type changed")),
        "should detect FIELD_SCALAR_KIND_CHANGED: {:?}",
        result.messages
    );
}
