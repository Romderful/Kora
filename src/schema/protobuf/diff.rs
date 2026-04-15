//! Protobuf structural diff engine (Confluent-compatible).
//!
//! Computes structural differences between two `.proto` schemas and classifies each
//! as compatible or incompatible using Confluent's `COMPATIBLE_CHANGES` set.
//!
//! Handles messages, enums, fields (type/name/label changes), oneofs,
//! wire-compatible scalar promotions, and external type resolution through
//! dependency registries for imported `.proto` files.

use std::collections::{HashMap, HashSet};

use crate::error::KoraError;

// -- Diff types --

/// A detected difference between two Protobuf schemas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
enum DiffType {
    PackageChanged,
    MessageAdded,
    MessageRemoved,
    MessageMoved,
    EnumAdded,
    EnumRemoved,
    EnumConstAdded,
    EnumConstChanged,
    EnumConstRemoved,
    FieldAdded,
    FieldRemoved,
    FieldNameChanged,
    FieldKindChanged,
    FieldScalarKindChanged,
    FieldNamedTypeChanged,
    FieldNumericLabelChanged,
    FieldStringOrBytesLabelChanged,
    RequiredFieldAdded,
    RequiredFieldRemoved,
    OneofAdded,
    OneofRemoved,
    OneofFieldAdded,
    OneofFieldRemoved,
    MultipleFieldsMovedToOneof,
    FieldMovedToExistingOneof,
}

/// Changes considered backward-compatible (matching Confluent's `COMPATIBLE_CHANGES`).
const COMPATIBLE_CHANGES: &[DiffType] = &[
    DiffType::MessageAdded,
    DiffType::MessageMoved,
    DiffType::EnumAdded,
    DiffType::EnumRemoved,
    DiffType::EnumConstAdded,
    DiffType::EnumConstChanged,
    DiffType::EnumConstRemoved,
    DiffType::FieldAdded,
    DiffType::FieldRemoved,
    DiffType::FieldNameChanged,
    DiffType::FieldStringOrBytesLabelChanged,
    DiffType::OneofAdded,
    DiffType::OneofRemoved,
    DiffType::OneofFieldAdded,
];

// -- Public entry point --

/// Check backward compatibility between two Protobuf schemas (old → new).
///
/// # Errors
///
/// Returns `KoraError::InvalidSchema` if either schema is malformed.
pub fn check(old: &str, new: &str) -> Result<(bool, Vec<String>), KoraError> {
    check_with_deps(old, new, &[], &[])
}

