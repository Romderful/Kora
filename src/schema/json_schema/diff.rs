//! JSON Schema structural diff engine (Confluent-compatible).
//!
//! Computes structural differences between two JSON Schemas and classifies each
//! as compatible or incompatible using Confluent's `COMPATIBLE_CHANGES_STRICT` set.
//!
//! Handles draft 4-7 and 2020-12, including combined schemas (allOf/oneOf/anyOf),
//! `$ref` resolution, partially open content models, tuple items, dependencies
//! (`dependentRequired`/`dependentSchemas`), and Kafka Connect `connect.type` extensions.

use std::collections::HashSet;

use crate::error::KoraError;

// -- Diff types (matching Confluent's Difference.Type) --

/// A detected difference between two JSON Schemas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(clippy::enum_variant_names, dead_code)]
enum DiffType {
    // Metadata
    IdChanged,
    DescriptionChanged,
    TitleChanged,
    DefaultChanged,
    SchemaAdded,
    SchemaRemoved,
    // Type
    TypeExtended,
    TypeNarrowed,
    TypeChanged,
    // String constraints
    MaxLengthAdded,
    MaxLengthRemoved,
    MaxLengthIncreased,
    MaxLengthDecreased,
    MinLengthAdded,
    MinLengthRemoved,
    MinLengthIncreased,
    MinLengthDecreased,
    PatternAdded,
    PatternRemoved,
    PatternChanged,
    // Number constraints
    MaximumAdded,
    MaximumRemoved,
    MaximumIncreased,
    MaximumDecreased,
    MinimumAdded,
    MinimumRemoved,
    MinimumIncreased,
    MinimumDecreased,
    ExclusiveMaximumAdded,
    ExclusiveMaximumRemoved,
    ExclusiveMaximumIncreased,
    ExclusiveMaximumDecreased,
    ExclusiveMinimumAdded,
    ExclusiveMinimumRemoved,
    ExclusiveMinimumIncreased,
    ExclusiveMinimumDecreased,
    MultipleOfAdded,
    MultipleOfRemoved,
    MultipleOfExpanded,
    MultipleOfReduced,
    MultipleOfChanged,
    // Object properties
    RequiredAttributeAdded,
    RequiredAttributeWithDefaultAdded,
    RequiredAttributeRemoved,
    MaxPropertiesAdded,
    MaxPropertiesRemoved,
    MaxPropertiesIncreased,
    MaxPropertiesDecreased,
    MinPropertiesAdded,
    MinPropertiesRemoved,
    MinPropertiesIncreased,
    MinPropertiesDecreased,
    AdditionalPropertiesAdded,
    AdditionalPropertiesRemoved,
    AdditionalPropertiesExtended,
    AdditionalPropertiesNarrowed,
    PropertyAddedToOpenContentModel,
    RequiredPropertyAddedToUnopenContentModel,
    RequiredPropertyWithDefaultAddedToUnopenContentModel,
    OptionalPropertyAddedToUnopenContentModel,
    PropertyRemovedFromOpenContentModel,
    PropertyRemovedFromClosedContentModel,
    PropertyWithFalseRemovedFromClosedContentModel,
    PropertyWithEmptySchemaAddedToOpenContentModel,
    DependencyArrayAdded,
    DependencyArrayRemoved,
    DependencyArrayExtended,
    DependencyArrayNarrowed,
    DependencyArrayChanged,
    DependencySchemaAdded,
    DependencySchemaRemoved,
    PropertyAddedIsCoveredByPartiallyOpenContentModel,
    PropertyAddedNotCoveredByPartiallyOpenContentModel,
    PropertyRemovedIsCoveredByPartiallyOpenContentModel,
    PropertyRemovedNotCoveredByPartiallyOpenContentModel,
    // Array items
    MaxItemsAdded,
    MaxItemsRemoved,
    MaxItemsIncreased,
    MaxItemsDecreased,
    MinItemsAdded,
    MinItemsRemoved,
    MinItemsIncreased,
    MinItemsDecreased,
    UniqueItemsAdded,
    UniqueItemsRemoved,
    AdditionalItemsAdded,
    AdditionalItemsRemoved,
    AdditionalItemsExtended,
    AdditionalItemsNarrowed,
    ItemAddedToOpenContentModel,
    ItemAddedToClosedContentModel,
    ItemRemovedFromOpenContentModel,
    ItemWithFalseRemovedFromClosedContentModel,
    ItemRemovedFromClosedContentModel,
    ItemWithEmptySchemaAddedToOpenContentModel,
    // Enum
    EnumArrayExtended,
    EnumArrayNarrowed,
    EnumArrayChanged,
    // Combined types
    CombinedTypeExtended,
    CombinedTypeChanged,
    ProductTypeExtended,
    ProductTypeNarrowed,
    SumTypeExtended,
    SumTypeNarrowed,
    NotTypeExtended,
    NotTypeNarrowed,
    CombinedTypeSubschemasChanged,
    // Items (tuple validation)
    ItemAddedIsCoveredByPartiallyOpenContentModel,
    ItemAddedNotCoveredByPartiallyOpenContentModel,
    ItemRemovedIsCoveredByPartiallyOpenContentModel,
    ItemRemovedNotCoveredByPartiallyOpenContentModel,
}

/// Changes considered backward-compatible (matching Confluent's `COMPATIBLE_CHANGES_STRICT`).
const COMPATIBLE_CHANGES: &[DiffType] = &[
    DiffType::IdChanged,
    DiffType::DescriptionChanged,
    DiffType::TitleChanged,
    DiffType::DefaultChanged,
    DiffType::SchemaRemoved,
    DiffType::TypeExtended,
    DiffType::MaxLengthIncreased,
    DiffType::MaxLengthRemoved,
    DiffType::MinLengthDecreased,
    DiffType::MinLengthRemoved,
    DiffType::PatternRemoved,
    DiffType::MaximumIncreased,
    DiffType::MaximumRemoved,
    DiffType::MinimumDecreased,
    DiffType::MinimumRemoved,
    DiffType::ExclusiveMaximumIncreased,
    DiffType::ExclusiveMaximumRemoved,
    DiffType::ExclusiveMinimumDecreased,
    DiffType::ExclusiveMinimumRemoved,
    DiffType::MultipleOfReduced,
    DiffType::MultipleOfRemoved,
    DiffType::RequiredAttributeWithDefaultAdded,
    DiffType::RequiredAttributeRemoved,
    DiffType::DependencyArrayNarrowed,
    DiffType::DependencyArrayRemoved,
    DiffType::DependencySchemaRemoved,
    DiffType::MaxPropertiesIncreased,
    DiffType::MaxPropertiesRemoved,
    DiffType::MinPropertiesDecreased,
    DiffType::MinPropertiesRemoved,
    DiffType::AdditionalPropertiesAdded,
    DiffType::AdditionalPropertiesExtended,
    DiffType::PropertyWithEmptySchemaAddedToOpenContentModel,
    DiffType::RequiredPropertyWithDefaultAddedToUnopenContentModel,
    DiffType::OptionalPropertyAddedToUnopenContentModel,
    DiffType::PropertyWithFalseRemovedFromClosedContentModel,
    DiffType::PropertyRemovedFromOpenContentModel,
    DiffType::PropertyAddedIsCoveredByPartiallyOpenContentModel,
    DiffType::PropertyRemovedIsCoveredByPartiallyOpenContentModel,
    DiffType::MaxItemsIncreased,
    DiffType::MaxItemsRemoved,
    DiffType::MinItemsDecreased,
    DiffType::MinItemsRemoved,
    DiffType::UniqueItemsRemoved,
    DiffType::AdditionalItemsAdded,
    DiffType::AdditionalItemsExtended,
    DiffType::ItemWithEmptySchemaAddedToOpenContentModel,
    DiffType::ItemAddedToClosedContentModel,
    DiffType::ItemWithFalseRemovedFromClosedContentModel,
    DiffType::ItemRemovedFromOpenContentModel,
    DiffType::ItemAddedIsCoveredByPartiallyOpenContentModel,
    DiffType::ItemRemovedIsCoveredByPartiallyOpenContentModel,
    DiffType::EnumArrayExtended,
    DiffType::CombinedTypeExtended,
    DiffType::ProductTypeNarrowed,
    DiffType::SumTypeExtended,
    DiffType::NotTypeNarrowed,
];

