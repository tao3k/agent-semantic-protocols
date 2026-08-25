//! Canonical Schema responsibility classification and ownership validation.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::manager::LanguageSchemaProfileRegistry;

/// Contractual owner family for a canonical Schema namespace.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SchemaFamily {
    pub family_id: String,
    pub owner: String,
    pub rationale: String,
    #[serde(default)]
    pub priority: i64,
    #[serde(default)]
    pub parent_family_id: Option<String>,
    pub namespace: SchemaFamilyNamespace,
    #[serde(default)]
    pub membership_overrides: SchemaFamilyMembershipOverrides,
    #[serde(default)]
    pub definition_schema_path: Option<String>,
    #[serde(default)]
    pub definition_visibility: Option<String>,
}

/// Filename and Schema identifier selectors owned by one family.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SchemaFamilyNamespace {
    #[serde(default)]
    pub filename_prefixes: Vec<String>,
    #[serde(default)]
    pub schema_identifier_prefixes: Vec<String>,
}

/// Explicit family membership exceptions for schemas that predate a namespace.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SchemaFamilyMembershipOverrides {
    #[serde(default)]
    pub include_schema_paths: Vec<String>,
    #[serde(default)]
    pub exclude_schema_paths: Vec<String>,
}

/// Fully resolved responsibility for one canonical Schema.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaResponsibility {
    pub name: String,
    pub schema_id: String,
    pub family_id: String,
    pub owner: String,
    pub purpose: String,
}

/// Accepted decision for structurally equal shapes owned by different families.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SchemaReferenceDecision {
    pub fingerprint: String,
    pub occurrence_set_digest: String,
    pub family_ids: Vec<String>,
    pub decision: String,
    pub review_state: String,
    pub owner: String,
    pub rationale: String,
}

pub(crate) fn audit_schema_responsibilities(
    workspace_root: &Path,
    registry: &LanguageSchemaProfileRegistry,
) -> Result<Vec<SchemaResponsibility>, String> {
    let families = validate_families(workspace_root, &registry.families)?;
    validate_reference_decisions(&families, &registry.reference_decisions)?;
    let paths = canonical_schema_paths(workspace_root)?;
    let mut schema_ids = BTreeMap::<String, String>::new();
    let mut purposes = BTreeMap::<String, String>::new();
    let mut responsibilities = Vec::with_capacity(paths.len());
    for path in paths {
        let name = schema_name(&path)?;
        let value = read_schema(&path, &name)?;
        let schema_id = required_schema_string(&value, "$id", &name)?;
        let purpose = required_schema_string(&value, "title", &name)?;
        insert_unique(
            &mut schema_ids,
            schema_id.clone(),
            &name,
            "canonical schema $id",
        )?;
        insert_unique(
            &mut purposes,
            purpose.clone(),
            &name,
            "schema responsibility purpose",
        )?;
        let registry_path = format!("schemas/{name}");
        let family = resolve_family(&registry.families, &registry_path, &name, &schema_id)?;
        responsibilities.push(SchemaResponsibility {
            name,
            schema_id,
            family_id: family.family_id.clone(),
            owner: family.owner.clone(),
            purpose,
        });
    }
    if responsibilities.is_empty() || families.is_empty() {
        return Err("schema responsibility registry must not be empty".to_owned());
    }
    Ok(responsibilities)
}

fn validate_reference_decisions(
    families: &BTreeMap<&str, &SchemaFamily>,
    decisions: &[SchemaReferenceDecision],
) -> Result<(), String> {
    let mut fingerprints = BTreeMap::new();
    for decision in decisions {
        validate_digest("reference decision fingerprint", &decision.fingerprint)?;
        validate_digest(
            "reference decision occurrenceSetDigest",
            &decision.occurrence_set_digest,
        )?;
        validate_nonempty("reference decision owner", &decision.owner)?;
        validate_nonempty("reference decision rationale", &decision.rationale)?;
        if decision.review_state != "accepted" {
            return Err(format!(
                "reference decision must be accepted: {}",
                decision.fingerprint
            ));
        }
        if !matches!(
            decision.decision.as_str(),
            "extract" | "defer" | "intentional-duplication"
        ) {
            return Err(format!(
                "unknown reference decision {}: {}",
                decision.fingerprint, decision.decision
            ));
        }
        for family_id in &decision.family_ids {
            if !families.contains_key(family_id.as_str()) {
                return Err(format!(
                    "reference decision {} names unknown family: {family_id}",
                    decision.fingerprint
                ));
            }
        }
        if fingerprints
            .insert(decision.fingerprint.as_str(), decision)
            .is_some()
        {
            return Err(format!(
                "duplicate reference decision fingerprint: {}",
                decision.fingerprint
            ));
        }
    }
    Ok(())
}

fn validate_digest(field: &str, value: &str) -> Result<(), String> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(format!("{field} must use sha256"));
    };
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("{field} must contain 64 hexadecimal digits"));
    }
    Ok(())
}

