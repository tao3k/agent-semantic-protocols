use agent_semantic_content_identity::active_artifact_merkle_v1::{
    ActiveArtifactKindV1, ActiveArtifactLeafV1, ActiveAspArtifactReceiptV1,
};
use agent_semantic_content_identity::exact_selector_merkle::{
    blake3_content_digest_v1, parse_content_digest_v1,
};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_config::{LanguageId, ProviderId};

const ACTIVE_ASP_ARTIFACT_RECEIPT_FILE: &str = "active-asp-artifact-receipt.v1.json";
const ACTIVE_ASP_ARTIFACT_SET_ID: &str = "asp-runtime";

#[derive(Debug, Clone)]
pub struct ActiveAspArtifactMaterialization {
    pub receipt_path: PathBuf,
    pub receipt: ActiveAspArtifactReceiptV1,
    pub artifact_byte_reads: usize,
    pub artifact_bytes_read: u64,
    pub receipt_writes: usize,
}

#[derive(Debug, Clone)]
pub struct ActiveAspArtifactInput {
    pub logical_path: String,
    pub artifact_kind: ActiveArtifactKindV1,
    pub materialized_path: PathBuf,
    pub artifact_digest: String,
}

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

#[derive(Clone, Debug, Eq, PartialEq)]
struct ActiveArtifactMetadataFingerprint {
    materialized_path: String,
    size_bytes: u64,
    modified_unix_nanos: u64,
    change_time_unix_nanos: Option<i64>,
}

#[derive(Clone, Debug)]
struct VerifiedActiveAspArtifactReceiptCacheEntry {
    receipt_path: PathBuf,
    receipt_size_bytes: u64,
    receipt_modified_unix_nanos: u64,
    receipt_change_time_unix_nanos: Option<i64>,
    activation_path: PathBuf,
    asp_paths: Vec<PathBuf>,
    leaf_fingerprints: Vec<ActiveArtifactMetadataFingerprint>,
    receipt: ActiveAspArtifactReceiptV1,
}

static VERIFIED_ACTIVE_ASP_ARTIFACT_RECEIPT_CACHE: OnceLock<
    Mutex<Option<VerifiedActiveAspArtifactReceiptCacheEntry>>,
> = OnceLock::new();

pub fn active_asp_artifact_receipt_path(activation_path: &Path) -> Result<PathBuf, String> {
    let parent = activation_path.parent().ok_or_else(|| {
        format!(
            "activation path has no state directory: {}",
            activation_path.display()
        )
    })?;
    Ok(parent.join(ACTIVE_ASP_ARTIFACT_RECEIPT_FILE))
}

pub fn active_exact_selector_fixture_artifact_input_v1(
    activation_path: &Path,
) -> Result<Option<ActiveAspArtifactInput>, String> {
    let receipt_path = active_asp_artifact_receipt_path(activation_path)?;
    let bytes = match fs::read(&receipt_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "failed to read active exact-selector receipt {}: {error}",
                receipt_path.display()
            ));
        }
    };
    let receipt: ActiveAspArtifactReceiptV1 = serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to parse {}: {error}", receipt_path.display()))?;
    receipt
        .validate()
        .map_err(|error| format!("invalid active ASP artifact receipt: {error:?}"))?;
    let Some(leaf) = receipt
        .leaves()
        .iter()
        .find(|leaf| leaf.artifact_kind() == ActiveArtifactKindV1::ExactSelectorGenerationFixture)
    else {
        return Ok(None);
    };
    verify_materialized_leaf(
        Path::new(leaf.materialized_path()),
        leaf,
        "exact-selector-generation-fixture",
        MaterializationMatchPolicy::Exact,
    )?;
    Ok(Some(ActiveAspArtifactInput {
        logical_path: leaf.logical_path().to_owned(),
        artifact_kind: leaf.artifact_kind(),
        materialized_path: PathBuf::from(leaf.materialized_path()),
        artifact_digest: leaf.artifact_digest().as_str().to_owned(),
    }))
}

