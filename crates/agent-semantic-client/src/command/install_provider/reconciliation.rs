// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Qualified staged Provider inputs for atomic protocol reconciliation.

use std::path::{Path, PathBuf};

pub(super) fn qualified_staged_development_provider(
    state_home: &Path,
    developer_root: &Path,
    language_id: &str,
    provider_id: &str,
) -> Result<PathBuf, String> {
    let registration =
        crate::command::provider_install_registry::provider_install_registration(language_id)?;
    if registration.provider_id != provider_id {
        return Err(format!(
            "reasonKind=provider-reconciliation-identity-drift languageId={language_id} activeProvider={provider_id} registeredProvider={}",
            registration.provider_id
        ));
    }
    let layout = agent_semantic_artifacts::RuntimeArtifactStateLayout::new(state_home);
    let receipt_path = layout
        .provider_staging()
        .join("receipts")
        .join(format!("{language_id}.lock.toml"));
    let receipt_bytes = std::fs::read_to_string(&receipt_path).map_err(|error| {
        format!(
            "reasonKind=provider-reconciliation-receipt-unavailable path={} error={error}",
            receipt_path.display()
        )
    })?;
    let receipt: toml::Value = toml::from_str(&receipt_bytes).map_err(|error| {
        format!(
            "reasonKind=provider-reconciliation-receipt-invalid path={} error={error}",
            receipt_path.display()
        )
    })?;
    for (key, expected) in [
        ("schemaId", "asp.provider-install-lock.v1"),
        ("scope", "state-home"),
        ("language", language_id),
        ("provider", provider_id),
        ("sourceKind", "develop-workspace-tree"),
    ] {
        if receipt_string(&receipt, key)? != expected {
            return Err(format!(
                "reasonKind=provider-reconciliation-receipt-identity-drift field={key}"
            ));
        }
    }
    let receipt_root = canonical_receipt_path(&receipt, "checkoutRoot")?;
    let developer_root = developer_root.canonicalize().map_err(|error| {
        format!(
            "canonicalize Runtime developer root {}: {error}",
            developer_root.display()
        )
    })?;
    if receipt_root != developer_root {
        return Err("reasonKind=provider-reconciliation-receipt-developer-root-drift".to_owned());
    }
    let expected_provider_digest =
        crate::command::provider_install_registry::provider_install_registration_digest(
            &registration,
        )?;
    if receipt_string(&receipt, "providerDigest")? != expected_provider_digest {
        return Err("reasonKind=provider-reconciliation-receipt-registration-drift".to_owned());
    }
    let package_path = canonical_receipt_path(&receipt, "packagePath")?;
    let provider_artifact_root = layout
        .provider_staging()
        .join(provider_id)
        .join("artifacts/blake3-merkle-v1")
        .canonicalize()
        .map_err(|error| {
            format!(
                "reasonKind=provider-reconciliation-artifact-root-unavailable providerId={provider_id} error={error}"
            )
        })?;
    if !package_path.starts_with(&provider_artifact_root) {
        return Err("reasonKind=provider-reconciliation-artifact-escaped".to_owned());
    }
    let (artifact_digest, artifact_leaf_count) =
        super::workspace::artifact_snapshot(&package_path)?;
    if receipt_string(&receipt, "artifactDigest")? != artifact_digest
        || receipt_integer(&receipt, "artifactLeafCount")? != artifact_leaf_count as i64
    {
        return Err("reasonKind=provider-reconciliation-artifact-drift".to_owned());
    }
    let launcher = package_path
        .parent()
        .ok_or_else(|| "provider reconciliation package has no artifact directory".to_owned())?
        .join("launcher")
        .canonicalize()
        .map_err(|error| {
            format!(
                "reasonKind=provider-reconciliation-launcher-unavailable providerId={provider_id} error={error}"
            )
        })?;
    let launcher_digest = agent_semantic_content_identity::file_content_digest_v1(&launcher)?;
    if receipt_string(&receipt, "launcherDigest")? != launcher_digest {
        return Err("reasonKind=provider-reconciliation-launcher-drift".to_owned());
    }
    Ok(launcher)
}

fn receipt_string<'a>(receipt: &'a toml::Value, key: &str) -> Result<&'a str, String> {
    receipt
        .get(key)
        .and_then(toml::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("provider reconciliation receipt omitted `{key}`"))
}

fn receipt_integer(receipt: &toml::Value, key: &str) -> Result<i64, String> {
    receipt
        .get(key)
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| format!("provider reconciliation receipt omitted `{key}`"))
}

fn canonical_receipt_path(receipt: &toml::Value, key: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(receipt_string(receipt, key)?);
    if !path.is_absolute() {
        return Err(format!(
            "provider reconciliation receipt `{key}` is not absolute"
        ));
    }
    path.canonicalize().map_err(|error| {
        format!(
            "canonicalize provider reconciliation receipt `{key}` {}: {error}",
            path.display()
        )
    })
}

#[cfg(test)]
#[path = "../../../tests/unit/install_provider_reconciliation.rs"]
mod tests;
