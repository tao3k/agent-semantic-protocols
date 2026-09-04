//! Extracts typed schema-contract identities from canonical schema documents.

use std::collections::BTreeSet;

use serde_json::Value;

use crate::ArtifactJson;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
/// Stable schema identifier and version pair discovered in one contract.
pub struct SchemaContractIdentity {
    pub schema_id: String,
    pub schema_version: String,
}

/// Returns every schema identity reachable from the supplied canonical document.
pub fn schema_contract_identities(
    schema_name: &str,
    schema: &ArtifactJson,
) -> Result<Vec<SchemaContractIdentity>, String> {
    let mut identities = BTreeSet::new();
    collect_contract_identities(schema_name, schema.as_value(), &mut identities)?;
    Ok(identities
        .into_iter()
        .map(|(schema_id, schema_version)| SchemaContractIdentity {
            schema_id,
            schema_version,
        })
        .collect())
}

fn collect_contract_identities(
    schema_name: &str,
    value: &Value,
    identities: &mut BTreeSet<(String, String)>,
) -> Result<(), String> {
    if let Some(schema_id) = value
        .get("properties")
        .and_then(|properties| properties.get("schemaId"))
        .and_then(|property| property.get("const"))
        .and_then(Value::as_str)
    {
        let schema_version = value
            .get("properties")
            .and_then(|properties| properties.get("schemaVersion"))
            .and_then(|property| property.get("const"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| version_suffix(schema_id))
            .or_else(|| version_suffix(schema_name))
            .ok_or_else(|| {
                format!(
                    "schema contract identity has no explicit or .v<digits> version: schema={schema_name} schemaId={schema_id}"
                )
            })?;
        identities.insert((schema_id.to_owned(), schema_version));
    }

    match value {
        Value::Array(items) => {
            for item in items {
                collect_contract_identities(schema_name, item, identities)?;
            }
        }
        Value::Object(entries) => {
            for entry in entries.values() {
                collect_contract_identities(schema_name, entry, identities)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn version_suffix(value: &str) -> Option<String> {
    let value = value.strip_suffix(".schema.json").unwrap_or(value);
    let (_, version) = value.rsplit_once(".v")?;
    (!version.is_empty() && version.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| version.to_owned())
}