pub fn reconcile_active_asp_artifact_receipt_from_materialized_set(
    activation_path: &Path,
) -> Result<bool, String> {
    let receipt_path = active_asp_artifact_receipt_path(activation_path)?;
    let receipt_bytes = fs::read(&receipt_path).map_err(|error| {
        format!(
            "failed to read active ASP artifact receipt {}: {error}",
            receipt_path.display()
        )
    })?;
    let previous_receipt: ActiveAspArtifactReceiptV1 = serde_json::from_slice(&receipt_bytes)
        .map_err(|error| {
            format!(
                "failed to parse active ASP artifact receipt {}: {error}",
                receipt_path.display()
            )
        })?;
    previous_receipt.validate().map_err(|error| {
        format!(
            "invalid active ASP artifact receipt {}: {error:?}",
            receipt_path.display()
        )
    })?;

    let mut leaves = Vec::with_capacity(previous_receipt.leaves().len());
    for leaf in previous_receipt.leaves() {
        let materialized_path = if leaf.artifact_kind() == ActiveArtifactKindV1::Activation {
            activation_path
        } else {
            Path::new(leaf.materialized_path())
        };
        let canonical =
            canonical_regular_file(materialized_path, leaf.artifact_kind().canonical_name())?;
        let metadata = fs::metadata(&canonical)
            .map_err(|error| format!("failed to inspect {}: {error}", canonical.display()))?;
        let size_bytes = metadata.len();
        let expected_modified_unix_nanos = modified_unix_nanos(&metadata)?;
        let expected_change_time_unix_nanos = change_time_unix_nanos(&metadata);
        if canonical == Path::new(leaf.materialized_path())
            && size_bytes == leaf.size_bytes()
            && expected_modified_unix_nanos == leaf.modified_unix_nanos()
            && expected_change_time_unix_nanos == leaf.change_time_unix_nanos()
        {
            leaves.push(leaf.clone());
            continue;
        }
        let bytes = fs::read(&canonical)
            .map_err(|error| format!("failed to read {}: {error}", canonical.display()))?;
        let verified_metadata = fs::metadata(&canonical)
            .map_err(|error| format!("failed to re-inspect {}: {error}", canonical.display()))?;
        let verified_size_bytes = verified_metadata.len();
        let verified_modified_unix_nanos = modified_unix_nanos(&verified_metadata)?;
        let verified_change_time_unix_nanos = change_time_unix_nanos(&verified_metadata);
        if verified_size_bytes != size_bytes
            || verified_modified_unix_nanos != expected_modified_unix_nanos
            || verified_change_time_unix_nanos != expected_change_time_unix_nanos
        {
            return Err(format!(
                "active artifact changed during reconciliation: {}",
                canonical.display()
            ));
        }
        leaves.push(ActiveArtifactLeafV1::new(
            leaf.logical_path().to_string(),
            utf8_path(&canonical, leaf.artifact_kind().canonical_name())?,
            leaf.artifact_kind(),
            blake3_content_digest_v1(&bytes),
            verified_size_bytes,
            verified_modified_unix_nanos,
            verified_change_time_unix_nanos,
        )?);
    }

    let receipt = ActiveAspArtifactReceiptV1::build(ACTIVE_ASP_ARTIFACT_SET_ID, leaves)
        .map_err(|error| format!("failed to build active ASP artifact receipt: {error:?}"))?;
    verify_active_provider_artifact_closure(activation_path, &receipt)?;
    if receipt == previous_receipt {
        return Ok(false);
    }
    let bytes = serde_json::to_vec_pretty(&receipt)
        .map_err(|error| format!("failed to encode active ASP artifact receipt: {error}"))?;
    atomic_write(&receipt_path, &bytes)?;
    Ok(true)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActiveAspArtifactReconciliationV1 {
    NotMaterialized,
    Current,
    Updated,
}

impl ActiveAspArtifactReconciliationV1 {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotMaterialized => "not-materialized",
            Self::Current => "current",
            Self::Updated => "updated",
        }
    }
}

pub fn reconcile_active_asp_artifact_receipt_if_present(
    activation_path: &Path,
) -> Result<ActiveAspArtifactReconciliationV1, String> {
    let receipt_path = active_asp_artifact_receipt_path(activation_path)?;
    if !receipt_path
        .try_exists()
        .map_err(|error| format!("failed to inspect {}: {error}", receipt_path.display()))?
    {
        return Ok(ActiveAspArtifactReconciliationV1::NotMaterialized);
    }
    if reconcile_active_asp_artifact_receipt_from_materialized_set(activation_path)? {
        Ok(ActiveAspArtifactReconciliationV1::Updated)
    } else {
        Ok(ActiveAspArtifactReconciliationV1::Current)
    }
}