// -- Public entry point --

/// Check backward compatibility between two JSON Schemas (old → new).
///
/// Computes a structural diff and filters against the Confluent-compatible
/// `COMPATIBLE_CHANGES_STRICT` set.
///
/// # Errors
///
/// Returns `KoraError::InvalidSchema` if either schema is malformed JSON.
pub fn check(old_str: &str, new_str: &str) -> Result<(bool, Vec<String>), KoraError> {
    let old_raw: serde_json::Value =
        serde_json::from_str(old_str).map_err(|e| KoraError::InvalidSchema(e.to_string()))?;
    let new_raw: serde_json::Value =
        serde_json::from_str(new_str).map_err(|e| KoraError::InvalidSchema(e.to_string()))?;

    // Resolve $ref inline before comparing (single-document $ref only).
    let old = resolve_refs(&old_raw, &old_raw);
    let new = resolve_refs(&new_raw, &new_raw);

    let compatible_set: HashSet<DiffType> = COMPATIBLE_CHANGES.iter().copied().collect();
    let diffs = compare(&old, &new);

    let mut messages = Vec::new();
    let mut compatible = true;
    for (dt, msg) in &diffs {
        if !compatible_set.contains(dt) {
            compatible = false;
            messages.push(msg.clone());
        }
    }
    Ok((compatible, messages))
}

// -- Schema type helpers --

/// JSON Schema `false` or `{"not":{}}` — rejects everything.
fn is_false_schema_value(schema: &serde_json::Value) -> bool {
    schema == &serde_json::Value::Bool(false)
        || (schema.is_object()
            && schema.as_object().is_some_and(|m| m.len() == 1)
            && schema
                .get("not")
                .is_some_and(|n| n == &serde_json::json!({})))
}

/// JSON Schema `true` or `{}` — accepts everything.
fn is_empty_schema_value(schema: &serde_json::Value) -> bool {
    schema == &serde_json::Value::Bool(true)
        || schema.as_object().is_some_and(serde_json::Map::is_empty)
}

/// Kafka Connect `connect.type: "bytes"` — schemas with this are type-equivalent.
fn is_connect_bytes(schema: &serde_json::Value) -> bool {
    schema.get("connect.type").and_then(|v| v.as_str()) == Some("bytes")
}

/// Whether a schema uses combined keywords (anyOf, oneOf, allOf).
fn is_combined_schema(schema: &serde_json::Value) -> bool {
    schema.get("anyOf").is_some() || schema.get("oneOf").is_some() || schema.get("allOf").is_some()
}

/// Get the combined criterion keyword ("anyOf", "oneOf", "allOf") or empty.
fn combined_criterion(schema: &serde_json::Value) -> String {
    for kw in &["oneOf", "anyOf", "allOf"] {
        if schema.get(*kw).is_some() {
            return (*kw).to_string();
        }
    }
    String::new()
}

/// Non-combined schema → combined schema transition.
///
/// Confluent: if old can be found compatible with at least one subschema in new,
/// the transition is `SUM_TYPE_EXTENDED` (compatible).
fn compare_schema_to_combined(
    old: &serde_json::Value,
    new: &serde_json::Value,
) -> Vec<(DiffType, String)> {
    let compatible_set: HashSet<DiffType> = COMPATIBLE_CHANGES.iter().copied().collect();

    // allOf: old must be compatible with ALL subschemas (intersection semantics).
    if let Some(arr) = new.get("allOf").and_then(|v| v.as_array()) {
        for sub in arr {
            let sub_diffs = compare(old, sub);
            if sub_diffs.iter().any(|(dt, _)| !compatible_set.contains(dt)) {
                return vec![(
                    DiffType::TypeChanged,
                    "Schema changed to allOf type incompatibly".into(),
                )];
            }
        }
        return vec![(DiffType::SumTypeExtended, "Schema widened to allOf".into())];
    }

    // oneOf/anyOf: old must be compatible with at least one subschema.
    for kw in &["anyOf", "oneOf"] {
        if let Some(arr) = new.get(*kw).and_then(|v| v.as_array()) {
            for sub in arr {
                let sub_diffs = compare(old, sub);
                if sub_diffs.iter().all(|(dt, _)| compatible_set.contains(dt)) {
                    let mut diffs = sub_diffs;
                    diffs.push((DiffType::SumTypeExtended, format!("Schema widened to {kw}")));
                    return diffs;
                }
            }
        }
    }

    vec![(
        DiffType::TypeChanged,
        "Schema changed to combined type incompatibly".into(),
    )]
}

/// Combined schema → non-combined schema transition.
fn compare_combined_to_schema(
    old: &serde_json::Value,
    new: &serde_json::Value,
) -> Vec<(DiffType, String)> {
    let compatible_set: HashSet<DiffType> = COMPATIBLE_CHANGES.iter().copied().collect();

    for kw in &["anyOf", "oneOf", "allOf"] {
        if let Some(arr) = old.get(*kw).and_then(|v| v.as_array()) {
            let narrowed_type = if *kw == "allOf" {
                DiffType::ProductTypeNarrowed
            } else {
                DiffType::SumTypeNarrowed
            };
            for sub in arr {
                let sub_diffs = compare(sub, new);
                if sub_diffs.iter().all(|(dt, _)| compatible_set.contains(dt)) {
                    let mut diffs = sub_diffs;
                    diffs.push((narrowed_type, format!("Schema narrowed from {kw}")));
                    return diffs;
                }
            }
        }
    }

    vec![(
        DiffType::TypeChanged,
        "Schema changed from combined type incompatibly".into(),
    )]
}