/// Check backward compatibility with dependency resolution.
///
/// Dependencies are `(filename, proto_content)` pairs representing imported files.
pub fn check_with_deps(
    old: &str,
    new: &str,
    old_deps: &[(String, String)],
    new_deps: &[(String, String)],
) -> Result<(bool, Vec<String>), KoraError> {
    let compatible_set: HashSet<DiffType> = COMPATIBLE_CHANGES.iter().copied().collect();
    let diffs = compare_with_deps(old, new, old_deps, new_deps)?;

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

// -- Dependency type registry --

/// Index of fully-qualified type name → parsed message descriptor from dependencies.
struct TypeRegistry {
    messages: HashMap<String, prost_types::DescriptorProto>,
}

impl TypeRegistry {
    fn from_deps(deps: &[(String, String)]) -> Result<Self, KoraError> {
        let mut messages = HashMap::new();
        for (name, schema) in deps {
            let file = protox_parse::parse(name, schema)
                .map_err(|e| KoraError::InvalidSchema(e.to_string()))?;
            let prefix = file.package.clone().unwrap_or_default();
            Self::index_messages(&file.message_type, &prefix, &mut messages);
        }
        Ok(TypeRegistry { messages })
    }

    fn index_messages(
        msgs: &[prost_types::DescriptorProto],
        prefix: &str,
        map: &mut HashMap<String, prost_types::DescriptorProto>,
    ) {
        for m in msgs {
            if let Some(name) = &m.name {
                let qualified = if prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{prefix}.{name}")
                };
                Self::index_messages(&m.nested_type, &qualified, map);
                map.insert(qualified, m.clone());
            }
        }
    }

    fn get(&self, qualified_name: &str) -> Option<&prost_types::DescriptorProto> {
        self.messages.get(qualified_name)
    }

    fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

// -- Diff engine --

/// Protobuf field type constants (from descriptor.proto).
const TYPE_INT64: i32 = 3;
const TYPE_UINT64: i32 = 4;
const TYPE_INT32: i32 = 5;
const TYPE_FIXED64: i32 = 6;
const TYPE_FIXED32: i32 = 7;
const TYPE_STRING: i32 = 9;
const TYPE_MESSAGE: i32 = 11;
const TYPE_BYTES: i32 = 12;
const TYPE_UINT32: i32 = 13;
const TYPE_ENUM: i32 = 14;
const TYPE_SFIXED32: i32 = 15;
const TYPE_SFIXED64: i32 = 16;
const TYPE_SINT32: i32 = 17;
const TYPE_SINT64: i32 = 18;
const LABEL_REQUIRED: i32 = 2;

/// Wire-compatible scalar type pairs (same encoding on the wire).
fn is_wire_compatible(a: i32, b: i32) -> bool {
    if a == b {
        return true;
    }
    // int32 ↔ int64, uint32 ↔ uint64, sint32 ↔ sint64, fixed32 ↔ fixed64, sfixed32 ↔ sfixed64
    let pair = (a.min(b), a.max(b));
    matches!(
        pair,
        (TYPE_INT64, TYPE_INT32)       // 3, 5
        | (TYPE_UINT64, TYPE_UINT32)   // 4, 13
        | (TYPE_SFIXED32, TYPE_SFIXED64) // 15, 16
        | (TYPE_SINT32, TYPE_SINT64)   // 17, 18
        | (TYPE_FIXED64, TYPE_FIXED32) // 6, 7
        | (TYPE_STRING, TYPE_BYTES)    // 9, 12
        | (TYPE_INT32, TYPE_ENUM) // 5, 14
    )
}

/// Collect all enum names (fully qualified) from a file descriptor, for type resolution.
fn collect_enum_names(
    enums: &[prost_types::EnumDescriptorProto],
    msgs: &[prost_types::DescriptorProto],
    prefix: &str,
) -> HashSet<String> {
    let mut names = HashSet::new();
    for e in enums {
        if let Some(name) = &e.name {
            names.insert(format!("{prefix}{name}"));
        }
    }
    for m in msgs {
        if let Some(name) = &m.name {
            let nested_prefix = format!("{prefix}{name}.");
            names.extend(collect_enum_names(
                &m.enum_type,
                &m.nested_type,
                &nested_prefix,
            ));
        }
    }
    names
}

/// Resolve a field's effective type using the file's enum definitions.
///
/// When `protox_parse` leaves `r#type` as None (for named type references),
/// check if the `type_name` refers to an enum (→ `TYPE_ENUM`) or message (→ `TYPE_MESSAGE`).
fn resolve_field_type(
    field: &prost_types::FieldDescriptorProto,
    enum_names: &HashSet<String>,
) -> i32 {
    if let Some(t) = field.r#type {
        return t;
    }
    if let Some(type_name) = &field.type_name {
        let qualified = type_name.trim_start_matches('.');
        if enum_names.contains(qualified) {
            return TYPE_ENUM;
        }
        return TYPE_MESSAGE;
    }
    0
}