pub fn rebind_active_asp_binary_receipt_if_present(
    binary_path: &Path,
    binary_digest: &str,
    activation_path: &Path,
) -> Result<ActiveAspArtifactReconciliationV1, String> {
    if !activation_path
        .try_exists()
        .map_err(|error| format!("failed to inspect {}: {error}", activation_path.display()))?
    {
        return Ok(ActiveAspArtifactReconciliationV1::NotMaterialized);
    }
    let receipt_path = active_asp_artifact_receipt_path(activation_path)?;
    if !receipt_path
        .try_exists()
        .map_err(|error| format!("failed to inspect {}: {error}", receipt_path.display()))?
    {
        return Ok(ActiveAspArtifactReconciliationV1::NotMaterialized);
    }
    let receipt_bytes = fs::read(&receipt_path).map_err(|error| {
        format!(
            "failed to read active ASP artifact receipt {}: {error}",
            receipt_path.display()
        )
    })?;
    let previous_receipt: ActiveAspArtifactReceiptV1 = serde_json::from_slice(&receipt_bytes)
        .map_err(|error| {
            format!(
                "failed to parse active ASP artifact receipt {}: {error}",
                receipt_path.display()
            )
        })?;
    previous_receipt.validate().map_err(|error| {
        format!(
            "invalid active ASP artifact receipt {}: {error:?}",
            receipt_path.display()
        )
    })?;
    let additional_artifacts = previous_receipt
        .leaves()
        .iter()
        .filter(|leaf| {
            !matches!(
                leaf.artifact_kind(),
                ActiveArtifactKindV1::AspBinary | ActiveArtifactKindV1::Activation
            )
        })
        .map(|leaf| ActiveAspArtifactInput {
            logical_path: leaf.logical_path().to_string(),
            artifact_kind: leaf.artifact_kind(),
            materialized_path: PathBuf::from(leaf.materialized_path()),
            artifact_digest: leaf.artifact_digest().as_str().to_string(),
        })
        .collect::<Vec<_>>();
    let materialization = materialize_active_asp_artifact_receipt(
        binary_path,
        binary_digest,
        activation_path,
        &additional_artifacts,
    )?;
    if materialization.receipt_writes == 0 {
        Ok(ActiveAspArtifactReconciliationV1::Current)
    } else {
        Ok(ActiveAspArtifactReconciliationV1::Updated)
    }
}

#[cfg(test)]
#[path = "../tests/unit/active_artifact_receipt_reconciliation.rs"]
mod active_artifact_receipt_reconciliation_tests;