/// Combined criteria change (e.g., `oneOf` → `anyOf`, `oneOf` → `allOf`).
///
/// Confluent: singleton transitions and certain criteria changes are `COMBINED_TYPE_EXTENDED`.
fn compare_combined_criteria_change(
    old: &serde_json::Value,
    new: &serde_json::Value,
    old_criterion: &str,
    new_criterion: &str,
) -> Vec<(DiffType, String)> {
    let old_arr = old.get(old_criterion).and_then(|v| v.as_array());
    let new_arr = new.get(new_criterion).and_then(|v| v.as_array());
    let old_is_singleton = old_arr.is_some_and(|a| a.len() == 1);
    let new_is_singleton = new_arr.is_some_and(|a| a.len() == 1);

    // Confluent: COMBINED_TYPE_EXTENDED when criteria change is relaxing:
    // - old is singleton (unwrap safe, semantics preserved)
    // - new criterion is anyOf (most permissive combined type)
    // - new is singleton AND new criterion is NOT allOf (allOf adds constraints)
    if old_is_singleton
        || new_criterion == "anyOf"
        || (new_is_singleton && new_criterion != "allOf")
    {
        let mut diffs = vec![(
            DiffType::CombinedTypeExtended,
            format!("{old_criterion} → {new_criterion}"),
        )];

        if let (Some(old_items), Some(new_items)) = (old_arr, new_arr) {
            let compatible_set: HashSet<DiffType> = COMPATIBLE_CHANGES.iter().copied().collect();

            let comparisons: Vec<Vec<Vec<(DiffType, String)>>> = old_items
                .iter()
                .map(|o| new_items.iter().map(|n| compare(o, n)).collect())
                .collect();
            let compat_matrix: Vec<Vec<bool>> = comparisons
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|d| d.iter().all(|(dt, _)| compatible_set.contains(dt)))
                        .collect()
                })
                .collect();

            // Bipartite matching for subschema pairing.
            let match_col =
                bipartite_matching_pairs(&compat_matrix, old_items.len(), new_items.len());

            // Length-based extended/narrowed using keyword semantics.
            let narrowed_type = if old_criterion == "allOf" {
                DiffType::ProductTypeNarrowed
            } else {
                DiffType::SumTypeNarrowed
            };
            let extended_type = if new_criterion == "allOf" {
                DiffType::ProductTypeExtended
            } else {
                DiffType::SumTypeExtended
            };
            if old_items.len() < new_items.len() {
                diffs.push((extended_type, format!("{new_criterion} extended")));
            } else if old_items.len() > new_items.len() {
                diffs.push((narrowed_type, format!("{old_criterion} narrowed")));
            }

            // Sub-diffs for bipartite-matched pairs.
            for (col, opt_row) in match_col.iter().enumerate() {
                if let Some(row) = opt_row {
                    diffs.extend(comparisons[*row][col].clone());
                }
            }
        }

        return diffs;
    }

    vec![(
        DiffType::CombinedTypeChanged,
        format!("{old_criterion} → {new_criterion}"),
    )]
}

/// Unwrap a singleton combined schema (oneOf/anyOf with exactly 1 element).
fn unwrap_singleton(schema: &serde_json::Value) -> Option<serde_json::Value> {
    for keyword in &["oneOf", "anyOf", "allOf"] {
        if let Some(arr) = schema.get(*keyword).and_then(|v| v.as_array())
            && arr.len() == 1
        {
            return Some(arr[0].clone());
        }
    }
    None
}

// -- $ref resolution --

/// Resolve `$ref` pointers within a single JSON Schema document.
///
/// Replaces `{"$ref": "#/definitions/Foo"}` with the actual schema from
/// the `definitions` (or `$defs`) section. Handles `"#"` (root self-ref).
/// Tracks visited refs to detect cycles.
fn resolve_refs(node: &serde_json::Value, root: &serde_json::Value) -> serde_json::Value {
    resolve_refs_inner(node, root, &mut HashSet::new())
}

fn resolve_refs_inner(
    node: &serde_json::Value,
    root: &serde_json::Value,
    visited: &mut HashSet<String>,
) -> serde_json::Value {
    match node {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::String(ref_str)) = map.get("$ref") {
                if visited.contains(ref_str.as_str()) {
                    return node.clone(); // cycle — stop
                }
                visited.insert(ref_str.clone());
                if let Some(resolved) = resolve_json_pointer(ref_str, root) {
                    return resolve_refs_inner(resolved, root, visited);
                }
            }
            let new_map: serde_json::Map<String, serde_json::Value> = map
                .iter()
                .map(|(k, v)| (k.clone(), resolve_refs_inner(v, root, visited)))
                .collect();
            serde_json::Value::Object(new_map)
        }
        serde_json::Value::Array(arr) => serde_json::Value::Array(
            arr.iter()
                .map(|v| resolve_refs_inner(v, root, visited))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Resolve a JSON Pointer reference. Handles `"#"` (root) and `"#/path/to/def"`.
fn resolve_json_pointer<'a>(
    ref_str: &str,
    root: &'a serde_json::Value,
) -> Option<&'a serde_json::Value> {
    if ref_str == "#" {
        return Some(root);
    }
    let path = ref_str.strip_prefix("#/")?;
    let mut current = root;
    for segment in path.split('/') {
        let decoded = segment.replace("~1", "/").replace("~0", "~");
        current = current.get(&decoded)?;
    }
    Some(current)
}

// -- Schema diff engine --

/// Compare two JSON Schemas and produce a list of differences.
fn compare(old: &serde_json::Value, new: &serde_json::Value) -> Vec<(DiffType, String)> {
    // FalseSchema ({"not":{}}) → anything OR anything → EmptySchema ({}) is always compatible.
    if is_false_schema_value(old) || is_empty_schema_value(new) {
        return Vec::new();
    }

    // Confluent: empty schema → schema with structural keywords (properties, items, etc.)
    // is treated as "schema adds properties" — a narrowing change.
    if is_empty_schema_value(old) && !is_empty_schema_value(new) && new.is_object() {
        return vec![(
            DiffType::TypeNarrowed,
            "Schema changed from empty to constrained".into(),
        )];
    }

    // Detect schema-level transitions (non-combined ↔ combined, or criteria change).
    // Confluent handles these at the top level before individual comparisons.
    let old_is_combined = is_combined_schema(old);
    let new_is_combined = is_combined_schema(new);

    // Non-combined → combined: check if old is compatible with any new subschema.
    if !old_is_combined && new_is_combined {
        return compare_schema_to_combined(old, new);
    }

    // Combined → non-combined: check if any old subschema is compatible with new.
    if old_is_combined && !new_is_combined {
        return compare_combined_to_schema(old, new);
    }

    // Both combined but different criteria (oneOf→anyOf, etc.): handle transition.
    if old_is_combined && new_is_combined {
        let old_criterion = combined_criterion(old);
        let new_criterion = combined_criterion(new);
        if old_criterion != new_criterion {
            return compare_combined_criteria_change(old, new, &old_criterion, &new_criterion);
        }
    }

    // Unwrap singleton combined schemas.
    let old_unwrapped = unwrap_singleton(old);
    let new_unwrapped = unwrap_singleton(new);
    let old = old_unwrapped.as_ref().unwrap_or(old);
    let new = new_unwrapped.as_ref().unwrap_or(new);

    // After unwrapping, one side may still be combined while the other is simple.
    // Re-enter the combined detection path to handle the transition correctly.
    if !is_combined_schema(old) && is_combined_schema(new) {
        return compare_schema_to_combined(old, new);
    }
    if is_combined_schema(old) && !is_combined_schema(new) {
        return compare_combined_to_schema(old, new);
    }

    let mut diffs = Vec::new();
    compare_metadata(old, new, &mut diffs);
    compare_type(old, new, &mut diffs);
    compare_const(old, new, &mut diffs);
    compare_string_constraints(old, new, &mut diffs);
    compare_number_constraints(old, new, &mut diffs);
    compare_object_properties(old, new, &mut diffs);
    compare_array_constraints(old, new, &mut diffs);
    compare_enum(old, new, &mut diffs);
    compare_combined(old, new, &mut diffs);
    diffs
}

