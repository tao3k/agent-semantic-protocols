use std::collections::BTreeSet;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::protocol_identity::SCHEMA_VERSION;

pub const SCHEMA_BUNDLE_METHOD: &str = "asp.schema.bundle";
pub const SCHEMA_BUNDLE_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.asp-client-schema-bundle-request";
pub const SCHEMA_BUNDLE_RESPONSE_SCHEMA_ID: &str =
    "agent.semantic-protocols.asp-client-schema-bundle-response";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SchemaBundleRequest {
    pub schema_id: String,
    pub schema_version: String,
    pub language_id: String,
    pub root_set_ids: Vec<String>,
    #[serde(default)]
    pub known_bundle_digest: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    tag = "state",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SchemaBundleResponse {
    Ready {
        schema_id: String,
        schema_version: String,
        receipt: SchemaBundleReceipt,
        entries: Vec<SchemaBundleEntry>,
        documents: Vec<SchemaBundleDocument>,
    },
    Unchanged {
        schema_id: String,
        schema_version: String,
        receipt: SchemaBundleReceipt,
        entries: Vec<SchemaBundleEntry>,
    },
    Failed {
        schema_id: String,
        schema_version: String,
        language_id: String,
        reason_kind: String,
        recommended_next: Value,
        details: Value,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SchemaBundleReceipt {
    pub language_id: String,
    pub root_set_ids: Vec<String>,
    pub bundle_digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SchemaBundleEntry {
    pub family_id: String,
    pub schema_id: String,
    pub schema_version: String,
    pub name: String,
    pub digest: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SchemaBundleDocument {
    pub entry: SchemaBundleEntry,
    pub document: Value,
}

impl SchemaBundleRequest {
    pub fn validate(&self) -> Result<(), String> {
        validate_schema_identity(
            &self.schema_id,
            &self.schema_version,
            SCHEMA_BUNDLE_REQUEST_SCHEMA_ID,
        )?;
        validate_non_empty("languageId", &self.language_id)?;
        validate_root_set_ids(&self.root_set_ids)?;
        if let Some(digest) = &self.known_bundle_digest {
            validate_digest("knownBundleDigest", digest)?;
        }
        Ok(())
    }
}

impl SchemaBundleResponse {
    pub fn validate(&self) -> Result<(), String> {
        let (schema_id, schema_version) = match self {
            Self::Ready {
                schema_id,
                schema_version,
                ..
            }
            | Self::Unchanged {
                schema_id,
                schema_version,
                ..
            }
            | Self::Failed {
                schema_id,
                schema_version,
                ..
            } => (schema_id, schema_version),
        };
        validate_schema_identity(schema_id, schema_version, SCHEMA_BUNDLE_RESPONSE_SCHEMA_ID)?;
        match self {
            Self::Ready {
                receipt,
                entries,
                documents,
                ..
            } => {
                validate_receipt(receipt)?;
                validate_entries(entries)?;
                if documents.is_empty() {
                    return Err("ready schema bundle must carry schema documents".to_owned());
                }
                if documents.iter().any(|document| {
                    !document.document.is_object() && !document.document.is_boolean()
                }) {
                    return Err(
                        "schema bundle documents must be JSON Schema objects or booleans"
                            .to_owned(),
                    );
                }
                let document_entries = documents
                    .iter()
                    .map(|document| &document.entry)
                    .collect::<Vec<_>>();
                let canonical_entries = entries.iter().collect::<Vec<_>>();
                if document_entries != canonical_entries {
                    return Err(
                        "ready schema bundle documents must exactly bind receipt entry identities"
                            .to_owned(),
                    );
                }
            }
            Self::Unchanged {
                receipt, entries, ..
            } => {
                validate_receipt(receipt)?;
                validate_entries(entries)?;
            }
            Self::Failed {
                language_id,
                reason_kind,
                recommended_next,
                details,
                ..
            } => {
                validate_non_empty("languageId", language_id)?;
                validate_non_empty("reasonKind", reason_kind)?;
                if recommended_next.is_null() {
                    return Err("failed schema bundle requires recommendedNext".to_owned());
                }
                if !details.is_object() {
                    return Err("failed schema bundle details must be an object".to_owned());
                }
            }
        }
        Ok(())
    }
}

fn validate_receipt(receipt: &SchemaBundleReceipt) -> Result<(), String> {
    validate_non_empty("languageId", &receipt.language_id)?;
    validate_root_set_ids(&receipt.root_set_ids)?;
    validate_digest("bundleDigest", &receipt.bundle_digest)?;
    Ok(())
}

fn validate_entries(entries: &[SchemaBundleEntry]) -> Result<(), String> {
    if entries.is_empty() {
        return Err("schema bundle receipt must contain entries".to_owned());
    }
    let names = sorted_unique_names(
        entries.iter().map(|entry| entry.name.as_str()),
        "schema bundle receipt entries",
    )?;
    for (entry, name) in entries.iter().zip(names) {
        if entry.name != name {
            return Err("schema bundle receipt entries must be sorted by name".to_owned());
        }
        validate_non_empty("familyId", &entry.family_id)?;
        validate_non_empty("schemaId", &entry.schema_id)?;
        if entry.schema_version != SCHEMA_VERSION {
            return Err("schema bundle entry schemaVersion must be 1".to_owned());
        }
        validate_digest("schema digest", &entry.digest)?;
    }
    Ok(())
}

fn validate_root_set_ids(root_set_ids: &[String]) -> Result<(), String> {
    if root_set_ids.is_empty()
        || root_set_ids
            .iter()
            .any(|root_set| root_set.trim().is_empty())
    {
        return Err("rootSetIds must contain registered non-empty identities".to_owned());
    }
    if root_set_ids.iter().collect::<BTreeSet<_>>().len() != root_set_ids.len() {
        return Err("rootSetIds must be unique".to_owned());
    }
    Ok(())
}

fn sorted_unique_names<'a>(
    names: impl IntoIterator<Item = &'a str>,
    field: &str,
) -> Result<Vec<&'a str>, String> {
    let names = names.into_iter().collect::<Vec<_>>();
    let unique = names.iter().copied().collect::<BTreeSet<_>>();
    if unique.len() != names.len() || names.iter().any(|name| !valid_schema_name(name)) {
        return Err(format!(
            "{field} must use unique canonical .schema.json names"
        ));
    }
    Ok(unique.into_iter().collect())
}

fn valid_schema_name(name: &str) -> bool {
    !name.is_empty() && !name.contains('/') && name.ends_with(".schema.json")
}

fn validate_schema_identity(
    schema_id: &str,
    schema_version: &str,
    expected_schema_id: &str,
) -> Result<(), String> {
    if schema_id != expected_schema_id || schema_version != SCHEMA_VERSION {
        return Err(format!(
            "unsupported schema identity: schemaId={schema_id} schemaVersion={schema_version}"
        ));
    }
    Ok(())
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{field} must be non-empty"));
    }
    Ok(())
}

fn validate_digest(field: &str, value: &str) -> Result<(), String> {
    let Some((algorithm, hex)) = value.split_once(':') else {
        return Err(format!("{field} must be a tagged content digest"));
    };
    if !matches!(algorithm, "blake3-256" | "sha256")
        || hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(format!("{field} must be a canonical content digest"));
    }
    Ok(())
}