pub fn materialize_active_asp_artifact_receipt(
    binary_path: &Path,
    binary_digest: &str,
    activation_path: &Path,
    additional_artifacts: &[ActiveAspArtifactInput],
) -> Result<ActiveAspArtifactMaterialization, String> {
    let binary_path = canonical_regular_file(binary_path, "ASP binary")?;
    let activation_path = canonical_regular_file(activation_path, "activation")?;
    let receipt_path = active_asp_artifact_receipt_path(&activation_path)?;
    let previous_receipt = if receipt_path.is_file() {
        let bytes = fs::read(&receipt_path).map_err(|error| {
            format!(
                "failed to read active ASP artifact receipt {}: {error}",
                receipt_path.display()
            )
        })?;
        let receipt: ActiveAspArtifactReceiptV1 =
            serde_json::from_slice(&bytes).map_err(|error| {
                format!(
                    "failed to decode active ASP artifact receipt {}: {error}",
                    receipt_path.display()
                )
            })?;
        receipt.validate().map_err(|error| {
            format!(
                "invalid active ASP artifact receipt {}: {error:?}",
                receipt_path.display()
            )
        })?;
        Some(receipt)
    } else {
        None
    };
    let binary_metadata = fs::metadata(&binary_path)
        .map_err(|error| format!("failed to inspect {}: {error}", binary_path.display()))?;
    let activation_bytes = fs::read(&activation_path)
        .map_err(|error| format!("failed to read {}: {error}", activation_path.display()))?;
    let binary_digest = parse_content_digest_v1(binary_digest)
        .map_err(|_| format!("invalid BLAKE3 ASP binary digest: {binary_digest}"))?;
    let artifact_byte_reads = 0;
    let artifact_bytes_read = 0;
    let mut leaves = vec![
        ActiveArtifactLeafV1::new(
            "runtime/asp",
            utf8_path(&binary_path, "ASP binary")?,
            ActiveArtifactKindV1::AspBinary,
            binary_digest,
            binary_metadata.len(),
            modified_unix_nanos(&binary_metadata)?,
            change_time_unix_nanos(&binary_metadata),
        )?,
        ActiveArtifactLeafV1::new(
            "state/activation.json",
            utf8_path(&activation_path, "activation")?,
            ActiveArtifactKindV1::Activation,
            blake3_content_digest_v1(&activation_bytes),
            activation_bytes.len() as u64,
            modified_unix_nanos(&fs::metadata(&activation_path).map_err(|error| {
                format!("failed to inspect {}: {error}", activation_path.display())
            })?)?,
            change_time_unix_nanos(&fs::metadata(&activation_path).map_err(|error| {
                format!("failed to inspect {}: {error}", activation_path.display())
            })?),
        )?,
    ];
    for artifact in additional_artifacts {
        if matches!(
            artifact.artifact_kind,
            ActiveArtifactKindV1::AspBinary | ActiveArtifactKindV1::Activation
        ) {
            return Err(format!(
                "additional active artifact cannot duplicate required kind: {:?}",
                artifact.artifact_kind
            ));
        }
        let materialized_path =
            canonical_regular_file(&artifact.materialized_path, "active artifact")?;
        let metadata = fs::metadata(&materialized_path).map_err(|error| {
            format!(
                "failed to inspect active artifact {}: {error}",
                materialized_path.display()
            )
        })?;
        let materialized_path = utf8_path(&materialized_path, "active artifact")?;
        let modified_unix_nanos = modified_unix_nanos(&metadata)?;
        let change_time_unix_nanos = change_time_unix_nanos(&metadata);
        let artifact_digest = parse_content_digest_v1(&artifact.artifact_digest).map_err(|_| {
            format!(
                "invalid BLAKE3 active artifact digest: logicalPath={} digest={}",
                artifact.logical_path, artifact.artifact_digest
            )
        })?;
        leaves.push(ActiveArtifactLeafV1::new(
            artifact.logical_path.clone(),
            materialized_path,
            artifact.artifact_kind,
            artifact_digest,
            metadata.len(),
            modified_unix_nanos,
            change_time_unix_nanos,
        )?);
    }
    let receipt = ActiveAspArtifactReceiptV1::build(ACTIVE_ASP_ARTIFACT_SET_ID, leaves)
        .map_err(|error| format!("failed to build active ASP artifact receipt: {error:?}"))?;
    if previous_receipt.as_ref() == Some(&receipt) {
        return Ok(ActiveAspArtifactMaterialization {
            receipt_path,
            receipt,
            artifact_byte_reads,
            artifact_bytes_read,
            receipt_writes: 0,
        });
    }
    let bytes = serde_json::to_vec_pretty(&receipt)
        .map_err(|error| format!("failed to encode active ASP artifact receipt: {error}"))?;
    atomic_write(&receipt_path, &bytes)?;
    Ok(ActiveAspArtifactMaterialization {
        receipt_path,
        receipt,
        artifact_byte_reads,
        artifact_bytes_read,
        receipt_writes: 1,
    })
}