fn compare_metadata(
    old: &serde_json::Value,
    new: &serde_json::Value,
    diffs: &mut Vec<(DiffType, String)>,
) {
    check_value_changed(old, new, "$id", DiffType::IdChanged, diffs);
    check_value_changed(old, new, "title", DiffType::TitleChanged, diffs);
    check_value_changed(old, new, "description", DiffType::DescriptionChanged, diffs);
    check_value_changed(old, new, "default", DiffType::DefaultChanged, diffs);
}

/// Confluent treats const changes as `ENUM_ARRAY_CHANGED` (same diff type as enum changes).
fn compare_const(
    old: &serde_json::Value,
    new: &serde_json::Value,
    diffs: &mut Vec<(DiffType, String)>,
) {
    if let (Some(o), Some(n)) = (old.get("const"), new.get("const"))
        && o != n
    {
        diffs.push((DiffType::EnumArrayChanged, "const value changed".into()));
    }
}

fn compare_type(
    old: &serde_json::Value,
    new: &serde_json::Value,
    diffs: &mut Vec<(DiffType, String)>,
) {
    // Confluent: Kafka Connect `connect.type: "bytes"` makes schemas equivalent regardless of type.
    if is_connect_bytes(old) && is_connect_bytes(new) {
        return;
    }

    let old_types = extract_types(old);
    let new_types = extract_types(new);

    // Confluent: no type diff when both schemas have no type field.
    if (old_types.is_empty() && new_types.is_empty()) || old_types == new_types {
        return;
    }

    // Normalize for type promotion: integer is a subtype of number.
    let old_normalized = normalize_types(&old_types);
    let new_normalized = normalize_types(&new_types);

    if old_normalized == new_normalized {
        if new_types.contains("number") && old_types.contains("integer") {
            diffs.push((
                DiffType::TypeExtended,
                format!("Type promoted from {old_types:?} to {new_types:?}"),
            ));
        } else {
            diffs.push((
                DiffType::TypeNarrowed,
                format!("Type narrowed from {old_types:?} to {new_types:?}"),
            ));
        }
        return;
    }

    if old_normalized.is_subset(&new_normalized) {
        diffs.push((
            DiffType::TypeExtended,
            format!("Type extended from {old_types:?} to {new_types:?}"),
        ));
    } else if new_normalized.is_subset(&old_normalized) {
        diffs.push((
            DiffType::TypeNarrowed,
            format!("Type narrowed from {old_types:?} to {new_types:?}"),
        ));
    } else {
        diffs.push((
            DiffType::TypeChanged,
            format!("Type changed from {old_types:?} to {new_types:?}"),
        ));
    }
}

fn normalize_types(types: &HashSet<String>) -> HashSet<String> {
    types
        .iter()
        .map(|t| {
            if t == "integer" {
                "number".to_string()
            } else {
                t.clone()
            }
        })
        .collect()
}

fn compare_string_constraints(
    old: &serde_json::Value,
    new: &serde_json::Value,
    diffs: &mut Vec<(DiffType, String)>,
) {
    compare_numeric_field(
        old,
        new,
        "maxLength",
        (
            DiffType::MaxLengthAdded,
            DiffType::MaxLengthRemoved,
            DiffType::MaxLengthIncreased,
            DiffType::MaxLengthDecreased,
        ),
        diffs,
    );
    compare_numeric_field(
        old,
        new,
        "minLength",
        (
            DiffType::MinLengthAdded,
            DiffType::MinLengthRemoved,
            DiffType::MinLengthIncreased,
            DiffType::MinLengthDecreased,
        ),
        diffs,
    );
    compare_string_field(
        old,
        new,
        "pattern",
        DiffType::PatternAdded,
        DiffType::PatternRemoved,
        DiffType::PatternChanged,
        diffs,
    );
}

fn compare_number_constraints(
    old: &serde_json::Value,
    new: &serde_json::Value,
    diffs: &mut Vec<(DiffType, String)>,
) {
    compare_numeric_field(
        old,
        new,
        "maximum",
        (
            DiffType::MaximumAdded,
            DiffType::MaximumRemoved,
            DiffType::MaximumIncreased,
            DiffType::MaximumDecreased,
        ),
        diffs,
    );
    compare_numeric_field(
        old,
        new,
        "minimum",
        (
            DiffType::MinimumAdded,
            DiffType::MinimumRemoved,
            DiffType::MinimumIncreased,
            DiffType::MinimumDecreased,
        ),
        diffs,
    );
    compare_numeric_field(
        old,
        new,
        "exclusiveMaximum",
        (
            DiffType::ExclusiveMaximumAdded,
            DiffType::ExclusiveMaximumRemoved,
            DiffType::ExclusiveMaximumIncreased,
            DiffType::ExclusiveMaximumDecreased,
        ),
        diffs,
    );
    compare_numeric_field(
        old,
        new,
        "exclusiveMinimum",
        (
            DiffType::ExclusiveMinimumAdded,
            DiffType::ExclusiveMinimumRemoved,
            DiffType::ExclusiveMinimumIncreased,
            DiffType::ExclusiveMinimumDecreased,
        ),
        diffs,
    );
    // multipleOf: uses divisibility, not numeric comparison.
    compare_multiple_of(old, new, diffs);
}

fn compare_object_properties(
    old: &serde_json::Value,
    new: &serde_json::Value,
    diffs: &mut Vec<(DiffType, String)>,
) {
    compare_additional_properties(old, new, diffs);

    let open = is_open_content_model(new);
    let new_required = extract_string_set(new, "required");
    compare_required_attributes(old, new, &new_required, diffs);
    compare_property_changes(old, new, open, &new_required, diffs);

    compare_numeric_field(
        old,
        new,
        "maxProperties",
        (
            DiffType::MaxPropertiesAdded,
            DiffType::MaxPropertiesRemoved,
            DiffType::MaxPropertiesIncreased,
            DiffType::MaxPropertiesDecreased,
        ),
        diffs,
    );
    compare_numeric_field(
        old,
        new,
        "minProperties",
        (
            DiffType::MinPropertiesAdded,
            DiffType::MinPropertiesRemoved,
            DiffType::MinPropertiesIncreased,
            DiffType::MinPropertiesDecreased,
        ),
        diffs,
    );
    compare_dependencies(old, new, diffs);
}

