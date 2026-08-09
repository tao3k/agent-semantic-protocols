use agent_semantic_content_identity::active_artifact_merkle_v1::{
    ActiveArtifactKindV1, ActiveArtifactLeafV1, ActiveAspArtifactReceiptV1,
};
use agent_semantic_content_identity::exact_selector_merkle::{
    blake3_content_digest_v1, parse_content_digest_v1,
};
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_config::{LanguageId, ProviderId};
use fs2::FileExt;

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

#[path = "active_artifact_provider_input.rs"]
mod provider_input;
pub(crate) use provider_input::installed_provider_artifact_digest;
pub use provider_input::{
    active_provider_artifact_input, active_provider_artifact_input_with_state_home,
};

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
    let expected_provider_paths = active_provider_logical_paths(activation_path)?;
    let materialized_provider_paths = previous_receipt
        .leaves()
        .iter()
        .filter(|leaf| leaf.artifact_kind() == ActiveArtifactKindV1::ProviderBinary)
        .map(|leaf| leaf.logical_path().to_string())
        .collect::<BTreeSet<_>>();
    if !expected_provider_paths.is_subset(&materialized_provider_paths) {
        let runtime = crate::load_activation(activation_path)?;
        let before = previous_receipt.clone();
        materialize_active_asp_artifact_receipt_for_current_process(activation_path, &runtime)?;
        let receipt_bytes = fs::read(&receipt_path).map_err(|error| {
            format!(
                "failed to read rematerialized active ASP artifact receipt {}: {error}",
                receipt_path.display()
            )
        })?;
        let receipt: ActiveAspArtifactReceiptV1 =
            serde_json::from_slice(&receipt_bytes).map_err(|error| {
                format!(
                    "failed to parse rematerialized active ASP artifact receipt {}: {error}",
                    receipt_path.display()
                )
            })?;
        verify_activation_provider_artifact_coverage(activation_path, &receipt)?;
        return Ok(receipt != before);
    }

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
    verify_activation_provider_artifact_coverage(activation_path, &receipt)?;
    if receipt == previous_receipt {
        return Ok(false);
    }
    let bytes = serde_json::to_vec_pretty(&receipt)
        .map_err(|error| format!("failed to encode active ASP artifact receipt: {error}"))?;
    atomic_write_compare_exchange(&receipt_path, &bytes, Some(&receipt_bytes))?;
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
    let binary_path = canonical_regular_file(binary_path, "ASP binary")?;
    let activation_path = canonical_regular_file(activation_path, "activation")?;
    let binary_metadata = fs::metadata(&binary_path)
        .map_err(|error| format!("failed to inspect {}: {error}", binary_path.display()))?;
    let activation_metadata = fs::metadata(&activation_path)
        .map_err(|error| format!("failed to inspect {}: {error}", activation_path.display()))?;
    let activation_bytes = fs::read(&activation_path)
        .map_err(|error| format!("failed to read {}: {error}", activation_path.display()))?;
    let binary_digest = parse_content_digest_v1(binary_digest)
        .map_err(|_| format!("invalid BLAKE3 ASP binary digest: {binary_digest}"))?;
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
            modified_unix_nanos(&activation_metadata)?,
            change_time_unix_nanos(&activation_metadata),
        )?,
    ];
    leaves.extend(
        previous_receipt
            .leaves()
            .iter()
            .filter(|leaf| {
                !matches!(
                    leaf.artifact_kind(),
                    ActiveArtifactKindV1::AspBinary | ActiveArtifactKindV1::Activation
                )
            })
            .cloned(),
    );
    let receipt = ActiveAspArtifactReceiptV1::build(ACTIVE_ASP_ARTIFACT_SET_ID, leaves)
        .map_err(|error| format!("failed to build active ASP artifact receipt: {error:?}"))?;
    verify_activation_provider_artifact_coverage(&activation_path, &receipt)?;
    if receipt == previous_receipt {
        return Ok(ActiveAspArtifactReconciliationV1::Current);
    }
    let bytes = serde_json::to_vec_pretty(&receipt)
        .map_err(|error| format!("failed to encode active ASP artifact receipt: {error}"))?;
    atomic_write_compare_exchange(&receipt_path, &bytes, Some(&receipt_bytes))?;
    Ok(ActiveAspArtifactReconciliationV1::Updated)
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
    let previous_receipt_bytes = if receipt_path.is_file() {
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
        Some((bytes, receipt))
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
    if previous_receipt_bytes
        .as_ref()
        .is_some_and(|(_, previous)| previous == &receipt)
    {
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
    atomic_write_compare_exchange(
        &receipt_path,
        &bytes,
        previous_receipt_bytes
            .as_ref()
            .map(|(bytes, _)| bytes.as_slice()),
    )?;
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
    materialize_active_asp_artifact_receipt_for_current_process_inner(
        activation_path,
        activation,
        None,
    )
}

pub fn materialize_active_asp_artifact_receipt_for_current_process_with_state_home(
    activation_path: &Path,
    activation: &crate::HookRuntime,
    state_home: &Path,
) -> Result<bool, String> {
    materialize_active_asp_artifact_receipt_for_current_process_inner(
        activation_path,
        activation,
        Some(state_home),
    )
}

fn materialize_active_asp_artifact_receipt_for_current_process_inner(
    activation_path: &Path,
    activation: &crate::HookRuntime,
    state_home: Option<&Path>,
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
    let runtime_profiles = match state_home {
        Some(state_home) => crate::runtime_profiles_for_runtime_with_state_home(
            project_root,
            state_home,
            activation,
        ),
        None => Ok(crate::runtime_profiles_for_runtime(
            project_root,
            activation,
        )),
    }?;
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
            match state_home {
                Some(state_home) => active_provider_artifact_input_with_state_home(
                    project_root,
                    state_home,
                    &provider.language_id,
                    &provider.provider_id,
                    PathBuf::from(binary),
                ),
                None => active_provider_artifact_input(
                    project_root,
                    &provider.language_id,
                    &provider.provider_id,
                    PathBuf::from(binary),
                ),
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    let runtime_config = match state_home {
        Some(state_home) => state_home.join("hooks").join("config.toml"),
        None => crate::default_client_config_path(&activation.project_root),
    };
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

fn active_provider_logical_paths(activation_path: &Path) -> Result<BTreeSet<String>, String> {
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
    Ok(activation
        .providers
        .iter()
        .map(|provider| active_provider_logical_path(&provider.language_id, &provider.provider_id))
        .collect())
}

fn verify_activation_provider_artifact_coverage(
    activation_path: &Path,
    receipt: &ActiveAspArtifactReceiptV1,
) -> Result<(), String> {
    let expected = active_provider_logical_paths(activation_path)?;
    let actual = receipt
        .leaves()
        .iter()
        .filter(|leaf| leaf.artifact_kind() == ActiveArtifactKindV1::ProviderBinary)
        .map(|leaf| leaf.logical_path().to_string())
        .collect::<BTreeSet<_>>();
    if !expected.is_subset(&actual) {
        return Err(format!(
            "active ASP provider artifact coverage is incomplete: required={} materialized={}",
            expected.into_iter().collect::<Vec<_>>().join(","),
            actual.into_iter().collect::<Vec<_>>().join(","),
        ));
    }
    Ok(())
}

#[path = "active_artifact_receipt_verification.rs"]
mod verification;
pub use verification::verify_active_asp_artifact_receipt;
use verification::{
    MaterializationMatchPolicy, change_time_unix_nanos, modified_unix_nanos,
    verify_materialized_leaf,
};

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

fn atomic_write_compare_exchange(
    path: &Path,
    bytes: &[u8],
    expected: Option<&[u8]>,
) -> Result<(), String> {
    let parent = path.parent().ok_or_else(|| {
        format!(
            "active artifact receipt path has no parent: {}",
            path.display()
        )
    })?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let lock_path = parent.join(format!(".{ACTIVE_ASP_ARTIFACT_RECEIPT_FILE}.lock"));
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|error| format!("failed to open {}: {error}", lock_path.display()))?;
    lock.lock_exclusive()
        .map_err(|error| format!("failed to lock {}: {error}", lock_path.display()))?;
    let current = match fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("failed to read {}: {error}", path.display())),
    };
    if current.as_deref() != expected {
        return Err(format!(
            "active artifact receipt compare-and-swap conflict: {}",
            path.display()
        ));
    }
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
