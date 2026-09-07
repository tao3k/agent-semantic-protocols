// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::fs;
use std::path::Path;
use std::path::PathBuf;

use agent_semantic_config::LanguageId;
use agent_semantic_config::ProviderId;
use agent_semantic_content_identity::exact_selector_merkle::parse_content_digest_v1;

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderInstallArtifactIdentity {
    schema_id: String,
    provider: String,
    installed_path: PathBuf,
    installed_entrypoint_digest: Option<String>,
    installed_entrypoint_metadata_digest: String,
}

pub fn installed_provider_artifact_digest(
    provider_lock_dir: &Path,
    language_id: &LanguageId,
    provider_id: &ProviderId,
    materialized_path: PathBuf,
) -> Result<String, String> {
    let canonical_materialized =
        canonical_regular_file(&materialized_path, "active provider binary")?;
    let direct_lock = provider_lock_dir.join(format!("{language_id}.lock.toml"));
    let mut lock_paths = Vec::new();
    if direct_lock.is_file() {
        lock_paths.push(direct_lock);
    }
    if provider_lock_dir.is_dir() {
        for entry in fs::read_dir(provider_lock_dir).map_err(|error| {
            format!(
                "failed to read provider lock registry {}: {error}",
                provider_lock_dir.display()
            )
        })? {
            let path = entry
                .map_err(|error| format!("failed to read provider lock entry: {error}"))?
                .path();
            if path.extension().and_then(|value| value.to_str()) == Some("toml")
                && path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|name| name.ends_with(".lock.toml"))
                && !lock_paths.contains(&path)
            {
                lock_paths.push(path);
            }
        }
    }
    lock_paths.sort();
    for lock_path in lock_paths {
        let contents = fs::read_to_string(&lock_path)
            .map_err(|error| format!("failed to read {}: {error}", lock_path.display()))?;
        let identity: ProviderInstallArtifactIdentity = toml::from_str(&contents)
            .map_err(|error| format!("failed to parse {}: {error}", lock_path.display()))?;
        if identity.schema_id != "asp.provider-install-lock.v1"
            || identity.provider != provider_id.as_str()
        {
            continue;
        }
        let installed_path =
            canonical_regular_file(&identity.installed_path, "installed provider binary")?;
        if installed_path != canonical_materialized {
            continue;
        }
        let digest = identity.installed_entrypoint_digest.ok_or_else(|| {
            format!(
                "provider install receipt lacks installedEntrypointDigest: language={language_id} provider={provider_id} lock={}",
                lock_path.display()
            )
        })?;
        parse_content_digest_v1(&digest).map_err(|_| {
            format!(
                "provider install receipt has invalid installedEntrypointDigest: language={language_id} provider={provider_id} lock={}",
                lock_path.display()
            )
        })?;
        let current_metadata_digest =
            agent_semantic_content_identity::file_artifact_metadata_digest_v1(
                &canonical_materialized,
            )?;
        if identity.installed_entrypoint_metadata_digest != current_metadata_digest {
            return Err(format!(
                "provider install receipt metadata drift: language={language_id} provider={provider_id} lock={} expected={} actual={current_metadata_digest}",
                lock_path.display(),
                identity.installed_entrypoint_metadata_digest,
            ));
        }
        return Ok(digest);
    }
    Err(format!(
        "provider install receipt is missing for active binary: language={language_id} provider={provider_id} path={}",
        canonical_materialized.display()
    ))
}

fn canonical_regular_file(path: &Path, label: &str) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("failed to resolve {label} {}: {error}", path.display()))?;
    if !canonical.is_file() {
        return Err(format!(
            "{label} is not a regular file: {}",
            canonical.display()
        ));
    }
    Ok(canonical)
}