fn compare_additional_properties(
    old: &serde_json::Value,
    new: &serde_json::Value,
    diffs: &mut Vec<(DiffType, String)>,
) {
    let default_true = serde_json::Value::Bool(true);
    let old_ap = old.get("additionalProperties").unwrap_or(&default_true);
    let new_ap = new.get("additionalProperties").unwrap_or(&default_true);
    if old_ap == new_ap {
        return;
    }
    if is_more_restrictive(old_ap, new_ap) {
        diffs.push((
            DiffType::AdditionalPropertiesNarrowed,
            "additionalProperties narrowed".into(),
        ));
    } else if old_ap == &serde_json::Value::Bool(false)
        || (old_ap.is_object() && new_ap == &default_true)
    {
        diffs.push((
            DiffType::AdditionalPropertiesAdded,
            "additionalProperties added".into(),
        ));
    } else {
        diffs.push((
            DiffType::AdditionalPropertiesExtended,
            "additionalProperties extended".into(),
        ));
    }
}

fn compare_required_attributes(
    old: &serde_json::Value,
    new: &serde_json::Value,
    new_required: &HashSet<String>,
    diffs: &mut Vec<(DiffType, String)>,
) {
    let old_required = extract_string_set(old, "required");
    for attr in old_required.difference(new_required) {
        diffs.push((
            DiffType::RequiredAttributeRemoved,
            format!("Required attribute '{attr}' removed"),
        ));
    }
    let new_props_keys = extract_property_keys(new);
    for attr in new_required.difference(&old_required) {
        if !new_props_keys.contains(attr) {
            continue;
        }
        let has_default = new
            .get("properties")
            .and_then(|p| p.get(attr.as_str()))
            .and_then(|p| p.get("default"))
            .is_some();
        let dt = if has_default {
            DiffType::RequiredAttributeWithDefaultAdded
        } else {
            DiffType::RequiredAttributeAdded
        };
        diffs.push((dt, format!("Required attribute '{attr}' added")));
    }
}

fn compare_property_changes(
    old: &serde_json::Value,
    new: &serde_json::Value,
    open: bool,
    new_required: &HashSet<String>,
    diffs: &mut Vec<(DiffType, String)>,
) {
    let old_props = extract_property_keys(old);
    let new_props = extract_property_keys(new);
    let partially_open_old = is_partially_open_content_model(old);
    let partially_open_new = is_partially_open_content_model(new);

    for prop in new_props.difference(&old_props) {
        let prop_schema = new.get("properties").and_then(|p| p.get(prop.as_str()));
        let dt = if partially_open_old {
            if is_covered_by_partial_model(old, prop, prop_schema, false) {
                DiffType::PropertyAddedIsCoveredByPartiallyOpenContentModel
            } else {
                DiffType::PropertyAddedNotCoveredByPartiallyOpenContentModel
            }
        } else if open {
            if is_empty_schema(new, prop) {
                DiffType::PropertyWithEmptySchemaAddedToOpenContentModel
            } else {
                DiffType::PropertyAddedToOpenContentModel
            }
        } else if new_required.contains(prop) {
            let has_default = new
                .get("properties")
                .and_then(|p| p.get(prop.as_str()))
                .and_then(|p| p.get("default"))
                .is_some();
            if has_default {
                DiffType::RequiredPropertyWithDefaultAddedToUnopenContentModel
            } else {
                DiffType::RequiredPropertyAddedToUnopenContentModel
            }
        } else {
            DiffType::OptionalPropertyAddedToUnopenContentModel
        };
        diffs.push((dt, format!("Property '{prop}' added")));
    }

    for prop in old_props.difference(&new_props) {
        let prop_schema = old.get("properties").and_then(|p| p.get(prop.as_str()));
        let dt = if partially_open_new {
            if is_covered_by_partial_model(new, prop, prop_schema, true) {
                DiffType::PropertyRemovedIsCoveredByPartiallyOpenContentModel
            } else {
                DiffType::PropertyRemovedNotCoveredByPartiallyOpenContentModel
            }
        } else if open {
            DiffType::PropertyRemovedFromOpenContentModel
        } else if is_false_schema(old, prop) {
            DiffType::PropertyWithFalseRemovedFromClosedContentModel
        } else {
            DiffType::PropertyRemovedFromClosedContentModel
        };
        diffs.push((dt, format!("Property '{prop}' removed")));
    }

    for prop in old_props.intersection(&new_props) {
        if let (Some(old_prop), Some(new_prop)) = (
            old.get("properties").and_then(|p| p.get(prop.as_str())),
            new.get("properties").and_then(|p| p.get(prop.as_str())),
        ) {
            diffs.extend(compare(old_prop, new_prop));
        }
    }
}

fn compare_array_constraints(
    old: &serde_json::Value,
    new: &serde_json::Value,
    diffs: &mut Vec<(DiffType, String)>,
) {
    compare_numeric_field(
        old,
        new,
        "maxItems",
        (
            DiffType::MaxItemsAdded,
            DiffType::MaxItemsRemoved,
            DiffType::MaxItemsIncreased,
            DiffType::MaxItemsDecreased,
        ),
        diffs,
    );
    compare_numeric_field(
        old,
        new,
        "minItems",
        (
            DiffType::MinItemsAdded,
            DiffType::MinItemsRemoved,
            DiffType::MinItemsIncreased,
            DiffType::MinItemsDecreased,
        ),
        diffs,
    );

    match (old.get("uniqueItems"), new.get("uniqueItems")) {
        (None | Some(serde_json::Value::Bool(false)), Some(serde_json::Value::Bool(true))) => {
            diffs.push((DiffType::UniqueItemsAdded, "uniqueItems added".into()));
        }
        (Some(serde_json::Value::Bool(true)), None | Some(serde_json::Value::Bool(false))) => {
            diffs.push((DiffType::UniqueItemsRemoved, "uniqueItems removed".into()));
        }
        _ => {}
    }

    // additionalItems
    // Absent additionalItems = default open. Normalize: true → absent, false → present.
    let ai_default = serde_json::Value::Bool(true);
    let old_ai = old.get("additionalItems").unwrap_or(&ai_default);
    let new_ai = new.get("additionalItems").unwrap_or(&ai_default);
    match (old_ai, new_ai) {
        (o, n) if o == n => {}
        (o, n)
            if o == &serde_json::Value::Bool(false)
                || (o.is_object() && n == &serde_json::Value::Bool(true)) =>
        {
            diffs.push((
                DiffType::AdditionalItemsAdded,
                "additionalItems added".into(),
            ));
        }
        (o, n) if is_more_restrictive(o, n) => {
            diffs.push((
                DiffType::AdditionalItemsNarrowed,
                "additionalItems narrowed".into(),
            ));
        }
        _ => {
            diffs.push((
                DiffType::AdditionalItemsExtended,
                "additionalItems extended".into(),
            ));
        }
    }

    // items — single schema (draft 2020-12) or legacy tuple (draft 4-7)
    match (old.get("items"), new.get("items")) {
        (Some(old_items), Some(new_items)) if old_items.is_object() && new_items.is_object() => {
            diffs.extend(compare(old_items, new_items));
        }
        (Some(old_items), Some(new_items)) if old_items.is_array() && new_items.is_array() => {
            compare_tuple_items(old_items, new_items, old, new, "additionalItems", diffs);
        }
        // 2020-12: boolean items (true/false) acts like additionalItems.
        (Some(serde_json::Value::Bool(true)), Some(serde_json::Value::Bool(false))) => {
            diffs.push((DiffType::AdditionalItemsRemoved, "items restricted".into()));
        }
        (Some(serde_json::Value::Bool(false)), Some(serde_json::Value::Bool(true))) => {
            diffs.push((DiffType::AdditionalItemsAdded, "items relaxed".into()));
        }
        (None, Some(_)) => diffs.push((DiffType::SchemaAdded, "items added".into())),
        (Some(_), None) => diffs.push((DiffType::SchemaRemoved, "items removed".into())),
        _ => {}
    }

    // prefixItems — tuple validation (draft 2020-12)
    match (old.get("prefixItems"), new.get("prefixItems")) {
        (Some(old_items), Some(new_items)) if old_items.is_array() && new_items.is_array() => {
            compare_tuple_items(old_items, new_items, old, new, "items", diffs);
        }
        _ => {}
    }
}

