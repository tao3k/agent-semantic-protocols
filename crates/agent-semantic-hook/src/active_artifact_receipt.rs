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

const ACTIVE_ASP_ARTIFACT_RECEIPT_FILE: &str = "active-asp-artifact-receipt.v1.json";
const ACTIVE_ASP_ARTIFACT_SET_ID: &str = "asp-runtime";

#[derive(Debug, Clone)]
pub struct ActiveAspArtifactMaterialization {
    pub receipt_path: PathBuf,
    pub receipt: ActiveAspArtifactReceiptV1,
}

#[derive(Debug, Clone)]
pub struct ActiveAspArtifactInput {
    pub logical_path: String,
    pub artifact_kind: ActiveArtifactKindV1,
    pub materialized_path: PathBuf,
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

pub fn materialize_active_asp_artifact_receipt(
    binary_path: &Path,
    binary_digest: &str,
    activation_path: &Path,
    additional_artifacts: &[ActiveAspArtifactInput],
) -> Result<ActiveAspArtifactMaterialization, String> {
    let binary_path = canonical_regular_file(binary_path, "ASP binary")?;
    let activation_path = canonical_regular_file(activation_path, "activation")?;
    let binary_metadata = fs::metadata(&binary_path)
        .map_err(|error| format!("failed to inspect {}: {error}", binary_path.display()))?;
    let activation_bytes = fs::read(&activation_path)
        .map_err(|error| format!("failed to read {}: {error}", activation_path.display()))?;
    let binary_digest = parse_content_digest_v1(binary_digest)
        .map_err(|_| format!("invalid BLAKE3 ASP binary digest: {binary_digest}"))?;
    let mut leaves = vec![
        ActiveArtifactLeafV1 {
            logical_path: "runtime/asp".to_string(),
            materialized_path: utf8_path(&binary_path, "ASP binary")?,
            artifact_kind: ActiveArtifactKindV1::AspBinary,
            artifact_digest: binary_digest,
            size_bytes: binary_metadata.len(),
            modified_unix_nanos: modified_unix_nanos(&binary_metadata)?,
            change_time_unix_nanos: change_time_unix_nanos(&binary_metadata),
        },
        ActiveArtifactLeafV1 {
            logical_path: "state/activation.json".to_string(),
            materialized_path: utf8_path(&activation_path, "activation")?,
            artifact_kind: ActiveArtifactKindV1::Activation,
            artifact_digest: blake3_content_digest_v1(&activation_bytes),
            size_bytes: activation_bytes.len() as u64,
            modified_unix_nanos: modified_unix_nanos(&fs::metadata(&activation_path).map_err(
                |error| format!("failed to inspect {}: {error}", activation_path.display()),
            )?)?,
            change_time_unix_nanos: change_time_unix_nanos(
                &fs::metadata(&activation_path).map_err(|error| {
                    format!("failed to inspect {}: {error}", activation_path.display())
                })?,
            ),
        },
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
        let bytes = fs::read(&materialized_path).map_err(|error| {
            format!(
                "failed to read active artifact {}: {error}",
                materialized_path.display()
            )
        })?;
        let metadata = fs::metadata(&materialized_path).map_err(|error| {
            format!(
                "failed to inspect active artifact {}: {error}",
                materialized_path.display()
            )
        })?;
        leaves.push(ActiveArtifactLeafV1 {
            logical_path: artifact.logical_path.clone(),
            materialized_path: utf8_path(&materialized_path, "active artifact")?,
            artifact_kind: artifact.artifact_kind,
            artifact_digest: blake3_content_digest_v1(&bytes),
            size_bytes: bytes.len() as u64,
            modified_unix_nanos: modified_unix_nanos(&metadata)?,
            change_time_unix_nanos: change_time_unix_nanos(&metadata),
        });
    }
    let receipt = ActiveAspArtifactReceiptV1::build(ACTIVE_ASP_ARTIFACT_SET_ID, leaves)
        .map_err(|error| format!("failed to build active ASP artifact receipt: {error:?}"))?;
    let receipt_path = active_asp_artifact_receipt_path(&activation_path)?;
    let bytes = serde_json::to_vec_pretty(&receipt)
        .map_err(|error| format!("failed to encode active ASP artifact receipt: {error}"))?;
    atomic_write(&receipt_path, &bytes)?;
    Ok(ActiveAspArtifactMaterialization {
        receipt_path,
        receipt,
    })
}

pub fn materialize_active_asp_artifact_receipt_for_current_process(
    activation_path: &Path,
    activation: &crate::HookRuntime,
) -> Result<bool, String> {
    let current_exe = std::env::current_exe()
        .map_err(|error| format!("failed to resolve current ASP binary: {error}"))?;
    let canonical = canonical_regular_file(&current_exe, "ASP binary")?;
    let Some(digest) = digest_addressed_binary_digest(&canonical) else {
        return Ok(false);
    };
    let mut provider_artifacts = activation
        .providers
        .iter()
        .filter_map(|provider| {
            provider
                .provider_command_prefix
                .iter()
                .map(PathBuf::from)
                .find(|path| path.is_absolute() && path.is_file())
                .map(|materialized_path| ActiveAspArtifactInput {
                    logical_path: format!(
                        "providers/{}/{}",
                        provider.language_id, provider.provider_id
                    ),
                    artifact_kind: ActiveArtifactKindV1::ProviderBinary,
                    materialized_path,
                })
        })
        .collect::<Vec<_>>();
    let runtime_config = crate::default_client_config_path(&activation.project_root);
    if runtime_config.is_file() {
        provider_artifacts.push(ActiveAspArtifactInput {
            logical_path: "runtime/hooks/config.toml".to_string(),
            artifact_kind: ActiveArtifactKindV1::RuntimeConfig,
            materialized_path: runtime_config,
        });
    }
    materialize_active_asp_artifact_receipt(
        &canonical,
        digest,
        activation_path,
        &provider_artifacts,
    )?;
    Ok(true)
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

    let mut leaf_fingerprints = Vec::with_capacity(receipt.leaves.len());
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
    for leaf in &receipt.leaves {
        if matches!(
            leaf.artifact_kind,
            ActiveArtifactKindV1::AspBinary | ActiveArtifactKindV1::Activation
        ) {
            continue;
        }
        leaf_fingerprints.push(verify_materialized_leaf(
            Path::new(&leaf.materialized_path),
            leaf,
            leaf.artifact_kind.canonical_name(),
            MaterializationMatchPolicy::Exact,
        )?);
    }
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
    let is_receipt_materialization = canonical == Path::new(&leaf.materialized_path);
    if !is_receipt_materialization && match_policy == MaterializationMatchPolicy::Exact {
        return Err(format!(
            "{label} target mismatch: actual={} receipt={}",
            canonical.display(),
            leaf.materialized_path
        ));
    }
    let metadata = fs::metadata(&canonical)
        .map_err(|error| format!("failed to inspect {}: {error}", canonical.display()))?;
    let size = metadata.len();
    if size != leaf.size_bytes {
        return Err(format!(
            "{label} size mismatch: actual={size} receipt={}",
            leaf.size_bytes
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
        && modified_unix_nanos == leaf.modified_unix_nanos
        && change_time_unix_nanos == leaf.change_time_unix_nanos
    {
        return Ok(fingerprint);
    }
    let bytes = fs::read(&canonical)
        .map_err(|error| format!("failed to read {}: {error}", canonical.display()))?;
    let digest = blake3_content_digest_v1(&bytes);
    if digest != leaf.artifact_digest {
        return Err(format!(
            "{label} content identity mismatch: actual={} receipt={}",
            digest.as_str(),
            leaf.artifact_digest.as_str()
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

fn digest_addressed_binary_digest(path: &Path) -> Option<&str> {
    let digest = path.parent()?.file_name()?.to_str()?;
    let algorithm = path.parent()?.parent()?.file_name()?.to_str()?;
    let artifacts = path.parent()?.parent()?.parent()?.file_name()?.to_str()?;
    (artifacts == ".asp-artifacts"
        && algorithm == "blake3-256"
        && digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
    .then_some(digest)
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