/// Compare two Protobuf schemas and produce a list of differences.
fn compare_with_deps(
    old: &str,
    new: &str,
    old_deps: &[(String, String)],
    new_deps: &[(String, String)],
) -> Result<Vec<(DiffType, String)>, KoraError> {
    let old_file = protox_parse::parse("old.proto", old)
        .map_err(|e| KoraError::InvalidSchema(e.to_string()))?;
    let new_file = protox_parse::parse("new.proto", new)
        .map_err(|e| KoraError::InvalidSchema(e.to_string()))?;

    let mut diffs = Vec::new();

    if old_file.package != new_file.package {
        diffs.push((
            DiffType::PackageChanged,
            format!(
                "Package changed from '{}' to '{}'",
                old_file.package.as_deref().unwrap_or(""),
                new_file.package.as_deref().unwrap_or("")
            ),
        ));
    }

    // Build enum name sets for type resolution.
    let old_enums = collect_enum_names(&old_file.enum_type, &old_file.message_type, "");
    let new_enums = collect_enum_names(&new_file.enum_type, &new_file.message_type, "");

    compare_messages(
        &old_file.message_type,
        &new_file.message_type,
        &old_enums,
        &new_enums,
        &mut diffs,
    );
    compare_enums(&old_file.enum_type, &new_file.enum_type, &mut diffs);

    // Resolve dependencies and compare external type references.
    let old_registry = TypeRegistry::from_deps(old_deps)?;
    let new_registry = TypeRegistry::from_deps(new_deps)?;

    if !old_registry.is_empty() || !new_registry.is_empty() {
        compare_external_type_refs(
            &old_file.message_type,
            &new_file.message_type,
            &old_registry,
            &new_registry,
            &old_enums,
            &new_enums,
            &mut diffs,
        );
    } else {
        // Fallback heuristic when no dependencies provided: flag external types when imports change.
        let old_imports: HashSet<&str> = old_file.dependency.iter().map(String::as_str).collect();
        let new_imports: HashSet<&str> = new_file.dependency.iter().map(String::as_str).collect();
        if old_imports != new_imports {
            let old_local = collect_message_names(&old_file.message_type, "");
            let new_local = collect_message_names(&new_file.message_type, "");
            detect_import_type_changes(
                &old_file.message_type,
                &new_file.message_type,
                &old_local,
                &new_local,
                &old_enums,
                &new_enums,
                &mut diffs,
            );
        }
    }

    Ok(diffs)
}

// -- Messages --

fn compare_messages(
    old_msgs: &[prost_types::DescriptorProto],
    new_msgs: &[prost_types::DescriptorProto],
    old_enums: &HashSet<String>,
    new_enums: &HashSet<String>,
    diffs: &mut Vec<(DiffType, String)>,
) {
    let old_by_name: HashMap<&str, (usize, &prost_types::DescriptorProto)> = old_msgs
        .iter()
        .enumerate()
        .filter_map(|(i, m)| m.name.as_deref().map(|n| (n, (i, m))))
        .collect();
    let new_by_name: HashMap<&str, (usize, &prost_types::DescriptorProto)> = new_msgs
        .iter()
        .enumerate()
        .filter_map(|(i, m)| m.name.as_deref().map(|n| (n, (i, m))))
        .collect();

    for (name, (old_idx, old_msg)) in &old_by_name {
        if let Some((new_idx, new_msg)) = new_by_name.get(name) {
            if old_idx != new_idx {
                diffs.push((DiffType::MessageMoved, format!("Message '{name}' moved")));
            }
            compare_fields(
                &old_msg.field,
                &new_msg.field,
                old_enums,
                new_enums,
                name,
                diffs,
            );
            detect_oneof_field_moves(
                &old_msg.field,
                &new_msg.field,
                &old_msg.oneof_decl,
                &new_msg.oneof_decl,
                name,
                diffs,
            );
            compare_enums(&old_msg.enum_type, &new_msg.enum_type, diffs);
            compare_messages(
                &old_msg.nested_type,
                &new_msg.nested_type,
                old_enums,
                new_enums,
                diffs,
            );
            compare_oneofs(
                &old_msg.oneof_decl,
                &new_msg.oneof_decl,
                &old_msg.field,
                &new_msg.field,
                name,
                diffs,
            );
        } else {
            diffs.push((
                DiffType::MessageRemoved,
                format!("Message '{name}' removed"),
            ));
        }
    }

    for name in new_by_name.keys() {
        if !old_by_name.contains_key(name) {
            diffs.push((DiffType::MessageAdded, format!("Message '{name}' added")));
        }
    }
}

// -- Fields --