/// Compare tuple-style `items` arrays (positional item schemas).
/// Compare tuple-style items arrays (positional item schemas).
///
/// `additional_keyword` is `"additionalItems"` for legacy `items`-as-array,
/// or `"items"` for draft 2020-12 `prefixItems`.
fn compare_tuple_items(
    old_items: &serde_json::Value,
    new_items: &serde_json::Value,
    old_schema: &serde_json::Value,
    new_schema: &serde_json::Value,
    additional_keyword: &str,
    diffs: &mut Vec<(DiffType, String)>,
) {
    let old_arr = old_items.as_array().unwrap();
    let new_arr = new_items.as_array().unwrap();
    let compatible_set: HashSet<DiffType> = COMPATIBLE_CHANGES.iter().copied().collect();

    // Items added to tuple — check OLD schema's additional items model for coverage.
    let old_additional_schema = old_schema.get(additional_keyword).filter(|v| v.is_object());
    let old_is_open = !matches!(
        old_schema.get(additional_keyword),
        Some(serde_json::Value::Bool(false))
    );
    for added_item in new_arr.iter().skip(old_arr.len()) {
        let dt = if let Some(add_schema) = old_additional_schema {
            let sub_diffs = compare(add_schema, added_item);
            if sub_diffs.iter().all(|(dt, _)| compatible_set.contains(dt)) {
                DiffType::ItemAddedIsCoveredByPartiallyOpenContentModel
            } else {
                DiffType::ItemAddedNotCoveredByPartiallyOpenContentModel
            }
        } else if old_is_open {
            if is_empty_schema_value(added_item) {
                DiffType::ItemWithEmptySchemaAddedToOpenContentModel
            } else {
                DiffType::ItemAddedToOpenContentModel
            }
        } else {
            DiffType::ItemAddedToClosedContentModel
        };
        diffs.push((dt, "Tuple item added".into()));
    }

    // Items removed from tuple — check NEW schema's additional items model for coverage.
    let new_additional_schema = new_schema.get(additional_keyword).filter(|v| v.is_object());
    let new_is_open = !matches!(
        new_schema.get(additional_keyword),
        Some(serde_json::Value::Bool(false))
    );
    for removed_item in old_arr.iter().skip(new_arr.len()) {
        let dt = if let Some(add_schema) = new_additional_schema {
            let sub_diffs = compare(removed_item, add_schema);
            if sub_diffs.iter().all(|(dt, _)| compatible_set.contains(dt)) {
                DiffType::ItemRemovedIsCoveredByPartiallyOpenContentModel
            } else {
                DiffType::ItemRemovedNotCoveredByPartiallyOpenContentModel
            }
        } else if new_is_open {
            DiffType::ItemRemovedFromOpenContentModel
        } else if removed_item == &serde_json::Value::Bool(false) {
            DiffType::ItemWithFalseRemovedFromClosedContentModel
        } else {
            DiffType::ItemRemovedFromClosedContentModel
        };
        diffs.push((dt, "Tuple item removed".into()));
    }

    // Recurse into common positions
    for (o, n) in old_arr.iter().zip(new_arr.iter()) {
        diffs.extend(compare(o, n));
    }
}

fn compare_enum(
    old: &serde_json::Value,
    new: &serde_json::Value,
    diffs: &mut Vec<(DiffType, String)>,
) {
    let old_enum = old.get("enum").and_then(|e| e.as_array());
    let new_enum = new.get("enum").and_then(|e| e.as_array());

    if let (Some(old_vals), Some(new_vals)) = (old_enum, new_enum) {
        let old_set: HashSet<String> = old_vals.iter().map(ToString::to_string).collect();
        let new_set: HashSet<String> = new_vals.iter().map(ToString::to_string).collect();

        if old_set == new_set {
            return;
        }

        let added = new_set.difference(&old_set).count() > 0;
        let removed = old_set.difference(&new_set).count() > 0;

        if added && !removed {
            diffs.push((DiffType::EnumArrayExtended, "Enum values extended".into()));
        } else if removed && !added {
            diffs.push((DiffType::EnumArrayNarrowed, "Enum values narrowed".into()));
        } else {
            diffs.push((DiffType::EnumArrayChanged, "Enum values changed".into()));
        }
    }
}

fn compare_combined(
    old: &serde_json::Value,
    new: &serde_json::Value,
    diffs: &mut Vec<(DiffType, String)>,
) {
    compare_combined_keyword(
        old,
        new,
        "anyOf",
        DiffType::SumTypeExtended,
        DiffType::SumTypeNarrowed,
        diffs,
    );
    compare_combined_keyword(
        old,
        new,
        "oneOf",
        DiffType::SumTypeExtended,
        DiffType::SumTypeNarrowed,
        diffs,
    );
    compare_combined_keyword(
        old,
        new,
        "allOf",
        DiffType::ProductTypeExtended,
        DiffType::ProductTypeNarrowed,
        diffs,
    );

    // not — Confluent compares in REVERSED order, then checks compatibility.
    // If reversed comparison is compatible → NOT_TYPE_NARROWED (more restrictive not = more permissive overall).
    // If incompatible → NOT_TYPE_EXTENDED.
    if let (Some(old_not), Some(new_not)) = (old.get("not"), new.get("not"))
        && old_not != new_not
    {
        let compatible_set: HashSet<DiffType> = COMPATIBLE_CHANGES.iter().copied().collect();
        let reversed_diffs = compare(new_not, old_not);
        let all_compatible = reversed_diffs
            .iter()
            .all(|(dt, _)| compatible_set.contains(dt));
        if all_compatible {
            diffs.push((DiffType::NotTypeNarrowed, "not type narrowed".into()));
        } else {
            diffs.push((DiffType::NotTypeExtended, "not type extended".into()));
        }
    }
}