pub fn materialize_active_asp_artifact_receipt_for_current_process(
    activation_path: &Path,
    activation: &crate::HookRuntime,
) -> Result<bool, String> {
    let ranker = activation
        .rankers
        .iter()
        .find(|ranker| ranker.ranker_id == "asp-graph-turbo")
        .ok_or_else(|| {
            "activation has no producer-owned ASP graph-turbo binary identity".to_string()
        })?;
    let canonical = canonical_regular_file(Path::new(&ranker.binary), "ASP binary")?;
    let current_metadata =
        agent_semantic_content_identity::file_artifact_metadata_digest_v1(&canonical)?.to_string();
    if current_metadata != ranker.artifact_metadata_digest {
        return Err(format!(
            "ASP graph-turbo binary metadata drift: binary={} expected={} actual={current_metadata}",
            canonical.display(),
            ranker.artifact_metadata_digest
        ));
    }
    let project_root = Path::new(&activation.project_root);
    let runtime_profiles = crate::runtime_profiles_for_runtime(project_root, activation);
    let mut provider_artifacts = activation
        .providers
        .iter()
        .map(|provider| {
            let profile = runtime_profiles
                .providers
                .iter()
                .find(|profile| {
                    profile.language_id == provider.language_id.as_str()
                        && profile.provider_id == provider.provider_id.as_str()
                })
                .ok_or_else(|| {
                    format!(
                        "active provider runtime profile is missing: language={} provider={}",
                        provider.language_id, provider.provider_id
                    )
                })?;
            let binary = profile.resolved_binary.as_ref().ok_or_else(|| {
                format!(
                    "active provider runtime profile has no resolved binary: language={} provider={}",
                    provider.language_id, provider.provider_id
                )
            })?;
            active_provider_artifact_input(
                project_root,
                &provider.language_id,
                &provider.provider_id,
                PathBuf::from(binary),
            )
        })
        .collect::<Result<Vec<_>, String>>()?;
    let runtime_config = crate::default_client_config_path(&activation.project_root);
    if runtime_config.is_file() {
        let artifact_digest =
            agent_semantic_content_identity::file_content_digest_v1(&runtime_config)?;
        provider_artifacts.push(ActiveAspArtifactInput {
            logical_path: "runtime/hooks/config.toml".to_string(),
            artifact_kind: ActiveArtifactKindV1::RuntimeConfig,
            materialized_path: runtime_config,
            artifact_digest,
        });
    }
    materialize_active_asp_artifact_receipt(
        &canonical,
        &ranker.content_digest,
        activation_path,
        &provider_artifacts,
    )?;
    Ok(true)
}

fn active_provider_logical_path(language_id: &LanguageId, provider_id: &ProviderId) -> String {
    format!("providers/{language_id}/{provider_id}")
}