fn compare_fields(
    old_fields: &[prost_types::FieldDescriptorProto],
    new_fields: &[prost_types::FieldDescriptorProto],
    old_enums: &HashSet<String>,
    new_enums: &HashSet<String>,
    msg_name: &str,
    diffs: &mut Vec<(DiffType, String)>,
) {
    let old_by_num: HashMap<i32, &prost_types::FieldDescriptorProto> = old_fields
        .iter()
        .filter_map(|f| f.number.map(|n| (n, f)))
        .collect();
    let new_by_num: HashMap<i32, &prost_types::FieldDescriptorProto> = new_fields
        .iter()
        .filter_map(|f| f.number.map(|n| (n, f)))
        .collect();

    for (&num, old_f) in &old_by_num {
        if let Some(new_f) = new_by_num.get(&num) {
            compare_field_pair(old_f, new_f, old_enums, new_enums, msg_name, diffs);
        } else {
            let name = old_f.name.as_deref().unwrap_or("?");
            let dt = if old_f.label == Some(LABEL_REQUIRED) {
                DiffType::RequiredFieldRemoved
            } else {
                DiffType::FieldRemoved
            };
            diffs.push((dt, format!("{msg_name}.{name}: field removed")));
        }
    }

    for (&num, new_f) in &new_by_num {
        if !old_by_num.contains_key(&num) {
            let name = new_f.name.as_deref().unwrap_or("?");
            let dt = if new_f.label == Some(LABEL_REQUIRED) {
                DiffType::RequiredFieldAdded
            } else {
                DiffType::FieldAdded
            };
            diffs.push((dt, format!("{msg_name}.{name}: field added")));
        }
    }
}

fn compare_field_pair(
    old_f: &prost_types::FieldDescriptorProto,
    new_f: &prost_types::FieldDescriptorProto,
    old_enums: &HashSet<String>,
    new_enums: &HashSet<String>,
    msg_name: &str,
    diffs: &mut Vec<(DiffType, String)>,
) {
    let old_name = old_f.name.as_deref().unwrap_or("");
    let new_name = new_f.name.as_deref().unwrap_or("");

    if old_name != new_name {
        diffs.push((
            DiffType::FieldNameChanged,
            format!("{msg_name}.{old_name}: field name changed to '{new_name}'"),
        ));
    }

    let old_type = resolve_field_type(old_f, old_enums);
    let new_type = resolve_field_type(new_f, new_enums);
    if old_type != new_type && !is_wire_compatible(old_type, new_type) {
        let is_scalar = |t: i32| t != TYPE_MESSAGE && t != TYPE_ENUM;
        let dt = if is_scalar(old_type) && is_scalar(new_type) {
            DiffType::FieldScalarKindChanged
        } else {
            DiffType::FieldKindChanged
        };
        diffs.push((dt, format!("{msg_name}.{old_name}: field type changed")));
    }

    // Compare named types — skip if wire-compatible (e.g., enum↔int32).
    // Also check if resolved types differ (e.g., same short name but enum vs message).
    let named_type_differs = !type_names_equal(old_f.type_name.as_ref(), new_f.type_name.as_ref())
        || (old_f.type_name.is_some() && new_f.type_name.is_some() && old_type != new_type);
    if named_type_differs
        && (old_type == TYPE_MESSAGE
            || old_type == TYPE_ENUM
            || new_type == TYPE_MESSAGE
            || new_type == TYPE_ENUM)
        && !is_wire_compatible(old_type, new_type)
    {
        diffs.push((
            DiffType::FieldNamedTypeChanged,
            format!("{msg_name}.{old_name}: named type changed"),
        ));
    }

    let old_label = old_f.label.unwrap_or(1);
    let new_label = new_f.label.unwrap_or(1);
    if old_label != new_label {
        let is_length_delimited = old_type == TYPE_STRING
            || old_type == TYPE_BYTES
            || old_type == TYPE_MESSAGE
            || old_type == TYPE_ENUM;
        let dt = if is_length_delimited {
            DiffType::FieldStringOrBytesLabelChanged
        } else {
            DiffType::FieldNumericLabelChanged
        };
        diffs.push((dt, format!("{msg_name}.{old_name}: label changed")));
    }
}

// -- Oneof --

