//! Validation and reference-discovery helpers for Schema Manager inputs.

use std::collections::BTreeSet;
use std::path::{Component, Path};

use serde_json::Value;

pub(crate) fn schema_references(value: &Value) -> Vec<&str> {
    let mut references = Vec::new();
    collect_schema_references(value, &mut references);
    references
}

fn collect_schema_references<'a>(value: &'a Value, references: &mut Vec<&'a str>) {
    match value {
        Value::Object(object) => {
            references.extend(
                ["$ref", "$dynamicRef"]
                    .iter()
                    .filter_map(|key| object.get(*key).and_then(Value::as_str)),
            );
            object
                .values()
                .for_each(|child| collect_schema_references(child, references));
        }
        Value::Array(array) => array
            .iter()
            .for_each(|child| collect_schema_references(child, references)),
        _ => {}
    }
}

pub(crate) fn local_schema_name(reference: &str) -> Option<&str> {
    let target = reference.split('#').next().unwrap_or_default();
    if target.is_empty() {
        return None;
    }
    let name = target.rsplit('/').next()?;
    name.ends_with(".schema.json").then_some(name)
}

pub(crate) fn validate_identity(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(format!("{field} must be a lowercase semantic identity"));
    }
    Ok(())
}

pub(crate) fn ensure_unique(field: &str, values: &[String]) -> Result<(), String> {
    let mut unique = BTreeSet::new();
    for value in values {
        if !unique.insert(value) {
            return Err(format!("duplicate {field}: {value}"));
        }
    }
    Ok(())
}

pub(crate) fn validate_schema_name(name: &str) -> Result<(), String> {
    if !name.ends_with(".schema.json") || Path::new(name).components().count() != 1 {
        return Err(format!(
            "schema name must be a basename ending in .schema.json: {name}"
        ));
    }
    Ok(())
}

pub(crate) fn validate_relative_path(field: &str, path: &str) -> Result<(), String> {
    let path = Path::new(path);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("{field} must be a normalized relative path"));
    }
    Ok(())
}