fn compare_combined_keyword(
    old: &serde_json::Value,
    new: &serde_json::Value,
    keyword: &str,
    extended_type: DiffType,
    narrowed_type: DiffType,
    diffs: &mut Vec<(DiffType, String)>,
) {
    let old_arr = old.get(keyword).and_then(|v| v.as_array());
    let new_arr = new.get(keyword).and_then(|v| v.as_array());

    let compatible_set: HashSet<DiffType> = COMPATIBLE_CHANGES.iter().copied().collect();

    match (old_arr, new_arr) {
        (Some(old_items), Some(new_items)) => {
            // Cache all pairwise comparisons.
            let comparisons: Vec<Vec<Vec<(DiffType, String)>>> = old_items
                .iter()
                .map(|o| new_items.iter().map(|n| compare(o, n)).collect())
                .collect();
            let compat_matrix: Vec<Vec<bool>> = comparisons
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|d| d.iter().all(|(dt, _)| compatible_set.contains(dt)))
                        .collect()
                })
                .collect();

            // Confluent: bipartite matching detects incompatible subschema changes.
            let match_col =
                bipartite_matching_pairs(&compat_matrix, old_items.len(), new_items.len());
            let matching = match_col.iter().filter(|m| m.is_some()).count();
            let smaller = old_items.len().min(new_items.len());

            if matching < smaller {
                diffs.push((
                    DiffType::CombinedTypeSubschemasChanged,
                    format!(
                        "{keyword} subschemas changed incompatibly ({matching}/{smaller} matched)"
                    ),
                ));
            }

            // Confluent: length-based extended/narrowed.
            if old_items.len() < new_items.len() {
                diffs.push((extended_type, format!("{keyword} extended")));
            } else if old_items.len() > new_items.len() {
                diffs.push((narrowed_type, format!("{keyword} narrowed")));
            }

            // Emit sub-diffs for bipartite-matched pairs.
            for (col, opt_row) in match_col.iter().enumerate() {
                if let Some(row) = opt_row {
                    diffs.extend(comparisons[*row][col].clone());
                }
            }
        }
        (None, Some(_)) => diffs.push((extended_type, format!("{keyword} added"))),
        (Some(_), None) => diffs.push((narrowed_type, format!("{keyword} removed"))),
        _ => {}
    }
}

// -- Bipartite matching --

/// Bipartite matching via augmenting paths. Returns `match_col[c] = Some(r)` for each matched pair.
fn bipartite_matching_pairs(matrix: &[Vec<bool>], rows: usize, cols: usize) -> Vec<Option<usize>> {
    let mut match_col: Vec<Option<usize>> = vec![None; cols];
    for r in 0..rows {
        let mut visited = vec![false; cols];
        augment(matrix, r, &mut match_col, &mut visited);
    }
    match_col
}

fn augment(
    matrix: &[Vec<bool>],
    r: usize,
    match_col: &mut [Option<usize>],
    visited: &mut [bool],
) -> bool {
    for c in 0..match_col.len() {
        if matrix[r][c] && !visited[c] {
            visited[c] = true;
            if match_col[c].is_none() || augment(matrix, match_col[c].unwrap(), match_col, visited)
            {
                match_col[c] = Some(r);
                return true;
            }
        }
    }
    false
}

// -- Diff helpers --

fn check_value_changed(
    old: &serde_json::Value,
    new: &serde_json::Value,
    field: &str,
    diff_type: DiffType,
    diffs: &mut Vec<(DiffType, String)>,
) {
    if old.get(field) != new.get(field) {
        diffs.push((diff_type, format!("'{field}' changed")));
    }
}

type NumericDiffTypes = (DiffType, DiffType, DiffType, DiffType);

fn compare_numeric_field(
    old: &serde_json::Value,
    new: &serde_json::Value,
    field: &str,
    types: NumericDiffTypes,
    diffs: &mut Vec<(DiffType, String)>,
) {
    let old_val = old.get(field).and_then(serde_json::Value::as_f64);
    let new_val = new.get(field).and_then(serde_json::Value::as_f64);

    match (old_val, new_val) {
        (None, Some(_)) => diffs.push((types.0, format!("'{field}' added"))),
        (Some(_), None) => diffs.push((types.1, format!("'{field}' removed"))),
        (Some(o), Some(n)) if (o - n).abs() > f64::EPSILON => {
            if n > o {
                diffs.push((types.2, format!("'{field}' increased")));
            } else {
                diffs.push((types.3, format!("'{field}' decreased")));
            }
        }
        _ => {}
    }
}

/// Compare `multipleOf` using divisibility (Confluent semantics).
///
/// - `update % original == 0` → EXPANDED (less restrictive: every value matching new also matches old)
/// - `original % update == 0` → REDUCED (more restrictive)
/// - Otherwise → CHANGED
fn compare_multiple_of(
    old: &serde_json::Value,
    new: &serde_json::Value,
    diffs: &mut Vec<(DiffType, String)>,
) {
    let old_val = old.get("multipleOf").and_then(serde_json::Value::as_f64);
    let new_val = new.get("multipleOf").and_then(serde_json::Value::as_f64);

    match (old_val, new_val) {
        (None, Some(_)) => diffs.push((DiffType::MultipleOfAdded, "'multipleOf' added".into())),
        (Some(_), None) => diffs.push((DiffType::MultipleOfRemoved, "'multipleOf' removed".into())),
        (Some(o), Some(n)) if (o - n).abs() > f64::EPSILON => {
            if o != 0.0 && (n % o).abs() < f64::EPSILON {
                // new is a multiple of old → expanded (less restrictive)
                diffs.push((DiffType::MultipleOfExpanded, "'multipleOf' expanded".into()));
            } else if n != 0.0 && (o % n).abs() < f64::EPSILON {
                // old is a multiple of new → reduced (more restrictive)
                diffs.push((DiffType::MultipleOfReduced, "'multipleOf' reduced".into()));
            } else {
                diffs.push((DiffType::MultipleOfChanged, "'multipleOf' changed".into()));
            }
        }
        _ => {}
    }
}

fn compare_string_field(
    old: &serde_json::Value,
    new: &serde_json::Value,
    field: &str,
    added: DiffType,
    removed: DiffType,
    changed: DiffType,
    diffs: &mut Vec<(DiffType, String)>,
) {
    let old_val = old.get(field).and_then(|v| v.as_str());
    let new_val = new.get(field).and_then(|v| v.as_str());

    match (old_val, new_val) {
        (None, Some(_)) => diffs.push((added, format!("'{field}' added"))),
        (Some(_), None) => diffs.push((removed, format!("'{field}' removed"))),
        (Some(o), Some(n)) if o != n => diffs.push((changed, format!("'{field}' changed"))),
        _ => {}
    }
}

fn extract_types(schema: &serde_json::Value) -> HashSet<String> {
    match schema.get("type") {
        Some(serde_json::Value::String(s)) => {
            let mut set = HashSet::new();
            set.insert(s.clone());
            set
        }
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        _ => HashSet::new(),
    }
}