fn validate_families<'a>(
    workspace_root: &Path,
    registered: &'a [SchemaFamily],
) -> Result<BTreeMap<&'a str, &'a SchemaFamily>, String> {
    let mut families = BTreeMap::new();
    for family in registered {
        validate_family_identity(&family.family_id)?;
        validate_nonempty("schema family owner", &family.owner)?;
        validate_nonempty("schema family rationale", &family.rationale)?;
        if families.insert(family.family_id.as_str(), family).is_some() {
            return Err(format!("duplicate schema family: {}", family.family_id));
        }
    }
    for family in registered {
        if let Some(parent) = &family.parent_family_id {
            if !families.contains_key(parent.as_str()) {
                return Err(format!(
                    "unknown parent schema family {parent} for {}",
                    family.family_id
                ));
            }
        }
        validate_family_paths(workspace_root, family)?;
    }
    validate_parent_cycles(registered)?;
    Ok(families)
}

fn validate_parent_cycles(families: &[SchemaFamily]) -> Result<(), String> {
    let parents = families
        .iter()
        .filter_map(|family| {
            family
                .parent_family_id
                .as_deref()
                .map(|parent| (family.family_id.as_str(), parent))
        })
        .collect::<BTreeMap<_, _>>();
    for family in families {
        let mut visited = Vec::new();
        let mut current = family.family_id.as_str();
        while let Some(parent) = parents.get(current).copied() {
            if let Some(index) = visited.iter().position(|item| *item == parent) {
                visited.push(parent);
                return Err(format!(
                    "schema family parent cycle: {}",
                    visited[index..].join(",")
                ));
            }
            visited.push(current);
            current = parent;
        }
    }
    Ok(())
}

fn validate_family_paths(workspace_root: &Path, family: &SchemaFamily) -> Result<(), String> {
    for path in family
        .membership_overrides
        .include_schema_paths
        .iter()
        .chain(&family.membership_overrides.exclude_schema_paths)
    {
        validate_schema_registry_path(path)?;
        if !workspace_root.join(path).is_file() {
            return Err(format!(
                "schema family {} names missing schema: {path}",
                family.family_id
            ));
        }
    }
    Ok(())
}

fn canonical_schema_paths(workspace_root: &Path) -> Result<Vec<PathBuf>, String> {
    let schema_root = workspace_root.join("schemas");
    let mut paths = fs::read_dir(&schema_root)
        .map_err(|error| {
            format!(
                "read canonical schema root {}: {error}",
                schema_root.display()
            )
        })?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".schema.json"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}

fn schema_name(path: &Path) -> Result<String, String> {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(str::to_owned)
        .ok_or_else(|| format!("canonical schema name is not UTF-8: {}", path.display()))
}

fn read_schema(path: &Path, name: &str) -> Result<Value, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("read canonical schema {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("decode canonical schema {name}: {error}"))
}

fn insert_unique(
    values: &mut BTreeMap<String, String>,
    identity: String,
    name: &str,
    field: &str,
) -> Result<(), String> {
    if let Some(previous) = values.insert(identity.clone(), name.to_owned()) {
        return Err(format!("duplicate {field} {identity}: {previous},{name}"));
    }
    Ok(())
}

fn resolve_family<'a>(
    families: &'a [SchemaFamily],
    registry_path: &str,
    name: &str,
    schema_id: &str,
) -> Result<&'a SchemaFamily, String> {
    let matches = families
        .iter()
        .filter(|family| family_matches(family, registry_path, name, schema_id))
        .collect::<Vec<_>>();
    let priority = matches
        .iter()
        .map(|family| family.priority)
        .max()
        .ok_or_else(|| format!("schema responsibility is undeclared: {registry_path}"))?;
    let winners = matches
        .into_iter()
        .filter(|family| family.priority == priority)
        .collect::<Vec<_>>();
    if winners.len() != 1 {
        return Err(format!(
            "schema responsibility is ambiguous for {registry_path}: {}",
            winners
                .iter()
                .map(|family| family.family_id.as_str())
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    Ok(winners[0])
}

fn family_matches(family: &SchemaFamily, registry_path: &str, name: &str, schema_id: &str) -> bool {
    if family
        .membership_overrides
        .exclude_schema_paths
        .iter()
        .any(|path| path == registry_path)
    {
        return false;
    }
    family
        .membership_overrides
        .include_schema_paths
        .iter()
        .any(|path| path == registry_path)
        || family
            .namespace
            .filename_prefixes
            .iter()
            .any(|prefix| name.starts_with(prefix))
        || family
            .namespace
            .schema_identifier_prefixes
            .iter()
            .any(|prefix| schema_id.starts_with(prefix))
}

fn required_schema_string(value: &Value, field: &str, name: &str) -> Result<String, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("canonical schema {name} must declare non-empty {field}"))
}

fn validate_family_identity(value: &str) -> Result<(), String> {
    if value.is_empty()
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'.')
        })
    {
        return Err("schema familyId must be a lowercase dotted semantic identity".to_owned());
    }
    Ok(())
}

fn validate_nonempty(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{field} must be non-empty"));
    }
    Ok(())
}

fn validate_schema_registry_path(path: &str) -> Result<(), String> {
    let Some(name) = path.strip_prefix("schemas/") else {
        return Err(format!(
            "schema registry path must start with schemas/: {path}"
        ));
    };
    if !name.ends_with(".schema.json") || Path::new(name).components().count() != 1 {
        return Err(format!(
            "schema name must be a basename ending in .schema.json: {name}"
        ));
    }
    Ok(())
}