#[derive(serde::Deserialize)]
struct ActiveProviderClosureActivation {
    #[serde(default)]
    providers: Vec<ActiveProviderClosureIdentity>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActiveProviderClosureIdentity {
    language_id: LanguageId,
    provider_id: ProviderId,
}

fn verify_active_provider_artifact_closure(
    activation_path: &Path,
    receipt: &ActiveAspArtifactReceiptV1,
) -> Result<(), String> {
    let activation_bytes = fs::read(activation_path).map_err(|error| {
        format!(
            "failed to read active provider closure {}: {error}",
            activation_path.display()
        )
    })?;
    let activation: ActiveProviderClosureActivation = serde_json::from_slice(&activation_bytes)
        .map_err(|error| {
            format!(
                "failed to parse active provider closure {}: {error}",
                activation_path.display()
            )
        })?;
    let expected = activation
        .providers
        .iter()
        .map(|provider| active_provider_logical_path(&provider.language_id, &provider.provider_id))
        .collect::<std::collections::BTreeSet<_>>();
    let actual = receipt
        .leaves()
        .iter()
        .filter(|leaf| leaf.artifact_kind() == ActiveArtifactKindV1::ProviderBinary)
        .map(|leaf| leaf.logical_path().to_string())
        .collect::<std::collections::BTreeSet<_>>();
    if actual != expected {
        return Err(format!(
            "active ASP provider artifact closure mismatch: expected={} actual={}",
            expected.into_iter().collect::<Vec<_>>().join(","),
            actual.into_iter().collect::<Vec<_>>().join(","),
        ));
    }
    Ok(())
}

pub fn verify_active_asp_artifact_receipt(
    activation_path: &Path,
    asp_paths: &[&Path],
) -> Result<ActiveAspArtifactReceiptV1, String> {
    let receipt_path = active_asp_artifact_receipt_path(activation_path)?;
    let receipt_metadata = fs::metadata(&receipt_path)
        .map_err(|error| format!("failed to inspect {}: {error}", receipt_path.display()))?;
    if let Some(receipt) = verified_active_receipt_cache_hit(
        &receipt_path,
        &receipt_metadata,
        activation_path,
        asp_paths,
    )? {
        return Ok(receipt);
    }

    let bytes = fs::read(&receipt_path)
        .map_err(|error| format!("failed to read {}: {error}", receipt_path.display()))?;
    let receipt: ActiveAspArtifactReceiptV1 = serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to parse {}: {error}", receipt_path.display()))?;
    receipt
        .validate()
        .map_err(|error| format!("invalid active ASP artifact receipt: {error:?}"))?;

    let mut leaf_fingerprints = Vec::with_capacity(receipt.leaves().len());
    leaf_fingerprints.push(verify_materialized_leaf(
        activation_path,
        receipt.activation_leaf(),
        "activation",
        MaterializationMatchPolicy::Exact,
    )?);
    for asp_path in asp_paths {
        leaf_fingerprints.push(verify_materialized_leaf(
            asp_path,
            receipt.asp_binary_leaf(),
            "ASP binary",
            MaterializationMatchPolicy::ContentEquivalentAlias,
        )?);
    }
    for leaf in receipt.leaves() {
        if matches!(
            leaf.artifact_kind(),
            ActiveArtifactKindV1::AspBinary | ActiveArtifactKindV1::Activation
        ) {
            continue;
        }
        leaf_fingerprints.push(verify_materialized_leaf(
            Path::new(leaf.materialized_path()),
            leaf,
            leaf.artifact_kind().canonical_name(),
            MaterializationMatchPolicy::Exact,
        )?);
    }
    verify_active_provider_artifact_closure(activation_path, &receipt)?;
    remember_verified_active_receipt(
        receipt_path,
        &receipt_metadata,
        activation_path,
        asp_paths,
        leaf_fingerprints,
        receipt.clone(),
    )?;
    Ok(receipt)
}

fn verified_active_receipt_cache_hit(
    receipt_path: &Path,
    receipt_metadata: &fs::Metadata,
    activation_path: &Path,
    asp_paths: &[&Path],
) -> Result<Option<ActiveAspArtifactReceiptV1>, String> {
    let Some(cache) = VERIFIED_ACTIVE_ASP_ARTIFACT_RECEIPT_CACHE.get() else {
        return Ok(None);
    };
    let guard = cache
        .lock()
        .map_err(|_| "active ASP artifact receipt cache lock poisoned".to_string())?;
    let Some(entry) = guard.as_ref() else {
        return Ok(None);
    };
    if entry.receipt_path != receipt_path
        || entry.receipt_size_bytes != receipt_metadata.len()
        || entry.receipt_modified_unix_nanos != modified_unix_nanos(receipt_metadata)?
        || entry.receipt_change_time_unix_nanos != change_time_unix_nanos(receipt_metadata)
        || entry.activation_path != activation_path
        || !same_asp_paths(&entry.asp_paths, asp_paths)
    {
        return Ok(None);
    }
    for fingerprint in &entry.leaf_fingerprints {
        if !current_metadata_matches_fingerprint(fingerprint)? {
            return Ok(None);
        }
    }
    Ok(Some(entry.receipt.clone()))
}

fn remember_verified_active_receipt(
    receipt_path: PathBuf,
    receipt_metadata: &fs::Metadata,
    activation_path: &Path,
    asp_paths: &[&Path],
    leaf_fingerprints: Vec<ActiveArtifactMetadataFingerprint>,
    receipt: ActiveAspArtifactReceiptV1,
) -> Result<(), String> {
    let cache = VERIFIED_ACTIVE_ASP_ARTIFACT_RECEIPT_CACHE.get_or_init(|| Mutex::new(None));
    let mut guard = cache
        .lock()
        .map_err(|_| "active ASP artifact receipt cache lock poisoned".to_string())?;
    *guard = Some(VerifiedActiveAspArtifactReceiptCacheEntry {
        receipt_path,
        receipt_size_bytes: receipt_metadata.len(),
        receipt_modified_unix_nanos: modified_unix_nanos(receipt_metadata)?,
        receipt_change_time_unix_nanos: change_time_unix_nanos(receipt_metadata),
        activation_path: activation_path.to_path_buf(),
        asp_paths: asp_paths.iter().map(|path| (*path).to_path_buf()).collect(),
        leaf_fingerprints,
        receipt,
    });
    Ok(())
}

fn same_asp_paths(cached: &[PathBuf], current: &[&Path]) -> bool {
    cached.len() == current.len()
        && cached
            .iter()
            .zip(current.iter())
            .all(|(cached, current)| cached == *current)
}

fn current_metadata_matches_fingerprint(
    fingerprint: &ActiveArtifactMetadataFingerprint,
) -> Result<bool, String> {
    let metadata = fs::metadata(&fingerprint.materialized_path).map_err(|error| {
        format!(
            "failed to inspect {}: {error}",
            fingerprint.materialized_path
        )
    })?;
    Ok(metadata.len() == fingerprint.size_bytes
        && modified_unix_nanos(&metadata)? == fingerprint.modified_unix_nanos
        && change_time_unix_nanos(&metadata) == fingerprint.change_time_unix_nanos)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MaterializationMatchPolicy {
    Exact,
    ContentEquivalentAlias,
}

fn verify_materialized_leaf(
    path: &Path,
    leaf: &ActiveArtifactLeafV1,
    label: &str,
    match_policy: MaterializationMatchPolicy,
) -> Result<ActiveArtifactMetadataFingerprint, String> {
    let canonical = canonical_regular_file(path, label)?;
    let is_receipt_materialization = canonical == Path::new(leaf.materialized_path());
    if !is_receipt_materialization && match_policy == MaterializationMatchPolicy::Exact {
        return Err(format!(
            "{label} target mismatch: actual={} receipt={}",
            canonical.display(),
            leaf.materialized_path()
        ));
    }
    let metadata = fs::metadata(&canonical)
        .map_err(|error| format!("failed to inspect {}: {error}", canonical.display()))?;
    let size = metadata.len();
    if size != leaf.size_bytes() {
        return Err(format!(
            "{label} size mismatch: actual={size} receipt={}",
            leaf.size_bytes()
        ));
    }
    let modified_unix_nanos = modified_unix_nanos(&metadata)?;
    let change_time_unix_nanos = change_time_unix_nanos(&metadata);
    let fingerprint = ActiveArtifactMetadataFingerprint {
        materialized_path: utf8_path(&canonical, label)?,
        size_bytes: size,
        modified_unix_nanos,
        change_time_unix_nanos,
    };
    if is_receipt_materialization
        && modified_unix_nanos == leaf.modified_unix_nanos()
        && change_time_unix_nanos == leaf.change_time_unix_nanos()
    {
        return Ok(fingerprint);
    }
    let bytes = fs::read(&canonical)
        .map_err(|error| format!("failed to read {}: {error}", canonical.display()))?;
    let digest = blake3_content_digest_v1(&bytes);
    if &digest != leaf.artifact_digest() {
        return Err(format!(
            "{label} content identity mismatch: actual={} receipt={}",
            digest.as_str(),
            leaf.artifact_digest().as_str()
        ));
    }
    Ok(fingerprint)
}

fn modified_unix_nanos(metadata: &fs::Metadata) -> Result<u64, String> {
    let nanos = metadata
        .modified()
        .map_err(|error| format!("failed to read artifact modification time: {error}"))?
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("artifact modification time predates UNIX epoch: {error}"))?
        .as_nanos();
    u64::try_from(nanos).map_err(|_| "artifact modification time exceeds u64".to_string())
}

#[cfg(unix)]
fn change_time_unix_nanos(metadata: &fs::Metadata) -> Option<i64> {
    use std::os::unix::fs::MetadataExt;
    metadata
        .ctime()
        .checked_mul(1_000_000_000)?
        .checked_add(metadata.ctime_nsec())
}

#[cfg(not(unix))]
fn change_time_unix_nanos(_metadata: &fs::Metadata) -> Option<i64> {
    None
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

fn utf8_path(path: &Path, label: &str) -> Result<String, String> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| format!("{label} path is not UTF-8: {}", path.display()))
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or_else(|| {
        format!(
            "active artifact receipt path has no parent: {}",
            path.display()
        )
    })?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let temporary = parent.join(format!(
        ".{ACTIVE_ASP_ARTIFACT_RECEIPT_FILE}.{}.{nonce}.tmp",
        process::id()
    ));
    let result = (|| -> Result<(), String> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| format!("failed to create {}: {error}", temporary.display()))?;
        file.write_all(bytes)
            .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;
        file.sync_all()
            .map_err(|error| format!("failed to sync {}: {error}", temporary.display()))?;
        fs::rename(&temporary, path)
            .map_err(|error| format!("failed to publish {}: {error}", path.display()))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
#[path = "../tests/unit/active_artifact_receipt.rs"]
mod active_artifact_receipt_tests;
