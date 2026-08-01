use std::{
    fs,
    path::{Path, PathBuf},
};

use agent_semantic_config::{LanguageId, ProviderId};
use agent_semantic_content_identity::active_artifact_merkle_v1::ActiveArtifactKindV1;
use agent_semantic_content_identity::exact_selector_merkle::parse_content_digest_v1;

use super::{ActiveAspArtifactInput, active_provider_logical_path, canonical_regular_file};

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderInstallArtifactIdentityV1 {
    schema_id: String,
    provider: String,
    installed_path: PathBuf,
    installed_entrypoint_digest: Option<String>,
    installed_entrypoint_metadata_digest: String,
}

pub fn active_provider_artifact_input(
    project_root: &Path,
    language_id: &LanguageId,
    provider_id: &ProviderId,
    materialized_path: PathBuf,
) -> Result<ActiveAspArtifactInput, String> {
    let paths = agent_semantic_runtime::project_state_paths(project_root)?;
    active_provider_artifact_input_from_lock_dir(
        &paths.provider_lock_dir,
        language_id,
        provider_id,
        materialized_path,
    )
}

/// Validate one installed provider artifact against the typed lock receipt
/// owned by an explicitly resolved State Home.
pub fn active_provider_artifact_input_with_state_home(
    project_root: &Path,
    state_home: &Path,
    language_id: &LanguageId,
    provider_id: &ProviderId,
    materialized_path: PathBuf,
) -> Result<ActiveAspArtifactInput, String> {
    let paths =
        agent_semantic_runtime::project_state_paths_with_state_home(project_root, state_home)?;
    active_provider_artifact_input_from_lock_dir(
        &paths.provider_lock_dir,
        language_id,
        provider_id,
        materialized_path,
    )
}

pub(crate) fn installed_provider_artifact_digest(
    provider_lock_dir: &Path,
    language_id: &LanguageId,
    provider_id: &ProviderId,
    materialized_path: PathBuf,
) -> Result<String, String> {
    active_provider_artifact_input_from_lock_dir(
        provider_lock_dir,
        language_id,
        provider_id,
        materialized_path,
    )
    .map(|input| input.artifact_digest)
}

fn active_provider_artifact_input_from_lock_dir(
    provider_lock_dir: &Path,
    language_id: &LanguageId,
    provider_id: &ProviderId,
    materialized_path: PathBuf,
) -> Result<ActiveAspArtifactInput, String> {
    let canonical_materialized =
        canonical_regular_file(&materialized_path, "active provider binary")?;
    let direct_lock = provider_lock_dir.join(format!("{language_id}.lock.toml"));
    let mut lock_paths = Vec::new();
    if direct_lock.is_file() {
        lock_paths.push(direct_lock);
    }
    if provider_lock_dir.is_dir() {
        let entries = fs::read_dir(provider_lock_dir).map_err(|error| {
            format!(
                "failed to read provider lock registry {}: {error}",
                provider_lock_dir.display()
            )
        })?;
        for entry in entries {
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
        let identity: ProviderInstallArtifactIdentityV1 = toml::from_str(&contents)
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
        return Ok(ActiveAspArtifactInput {
            logical_path: active_provider_logical_path(language_id, provider_id),
            artifact_kind: ActiveArtifactKindV1::ProviderBinary,
            materialized_path: canonical_materialized,
            artifact_digest: digest,
        });
    }
    Err(format!(
        "provider install receipt is missing for active binary: language={language_id} provider={provider_id} path={}",
        canonical_materialized.display()
    ))
}
