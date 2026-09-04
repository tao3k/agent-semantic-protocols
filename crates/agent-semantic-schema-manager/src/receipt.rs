//! Public identity receipt and manager-owned bundle membership verification.

use std::fs;
use std::path::Path;

use agent_semantic_content_identity::exact_selector_merkle::canonical_content_digest;
use serde::Deserialize;
use serde::Serialize;

use crate::manager::BUNDLE_RECEIPT_SCHEMA_ID;
use crate::manager::LanguageSchemaBundleReceipt;
use crate::manager::SCHEMA_VERSION;
use crate::manager::SchemaBundleEntry;
use crate::manager_validation::validate_identity;
use crate::manager_validation::validate_schema_name;

pub(crate) const BUNDLE_MEMBERSHIP_FILE: &str = ".asp-schema-manager-membership.json";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SchemaBundleMembership {
    pub(crate) language_id: String,
    pub(crate) profile_digest: String,
    pub(crate) bundle_digest: String,
    pub(crate) schemas: Vec<SchemaBundleEntry>,
}

pub(super) fn schema_digest(bytes: &[u8]) -> String {
    tagged_content_digest(b"asp.language-schema-file.v1", &[bytes])
}

pub(super) fn tagged_content_digest(domain: &[u8], parts: &[&[u8]]) -> String {
    format!(
        "blake3-256:{}",
        canonical_content_digest(domain, parts).as_str()
    )
}

pub(super) fn read_receipt_if_present(
    path: &Path,
) -> Result<Option<LanguageSchemaBundleReceipt>, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "read schema bundle receipt {}: {error}",
                path.display()
            ));
        }
    };
    let receipt: LanguageSchemaBundleReceipt = serde_json::from_slice(&bytes)
        .map_err(|error| format!("decode schema bundle receipt {}: {error}", path.display()))?;
    let Some(schema_root) = path.parent() else {
        return Ok(Some(receipt));
    };
    let membership_path = schema_root.join(BUNDLE_MEMBERSHIP_FILE);
    let membership = match fs::read(&membership_path) {
        Ok(membership_bytes) => serde_json::from_slice::<SchemaBundleMembership>(&membership_bytes)
            .map_err(|error| format!("decode schema bundle membership: {error}"))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Some(receipt)),
        Err(error) => {
            return Err(format!(
                "read schema bundle membership {}: {error}",
                membership_path.display()
            ));
        }
    };
    Ok(Some(LanguageSchemaBundleReceipt {
        language_id: membership.language_id,
        profile_digest: membership.profile_digest,
        bundle_digest: membership.bundle_digest,
        schemas: membership.schemas,
        ..receipt
    }))
}

pub(super) fn verify_bundle_receipt_blocking(
    receipt_path: &Path,
) -> Result<LanguageSchemaBundleReceipt, String> {
    let receipt = read_receipt_if_present(receipt_path)?.ok_or_else(|| {
        format!(
            "schema bundle receipt is missing: {}",
            receipt_path.display()
        )
    })?;
    if receipt.schema_id != BUNDLE_RECEIPT_SCHEMA_ID || receipt.schema_version != SCHEMA_VERSION {
        return Err("schema bundle receipt identity is unsupported".to_owned());
    }
    validate_identity("languageId", &receipt.language_id)?;
    let schema_root = receipt_path.parent().ok_or_else(|| {
        format!(
            "schema bundle receipt has no schema root: {}",
            receipt_path.display()
        )
    })?;
    let membership_path = schema_root.join(BUNDLE_MEMBERSHIP_FILE);
    let membership_bytes = fs::read(&membership_path).map_err(|error| {
        format!(
            "read schema bundle membership {}: {error}",
            membership_path.display()
        )
    })?;
    let membership: SchemaBundleMembership = serde_json::from_slice(&membership_bytes)
        .map_err(|error| format!("decode schema bundle membership: {error}"))?;
    if membership.schemas.is_empty()
        || membership.bundle_digest != receipt.schema_digest
        || membership.language_id.is_empty()
    {
        return Err("schema bundle membership does not match public receipt".to_owned());
    }
    let mut previous_name: Option<&str> = None;
    for entry in &membership.schemas {
        validate_schema_name(&entry.name)?;
        if previous_name.is_some_and(|previous| previous >= entry.name.as_str()) {
            return Err("schema bundle receipt entries must be sorted and unique".to_owned());
        }
        previous_name = Some(&entry.name);
        let bytes = fs::read(schema_root.join(&entry.name))
            .map_err(|error| format!("read receipt-owned schema {}: {error}", entry.name))?;
        let actual = schema_digest(&bytes);
        if actual != entry.digest {
            return Err(format!(
                "schema bundle receipt digest mismatch for {}: expected={} actual={actual}",
                entry.name, entry.digest
            ));
        }
    }
    let bundle_bytes = serde_json::to_vec(&membership.schemas)
        .map_err(|error| format!("encode receipt schema entries: {error}"))?;
    let actual_bundle = tagged_content_digest(b"asp.language-schema-bundle.v1", &[&bundle_bytes]);
    if actual_bundle != receipt.schema_digest {
        return Err(format!(
            "schema bundle receipt schema digest mismatch: expected={} actual={actual_bundle}",
            receipt.schema_digest
        ));
    }
    Ok(LanguageSchemaBundleReceipt {
        language_id: membership.language_id,
        profile_digest: membership.profile_digest,
        bundle_digest: membership.bundle_digest,
        schemas: membership.schemas,
        ..receipt
    })
}