fn detect_oneof_field_moves(
    old_fields: &[prost_types::FieldDescriptorProto],
    new_fields: &[prost_types::FieldDescriptorProto],
    _old_oneofs: &[prost_types::OneofDescriptorProto],
    new_oneofs: &[prost_types::OneofDescriptorProto],
    msg_name: &str,
    diffs: &mut Vec<(DiffType, String)>,
) {
    let old_by_num: HashMap<i32, &prost_types::FieldDescriptorProto> = old_fields
        .iter()
        .filter_map(|f| f.number.map(|n| (n, f)))
        .collect();

    // Set of field numbers that were in any oneof in old.
    let old_oneof_field_nums: HashSet<i32> = old_fields
        .iter()
        .filter(|f| f.oneof_index.is_some())
        .filter_map(|f| f.number)
        .collect();

    // Group new fields by their oneof index.
    let mut new_oneof_groups: HashMap<i32, Vec<i32>> = HashMap::new();
    for f in new_fields {
        if let (Some(num), Some(oi)) = (f.number, f.oneof_index) {
            new_oneof_groups.entry(oi).or_default().push(num);
        }
    }

    let mut moved_to_new = 0u32;
    let mut moved_to_existing = 0u32;

    for new_f in new_fields {
        let Some(num) = new_f.number else { continue };
        let Some(oneof_idx) = new_f.oneof_index else {
            continue;
        };

        if old_by_num.get(&num).is_none_or(|f| f.oneof_index.is_some()) {
            continue; // not moved from regular to oneof
        }

        // Check if the new oneof contains any field that was in a oneof in old.
        // This detects moves to renamed oneofs (not just same-name oneofs).
        let has_existing_oneof_field = new_oneof_groups.get(&oneof_idx).is_some_and(|fields| {
            fields
                .iter()
                .any(|&n| n != num && old_oneof_field_nums.contains(&n))
        });

        if has_existing_oneof_field {
            moved_to_existing += 1;
        } else {
            moved_to_new += 1;
        }
    }

    if moved_to_existing > 0 || moved_to_new > 1 {
        let oneof_name = new_oneofs
            .iter()
            .find(|o| {
                let idx = new_oneofs.iter().position(|x| std::ptr::eq(x, *o));
                idx.is_some_and(|i| new_oneof_groups.contains_key(&i32::try_from(i).unwrap_or(0)))
            })
            .and_then(|o| o.name.as_deref())
            .unwrap_or("?");

        if moved_to_existing + moved_to_new > 1 {
            diffs.push((
                DiffType::MultipleFieldsMovedToOneof,
                format!("{msg_name}: multiple fields moved into oneof '{oneof_name}'"),
            ));
        } else {
            diffs.push((
                DiffType::FieldMovedToExistingOneof,
                format!("{msg_name}: field moved into existing oneof '{oneof_name}'"),
            ));
        }
    }
    // moved_to_new == 1 with no moved_to_existing → just ONEOF_ADDED (emitted by compare_oneofs)
}

fn compare_oneofs(
    old_oneofs: &[prost_types::OneofDescriptorProto],
    new_oneofs: &[prost_types::OneofDescriptorProto],
    old_fields: &[prost_types::FieldDescriptorProto],
    new_fields: &[prost_types::FieldDescriptorProto],
    msg_name: &str,
    diffs: &mut Vec<(DiffType, String)>,
) {
    let old_names: HashSet<&str> = old_oneofs
        .iter()
        .filter_map(|o| o.name.as_deref())
        .collect();
    let new_names: HashSet<&str> = new_oneofs
        .iter()
        .filter_map(|o| o.name.as_deref())
        .collect();

    for name in &new_names {
        if !old_names.contains(name) {
            diffs.push((
                DiffType::OneofAdded,
                format!("{msg_name}: oneof '{name}' added"),
            ));
        }
    }

    for name in &old_names {
        if !new_names.contains(name) {
            diffs.push((
                DiffType::OneofRemoved,
                format!("{msg_name}: oneof '{name}' removed"),
            ));
        }
    }

    for (idx, old_oneof) in old_oneofs.iter().enumerate() {
        let oneof_name = old_oneof.name.as_deref().unwrap_or("?");
        if !new_names.contains(oneof_name) {
            continue;
        }
        if let Some(ni) = new_oneofs
            .iter()
            .position(|o| o.name.as_deref() == Some(oneof_name))
        {
            let old_oneof_fields: HashSet<i32> = old_fields
                .iter()
                .filter(|f| f.oneof_index == Some(i32::try_from(idx).unwrap_or(0)))
                .filter_map(|f| f.number)
                .collect();
            let new_oneof_fields: HashSet<i32> = new_fields
                .iter()
                .filter(|f| f.oneof_index == Some(i32::try_from(ni).unwrap_or(0)))
                .filter_map(|f| f.number)
                .collect();

            for &num in new_oneof_fields.difference(&old_oneof_fields) {
                let fname = new_fields
                    .iter()
                    .find(|f| f.number == Some(num))
                    .and_then(|f| f.name.as_deref())
                    .unwrap_or("?");
                diffs.push((
                    DiffType::OneofFieldAdded,
                    format!("{msg_name}.{oneof_name}: field '{fname}' added to oneof"),
                ));
            }

            for &num in old_oneof_fields.difference(&new_oneof_fields) {
                let fname = old_fields
                    .iter()
                    .find(|f| f.number == Some(num))
                    .and_then(|f| f.name.as_deref())
                    .unwrap_or("?");
                diffs.push((
                    DiffType::OneofFieldRemoved,
                    format!("{msg_name}.{oneof_name}: field '{fname}' removed from oneof"),
                ));
            }
        }
    }
}