fn extract_string_set(schema: &serde_json::Value, field: &str) -> HashSet<String> {
    schema
        .get(field)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn extract_property_keys(schema: &serde_json::Value) -> HashSet<String> {
    schema
        .get("properties")
        .and_then(|p| p.as_object())
        .map(|obj| obj.keys().cloned().collect())
        .unwrap_or_default()
}

fn is_open_content_model(schema: &serde_json::Value) -> bool {
    !matches!(
        schema.get("additionalProperties"),
        Some(serde_json::Value::Bool(false))
    )
}

fn is_false_schema(schema: &serde_json::Value, prop: &str) -> bool {
    schema
        .get("properties")
        .and_then(|p| p.get(prop))
        .is_some_and(|v| v == &serde_json::Value::Bool(false))
}

fn is_empty_schema(schema: &serde_json::Value, prop: &str) -> bool {
    schema
        .get("properties")
        .and_then(|p| p.get(prop))
        .is_some_and(|v| {
            v == &serde_json::Value::Bool(true)
                || v.as_object().is_some_and(serde_json::Map::is_empty)
        })
}

fn is_more_restrictive(old: &serde_json::Value, new: &serde_json::Value) -> bool {
    match (old, new) {
        (
            serde_json::Value::Bool(true),
            serde_json::Value::Bool(false) | serde_json::Value::Object(_),
        ) => true,
        (serde_json::Value::Object(_), serde_json::Value::Object(_)) => {
            let compatible_set: HashSet<DiffType> = COMPATIBLE_CHANGES.iter().copied().collect();
            let diffs = compare(old, new);
            diffs.iter().any(|(dt, _)| !compatible_set.contains(dt))
        }
        _ => false,
    }
}

// -- Dependencies --

fn compare_dependencies(
    old: &serde_json::Value,
    new: &serde_json::Value,
    diffs: &mut Vec<(DiffType, String)>,
) {
    // Merge legacy "dependencies" with 2020-12 "dependentRequired" / "dependentSchemas".
    let old_merged = merge_dependencies(old);
    let new_merged = merge_dependencies(new);

    if old_merged.is_empty() && new_merged.is_empty() {
        return;
    }

    let emit_dep = |map: &serde_json::Map<String, serde_json::Value>,
                    added: bool,
                    diffs: &mut Vec<(DiffType, String)>| {
        for key in map.keys() {
            let dt = if map[key].is_array() {
                if added {
                    DiffType::DependencyArrayAdded
                } else {
                    DiffType::DependencyArrayRemoved
                }
            } else if added {
                DiffType::DependencySchemaAdded
            } else {
                DiffType::DependencySchemaRemoved
            };
            let verb = if added { "added" } else { "removed" };
            diffs.push((dt, format!("Dependency '{key}' {verb}")));
        }
    };

    if old_merged.is_empty() {
        emit_dep(&new_merged, true, diffs);
        return;
    }
    if new_merged.is_empty() {
        emit_dep(&old_merged, false, diffs);
        return;
    }

    let old_keys: HashSet<&String> = old_merged.keys().collect();
    let new_keys: HashSet<&String> = new_merged.keys().collect();

    for key in new_keys.difference(&old_keys) {
        if new_merged[key.as_str()].is_array() {
            diffs.push((
                DiffType::DependencyArrayAdded,
                format!("Dependency array '{key}' added"),
            ));
        } else {
            diffs.push((
                DiffType::DependencySchemaAdded,
                format!("Dependency schema '{key}' added"),
            ));
        }
    }

    for key in old_keys.difference(&new_keys) {
        if old_merged[key.as_str()].is_array() {
            diffs.push((
                DiffType::DependencyArrayRemoved,
                format!("Dependency array '{key}' removed"),
            ));
        } else {
            diffs.push((
                DiffType::DependencySchemaRemoved,
                format!("Dependency schema '{key}' removed"),
            ));
        }
    }

    for key in old_keys.intersection(&new_keys) {
        let old_val = &old_merged[key.as_str()];
        let new_val = &new_merged[key.as_str()];

        if old_val.is_array() && new_val.is_array() {
            let old_arr: HashSet<String> = old_val
                .as_array()
                .unwrap()
                .iter()
                .map(ToString::to_string)
                .collect();
            let new_arr: HashSet<String> = new_val
                .as_array()
                .unwrap()
                .iter()
                .map(ToString::to_string)
                .collect();
            if old_arr != new_arr {
                if new_arr.is_subset(&old_arr) {
                    diffs.push((
                        DiffType::DependencyArrayNarrowed,
                        format!("Dependency array '{key}' narrowed"),
                    ));
                } else if old_arr.is_subset(&new_arr) {
                    diffs.push((
                        DiffType::DependencyArrayExtended,
                        format!("Dependency array '{key}' extended"),
                    ));
                } else {
                    diffs.push((
                        DiffType::DependencyArrayChanged,
                        format!("Dependency array '{key}' changed"),
                    ));
                }
            }
        } else if old_val.is_object() && new_val.is_object() {
            diffs.extend(compare(old_val, new_val));
        }
    }
}

/// Merge legacy `dependencies` with 2020-12 `dependentRequired` and `dependentSchemas`.
fn merge_dependencies(schema: &serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    let mut result = serde_json::Map::new();
    if let Some(deps) = schema
        .get("dependencies")
        .and_then(serde_json::Value::as_object)
    {
        result.extend(deps.clone());
    }
    if let Some(deps) = schema
        .get("dependentRequired")
        .and_then(serde_json::Value::as_object)
    {
        result.extend(deps.clone());
    }
    if let Some(deps) = schema
        .get("dependentSchemas")
        .and_then(serde_json::Value::as_object)
    {
        result.extend(deps.clone());
    }
    result
}

// -- Partially open content model --

fn is_partially_open_content_model(schema: &serde_json::Value) -> bool {
    if schema
        .get("patternProperties")
        .and_then(serde_json::Value::as_object)
        .is_some_and(|m| !m.is_empty())
    {
        return true;
    }
    matches!(
        schema.get("additionalProperties"),
        Some(serde_json::Value::Object(_))
    )
}

/// Check if a property is "covered" by a partially open content model.
///
/// Confluent: compare the property schema against the additionalProperties/patternProperties
/// schema recursively. If the sub-comparison is compatible, the property is "covered".
/// Check if a property is "covered" by a partially open content model.
///
/// `is_removal`: true when a property is being removed (compare property→model),
/// false when a property is being added (compare model→property).
fn is_covered_by_partial_model(
    model_schema: &serde_json::Value,
    prop_name: &str,
    prop_schema: Option<&serde_json::Value>,
    is_removal: bool,
) -> bool {
    let compatible_set: HashSet<DiffType> = COMPATIBLE_CHANGES.iter().copied().collect();

    // Check patternProperties first.
    if let Some(patterns) = model_schema
        .get("patternProperties")
        .and_then(serde_json::Value::as_object)
    {
        for (pattern, pattern_schema) in patterns {
            if regex::Regex::new(pattern).is_ok_and(|re| re.is_match(prop_name)) {
                if let Some(ps) = prop_schema {
                    // Removal: model must be at least as permissive as removed property.
                    // Addition: added property must be at least as permissive as old model.
                    let sub_diffs = if is_removal {
                        compare(ps, pattern_schema)
                    } else {
                        compare(pattern_schema, ps)
                    };
                    return sub_diffs.iter().all(|(dt, _)| compatible_set.contains(dt));
                }
                return true;
            }
        }
    }

    // Fall back to additionalProperties schema.
    if let Some(serde_json::Value::Object(_)) = model_schema.get("additionalProperties") {
        if let (Some(ap), Some(ps)) = (model_schema.get("additionalProperties"), prop_schema) {
            let sub_diffs = if is_removal {
                compare(ps, ap)
            } else {
                compare(ap, ps)
            };
            return sub_diffs.iter().all(|(dt, _)| compatible_set.contains(dt));
        }
        return true;
    }

    false
}