// -- Enums --

fn compare_enums(
    old_enums: &[prost_types::EnumDescriptorProto],
    new_enums: &[prost_types::EnumDescriptorProto],
    diffs: &mut Vec<(DiffType, String)>,
) {
    let old_by_name: HashMap<&str, &prost_types::EnumDescriptorProto> = old_enums
        .iter()
        .filter_map(|e| e.name.as_deref().map(|n| (n, e)))
        .collect();
    let new_by_name: HashMap<&str, &prost_types::EnumDescriptorProto> = new_enums
        .iter()
        .filter_map(|e| e.name.as_deref().map(|n| (n, e)))
        .collect();

    for (name, old_enum) in &old_by_name {
        if let Some(new_enum) = new_by_name.get(name) {
            compare_enum_values(old_enum, new_enum, name, diffs);
        } else {
            diffs.push((DiffType::EnumRemoved, format!("Enum '{name}' removed")));
        }
    }

    for name in new_by_name.keys() {
        if !old_by_name.contains_key(name) {
            diffs.push((DiffType::EnumAdded, format!("Enum '{name}' added")));
        }
    }
}

fn compare_enum_values(
    old_enum: &prost_types::EnumDescriptorProto,
    new_enum: &prost_types::EnumDescriptorProto,
    enum_name: &str,
    diffs: &mut Vec<(DiffType, String)>,
) {
    let old_by_name: HashMap<&str, i32> = old_enum
        .value
        .iter()
        .filter_map(|v| v.name.as_deref().map(|n| (n, v.number.unwrap_or(0))))
        .collect();
    let new_by_name: HashMap<&str, i32> = new_enum
        .value
        .iter()
        .filter_map(|v| v.name.as_deref().map(|n| (n, v.number.unwrap_or(0))))
        .collect();

    for (name, &old_num) in &old_by_name {
        if let Some(&new_num) = new_by_name.get(name) {
            if old_num != new_num {
                diffs.push((
                    DiffType::EnumConstChanged,
                    format!("{enum_name}.{name}: value changed from {old_num} to {new_num}"),
                ));
            }
        } else {
            diffs.push((
                DiffType::EnumConstRemoved,
                format!("{enum_name}.{name}: removed"),
            ));
        }
    }

    for name in new_by_name.keys() {
        if !old_by_name.contains_key(name) {
            diffs.push((
                DiffType::EnumConstAdded,
                format!("{enum_name}.{name}: added"),
            ));
        }
    }
}

// -- Helpers --

/// Collect all message names (fully qualified) from a file descriptor.
fn collect_message_names(msgs: &[prost_types::DescriptorProto], prefix: &str) -> HashSet<String> {
    let mut names = HashSet::new();
    for m in msgs {
        if let Some(name) = &m.name {
            let qualified = format!("{prefix}{name}");
            names.insert(qualified.clone());
            names.extend(collect_message_names(
                &m.nested_type,
                &format!("{qualified}."),
            ));
        }
    }
    names
}

/// Detect fields referencing external types when imports changed.
///
/// When import dependencies change, fields referencing types from those imports
/// may now resolve to different definitions, so we emit `FieldNamedTypeChanged`.
fn detect_import_type_changes(
    old_msgs: &[prost_types::DescriptorProto],
    new_msgs: &[prost_types::DescriptorProto],
    old_local_types: &HashSet<String>,
    new_local_types: &HashSet<String>,
    old_enums: &HashSet<String>,
    new_enums: &HashSet<String>,
    diffs: &mut Vec<(DiffType, String)>,
) {
    let old_by_name: HashMap<&str, &prost_types::DescriptorProto> = old_msgs
        .iter()
        .filter_map(|m| m.name.as_deref().map(|n| (n, m)))
        .collect();
    let new_by_name: HashMap<&str, &prost_types::DescriptorProto> = new_msgs
        .iter()
        .filter_map(|m| m.name.as_deref().map(|n| (n, m)))
        .collect();

    for (name, old_msg) in &old_by_name {
        let Some(new_msg) = new_by_name.get(name) else {
            continue;
        };
        let old_fields: HashMap<i32, &prost_types::FieldDescriptorProto> = old_msg
            .field
            .iter()
            .filter_map(|f| f.number.map(|n| (n, f)))
            .collect();
        let new_fields: HashMap<i32, &prost_types::FieldDescriptorProto> = new_msg
            .field
            .iter()
            .filter_map(|f| f.number.map(|n| (n, f)))
            .collect();

        for (&num, old_f) in &old_fields {
            let Some(new_f) = new_fields.get(&num) else {
                continue;
            };
            let Some(old_tn) = &old_f.type_name else {
                continue;
            };
            let Some(new_tn) = &new_f.type_name else {
                continue;
            };
            if old_tn != new_tn {
                continue; // already handled by compare_field_pair
            }
            // Same type_name but imports changed — check if type is external.
            let qualified = old_tn.trim_start_matches('.');
            let is_local = old_local_types.contains(qualified)
                || old_enums.contains(qualified)
                || new_local_types.contains(qualified)
                || new_enums.contains(qualified);
            if !is_local {
                let fname = old_f.name.as_deref().unwrap_or("?");
                diffs.push((
                    DiffType::FieldNamedTypeChanged,
                    format!("{name}.{fname}: named type changed (import changed)"),
                ));
            }
        }
    }
}

/// Compare external type references by resolving them through dependency registries.
///
/// When fields reference types defined in imported files, this resolves those types
/// and compares their definitions. If definitions differ, it emits `FieldNamedTypeChanged`
/// and sub-diffs for the changed fields within the dependency.
fn compare_external_type_refs(
    old_msgs: &[prost_types::DescriptorProto],
    new_msgs: &[prost_types::DescriptorProto],
    old_reg: &TypeRegistry,
    new_reg: &TypeRegistry,
    old_enums: &HashSet<String>,
    new_enums: &HashSet<String>,
    diffs: &mut Vec<(DiffType, String)>,
) {
    let old_by_name: HashMap<&str, &prost_types::DescriptorProto> = old_msgs
        .iter()
        .filter_map(|m| m.name.as_deref().map(|n| (n, m)))
        .collect();
    let new_by_name: HashMap<&str, &prost_types::DescriptorProto> = new_msgs
        .iter()
        .filter_map(|m| m.name.as_deref().map(|n| (n, m)))
        .collect();

    for (name, old_msg) in &old_by_name {
        let Some(new_msg) = new_by_name.get(name) else {
            continue;
        };
        let old_fields: HashMap<i32, &prost_types::FieldDescriptorProto> = old_msg
            .field
            .iter()
            .filter_map(|f| f.number.map(|n| (n, f)))
            .collect();
        let new_fields: HashMap<i32, &prost_types::FieldDescriptorProto> = new_msg
            .field
            .iter()
            .filter_map(|f| f.number.map(|n| (n, f)))
            .collect();

        for (&num, old_f) in &old_fields {
            let Some(new_f) = new_fields.get(&num) else {
                continue;
            };

            // Both must reference named types.
            let Some(old_tn) = &old_f.type_name else {
                continue;
            };
            let Some(new_tn) = &new_f.type_name else {
                continue;
            };

            let old_q = old_tn.trim_start_matches('.');
            let new_q = new_tn.trim_start_matches('.');

            // Resolve both through their dependency registries.
            let old_def = old_reg.get(old_q);
            let new_def = new_reg.get(new_q);

            match (old_def, new_def) {
                (Some(old_def), Some(new_def)) => {
                    // Both resolved — compare the dependency message definitions.
                    let sub_diffs =
                        compare_dep_message_fields(old_def, new_def, old_enums, new_enums);
                    if !sub_diffs.is_empty() {
                        let fname = old_f.name.as_deref().unwrap_or("?");
                        diffs.push((
                            DiffType::FieldNamedTypeChanged,
                            format!("{name}.{fname}: named type changed"),
                        ));
                        diffs.extend(sub_diffs);
                    }
                }
                (None, None) if old_q != new_q => {
                    // Neither resolved but names differ — flag as changed (import changed).
                    let fname = old_f.name.as_deref().unwrap_or("?");
                    diffs.push((
                        DiffType::FieldNamedTypeChanged,
                        format!("{name}.{fname}: named type changed"),
                    ));
                }
                _ => {}
            }
        }
    }
}

/// Compare field definitions within two dependency message descriptors.
fn compare_dep_message_fields(
    old_msg: &prost_types::DescriptorProto,
    new_msg: &prost_types::DescriptorProto,
    old_enums: &HashSet<String>,
    new_enums: &HashSet<String>,
) -> Vec<(DiffType, String)> {
    let mut diffs = Vec::new();
    let msg_name = new_msg.name.as_deref().unwrap_or("?");

    let old_fields: HashMap<i32, &prost_types::FieldDescriptorProto> = old_msg
        .field
        .iter()
        .filter_map(|f| f.number.map(|n| (n, f)))
        .collect();
    let new_fields: HashMap<i32, &prost_types::FieldDescriptorProto> = new_msg
        .field
        .iter()
        .filter_map(|f| f.number.map(|n| (n, f)))
        .collect();

    for (&num, old_f) in &old_fields {
        if let Some(new_f) = new_fields.get(&num) {
            let old_type = resolve_field_type(old_f, old_enums);
            let new_type = resolve_field_type(new_f, new_enums);
            if old_type != new_type && !is_wire_compatible(old_type, new_type) {
                let is_scalar = |t: i32| t != TYPE_MESSAGE && t != TYPE_ENUM;
                let dt = if is_scalar(old_type) && is_scalar(new_type) {
                    DiffType::FieldScalarKindChanged
                } else {
                    DiffType::FieldKindChanged
                };
                let fname = old_f.name.as_deref().unwrap_or("?");
                diffs.push((dt, format!("{msg_name}.{fname}: field type changed")));
            }
        }
    }

    // Recurse into nested messages.
    let old_nested: HashMap<&str, &prost_types::DescriptorProto> = old_msg
        .nested_type
        .iter()
        .filter_map(|m| m.name.as_deref().map(|n| (n, m)))
        .collect();
    let new_nested: HashMap<&str, &prost_types::DescriptorProto> = new_msg
        .nested_type
        .iter()
        .filter_map(|m| m.name.as_deref().map(|n| (n, m)))
        .collect();

    for (nested_name, old_nested_msg) in &old_nested {
        if let Some(new_nested_msg) = new_nested.get(nested_name) {
            diffs.extend(compare_dep_message_fields(
                old_nested_msg,
                new_nested_msg,
                old_enums,
                new_enums,
            ));
        }
    }

    diffs
}

/// Compare `type_name` strings with normalization.
///
/// Protobuf type names can be fully qualified (`.pkg.Type`) or short (`Type`).
/// Both refer to the same type.
fn type_names_equal(a: Option<&String>, b: Option<&String>) -> bool {
    fn short_name(s: &str) -> &str {
        s.rsplit('.').next().unwrap_or(s)
    }
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            if a == b {
                return true;
            }
            // Both fully qualified (leading dot): must match exactly.
            if a.starts_with('.') && b.starts_with('.') {
                return false;
            }
            // At least one is relative — fall back to short name comparison.
            short_name(a) == short_name(b)
        }
        _ => false,
    }
}
