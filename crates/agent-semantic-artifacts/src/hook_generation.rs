use std::fs;
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use fs2::FileExt;
use serde::{Deserialize, Serialize};

const HOOK_BINARY_FILE_NAME: &str = "asp-hook";
const COMPILED_GENERATION_FILE_NAME: &str = "compiled-hook-generation.json";
const CONFIG_FILE_NAME: &str = "config.toml";
const REGISTRY_FILE_NAME: &str = "registry.bin";
const RECEIPT_FILE_NAME: &str = "hook-generation-receipt.json";

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredHookGenerationReceipt {
    schema_id: String,
    schema_version: u32,
    hook_binary_file: String,
    generation_file: String,
    config_file: String,
    registry_file: String,
    generation_digest: String,
    hook_binary_digest: String,
    config_digest: String,
    compiled_matcher_digest: String,
    registry_digest: String,
}

pub fn read_current_hook_generation(
    state_home: &Path,
) -> Result<Option<HookGenerationReceipt>, String> {
    let hooks = state_home.join("hooks");
    let current = hooks.join("current");
    let current_metadata = match fs::symlink_metadata(&current) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "inspect current HookGeneration {}: {error}",
                current.display()
            ));
        }
    };
    if !current_metadata.file_type().is_symlink() {
        return Err(format!(
            "current HookGeneration {} is not a symlink",
            current.display()
        ));
    }
    let target = fs::read_link(&current)
        .map_err(|error| format!("read current HookGeneration {}: {error}", current.display()))?;
    let mut components = target.components();
    if components.next() != Some(std::path::Component::Normal("generations".as_ref()))
        || components.next() != Some(std::path::Component::Normal("blake3-256".as_ref()))
        || !matches!(components.next(), Some(std::path::Component::Normal(_)))
        || components.next().is_some()
    {
        return Err(format!(
            "current HookGeneration {} has invalid immutable target {}",
            current.display(),
            target.display()
        ));
    }
    let candidate = hooks.join(&target);
    let candidate_metadata = fs::symlink_metadata(&candidate).map_err(|error| {
        format!(
            "inspect current HookGeneration candidate {}: {error}",
            candidate.display()
        )
    })?;
    if candidate_metadata.file_type().is_symlink() || !candidate_metadata.is_dir() {
        return Err(format!(
            "current HookGeneration candidate {} is not a regular directory",
            candidate.display()
        ));
    }
    let receipt_path = candidate.join(RECEIPT_FILE_NAME);
    let receipt_bytes = read_regular_candidate_file(&receipt_path)?;
    let stored: StoredHookGenerationReceipt =
        serde_json::from_slice(&receipt_bytes).map_err(|error| {
            format!(
                "decode current HookGeneration receipt {}: {error}",
                receipt_path.display()
            )
        })?;
    if stored.schema_id != "agent.semantic-protocols.hook-generation-candidate-receipt"
        || stored.schema_version != 1
    {
        return Err(format!(
            "unsupported current HookGeneration receipt schema {} version {} at {}",
            stored.schema_id,
            stored.schema_version,
            receipt_path.display()
        ));
    }
    if stored.hook_binary_file != HOOK_BINARY_FILE_NAME
        || stored.generation_file != COMPILED_GENERATION_FILE_NAME
        || stored.config_file != CONFIG_FILE_NAME
        || stored.registry_file != REGISTRY_FILE_NAME
    {
        return Err(format!(
            "current HookGeneration receipt {} has invalid artifact bindings",
            receipt_path.display()
        ));
    }
    let hook_binary_path = candidate.join(HOOK_BINARY_FILE_NAME);
    let generation_path = candidate.join(COMPILED_GENERATION_FILE_NAME);
    let hook_binary = read_regular_candidate_file(&hook_binary_path)?;
    let config = read_regular_candidate_file(&candidate.join(CONFIG_FILE_NAME))?;
    let compiled_matcher = read_regular_candidate_file(&generation_path)?;
    let registry = read_regular_candidate_file(&candidate.join(REGISTRY_FILE_NAME))?;
    let expected_hook_binary = content_digest("hook-binary", &hook_binary);
    let expected_config = content_digest("config", &config);
    let expected_matcher = content_digest("compiled-matcher", &compiled_matcher);
    let expected_registry = content_digest("registry", &registry);
    let expected_generation =
        generation_digest(&hook_binary, &config, &compiled_matcher, &registry);
    if stored.hook_binary_digest != expected_hook_binary
        || stored.config_digest != expected_config
        || stored.compiled_matcher_digest != expected_matcher
        || stored.registry_digest != expected_registry
        || stored.generation_digest != expected_generation
    {
        return Err(format!(
            "current HookGeneration receipt {} does not match immutable candidate bytes",
            receipt_path.display()
        ));
    }
    Ok(Some(HookGenerationReceipt {
        schema_id: "agent.semantic-protocols.hook-generation-candidate-receipt",
        schema_version: 1,
        hook_binary_path,
        generation_path,
        generation_digest: stored.generation_digest,
        hook_binary_digest: stored.hook_binary_digest,
        config_digest: stored.config_digest,
        compiled_matcher_digest: stored.compiled_matcher_digest,
        registry_digest: stored.registry_digest,
    }))
}

fn read_regular_candidate_file(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "inspect HookGeneration artifact {}: {error}",
            path.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "HookGeneration artifact {} is not a regular non-symlink file",
            path.display()
        ));
    }
    fs::read(path)
        .map_err(|error| format!("read HookGeneration artifact {}: {error}", path.display()))
}

static PUBLICATION_NONCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookGenerationPublicationReceipt {
    pub schema_id: &'static str,
    pub schema_version: u32,
    pub generation: HookGenerationReceipt,
    pub current_path: PathBuf,
    pub lock_elapsed_micros: u64,
    pub retention_policy: &'static str,
    pub retained_generation_count: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct HookGenerationCandidate<'a> {
    pub hook_binary: &'a Path,
    pub config: &'a [u8],
    pub compiled_matcher: &'a [u8],
    pub registry: &'a [u8],
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookGenerationReceipt {
    pub schema_id: &'static str,
    pub schema_version: u32,
    pub hook_binary_path: PathBuf,
    pub generation_path: PathBuf,
    pub generation_digest: String,
    pub hook_binary_digest: String,
    pub config_digest: String,
    pub compiled_matcher_digest: String,
    pub registry_digest: String,
}

#[derive(Debug)]
pub struct PreparedHookGeneration {
    pub receipt: HookGenerationReceipt,
    staging_path: PathBuf,
    candidate_path: PathBuf,
}

impl Drop for PreparedHookGeneration {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.staging_path);
    }
}

pub fn prepare_hook_generation(
    state_home: &Path,
    candidate: HookGenerationCandidate<'_>,
) -> Result<PreparedHookGeneration, String> {
    let hook_binary_metadata = std::fs::symlink_metadata(candidate.hook_binary)
        .map_err(|error| format!("failed to inspect Hook binary: {error}"))?;
    if !hook_binary_metadata.file_type().is_file()
        || hook_binary_metadata.file_type().is_symlink()
        || hook_binary_metadata.permissions().mode() & 0o111 == 0
    {
        return Err("Hook binary must be a regular executable file".to_owned());
    }
    let hook_binary_bytes = std::fs::read(candidate.hook_binary)
        .map_err(|error| format!("failed to read Hook binary: {error}"))?;
    let artifact_digest = generation_digest(
        &hook_binary_bytes,
        candidate.config,
        candidate.compiled_matcher,
        candidate.registry,
    );
    let hook_binary_digest = content_digest("hook-binary", &hook_binary_bytes);
    let config_digest = content_digest("config", candidate.config);
    let compiled_matcher_digest = content_digest("compiled-matcher", candidate.compiled_matcher);
    let registry_digest = content_digest("registry", candidate.registry);
    let digest_name = artifact_digest.trim_start_matches("blake3-256:");
    let hooks_root = state_home.join("hooks");
    let generations_root = hooks_root.join("generations").join("blake3-256");
    std::fs::create_dir_all(&generations_root)
        .map_err(|error| format!("failed to create Hook generations: {error}"))?;

    let nonce = PUBLICATION_NONCE.fetch_add(1, Ordering::Relaxed);
    let staging = generations_root.join(format!(".candidate-{}-{nonce}", std::process::id()));
    let stored_receipt = StoredHookGenerationReceipt {
        schema_id: "agent.semantic-protocols.hook-generation-candidate-receipt".to_owned(),
        schema_version: 1,
        hook_binary_file: HOOK_BINARY_FILE_NAME.to_owned(),
        generation_file: COMPILED_GENERATION_FILE_NAME.to_owned(),
        config_file: CONFIG_FILE_NAME.to_owned(),
        registry_file: REGISTRY_FILE_NAME.to_owned(),
        generation_digest: artifact_digest.clone(),
        hook_binary_digest: hook_binary_digest.clone(),
        config_digest: config_digest.clone(),
        compiled_matcher_digest: compiled_matcher_digest.clone(),
        registry_digest: registry_digest.clone(),
    };
    materialize_candidate(
        &staging,
        &hook_binary_bytes,
        candidate.compiled_matcher,
        candidate.config,
        candidate.registry,
        &stored_receipt,
    )?;
    let staging_hook_binary = staging.join(HOOK_BINARY_FILE_NAME);
    let staging_generation = staging.join("compiled-hook-generation.json");
    let candidate = generations_root.join(digest_name);
    Ok(PreparedHookGeneration {
        receipt: HookGenerationReceipt {
            schema_id: "agent.semantic-protocols.hook-generation-candidate-receipt",
            schema_version: 1,
            hook_binary_path: staging_hook_binary,
            generation_path: staging_generation,
            generation_digest: artifact_digest,
            hook_binary_digest,
            config_digest,
            compiled_matcher_digest,
            registry_digest,
        },
        staging_path: staging,
        candidate_path: candidate,
    })
}

/// Recover a missing `hooks/current` publication from one already-validated
/// immutable candidate.  This is deliberately narrower than publication:
/// an absent current link is recoverable, while a malformed or tampered link
/// is a terminal state and must not be replaced by a second authority.
///
/// The caller owns the event-bound validation of the candidate (for example,
/// Runtime's activation event and content digest).  This function performs
/// the final HookGeneration byte/executable validation and uses the same
/// prepare/commit transaction as ordinary publication.
pub fn recover_missing_hook_generation(
    state_home: &Path,
    candidate: HookGenerationCandidate<'_>,
) -> Result<HookGenerationPublicationReceipt, String> {
    match read_current_hook_generation(state_home) {
        Ok(Some(_)) => {
            return Err(
                "HookGeneration current is already published; recovery requires an absent current"
                    .to_owned(),
            );
        }
        Ok(None) => {}
        Err(error) => {
            return Err(format!(
                "HookGeneration current is invalid; recovery preserves fail-closed state: {error}"
            ));
        }
    }
    let prepared = prepare_hook_generation(state_home, candidate)?;
    let publication = commit_hook_generation(state_home, &prepared)?;
    read_current_hook_generation(state_home)
        .map_err(|error| format!("validate recovered HookGeneration current: {error}"))?
        .ok_or_else(|| "recovered HookGeneration current disappeared".to_owned())?;
    Ok(publication)
}

pub fn commit_hook_generation(
    state_home: &Path,
    prepared: &PreparedHookGeneration,
) -> Result<HookGenerationPublicationReceipt, String> {
    let hooks_root = state_home.join("hooks");
    let artifact_digest = prepared.receipt.generation_digest.clone();
    let digest_name = artifact_digest.trim_start_matches("blake3-256:");
    let nonce = PUBLICATION_NONCE.fetch_add(1, Ordering::Relaxed);
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(hooks_root.join("publication.lock"))
        .map_err(|error| format!("failed to open Hook publication lock: {error}"))?;
    let lock_started = std::time::Instant::now();
    lock.try_lock_exclusive().map_err(|error| {
        format!(
            "HookGeneration publication is already active: reasonKind=hook-generation-publication-conflict error={error}"
        )
    })?;
    if prepared.candidate_path.exists() {
        validate_candidate_path(&prepared.candidate_path, &prepared.receipt)?;
        std::fs::remove_dir_all(&prepared.staging_path)
            .map_err(|error| format!("failed to discard duplicate Hook candidate: {error}"))?;
    } else {
        std::fs::rename(&prepared.staging_path, &prepared.candidate_path)
            .map_err(|error| format!("failed to commit Hook candidate: {error}"))?;
    }
    sync_directory(
        prepared
            .candidate_path
            .parent()
            .ok_or("Hook candidate has no parent")?,
    )?;
    let next_current = hooks_root.join(format!(".current-{}-{nonce}", std::process::id()));
    symlink(
        PathBuf::from("generations")
            .join("blake3-256")
            .join(digest_name),
        &next_current,
    )
    .map_err(|error| format!("failed to stage Hook current link: {error}"))?;
    let current = hooks_root.join("current");
    if let Ok(metadata) = std::fs::symlink_metadata(&current)
        && !metadata.file_type().is_symlink()
    {
        let _ = std::fs::remove_file(&next_current);
        let _ = FileExt::unlock(&lock);
        return Err(format!(
            "current HookGeneration {} is not a symlink; refusing to replace invalid authority",
            current.display()
        ));
    }
    if let Err(error) = std::fs::rename(&next_current, &current) {
        let _ = std::fs::remove_file(&next_current);
        let _ = FileExt::unlock(&lock);
        return Err(format!(
            "failed to atomically publish Hook current: {error}"
        ));
    }
    sync_directory(&hooks_root)?;
    let retained_generation_count = retain_current_and_previous(&hooks_root)?;
    let lock_elapsed_micros = lock_started.elapsed().as_micros() as u64;
    FileExt::unlock(&lock)
        .map_err(|error| format!("failed to unlock Hook publication: {error}"))?;

    Ok(HookGenerationPublicationReceipt {
        schema_id: "agent.semantic-protocols.hook-generation-publication-receipt",
        schema_version: 1,
        generation: HookGenerationReceipt {
            schema_id: prepared.receipt.schema_id,
            schema_version: prepared.receipt.schema_version,
            hook_binary_path: prepared.candidate_path.join(HOOK_BINARY_FILE_NAME),
            generation_path: prepared
                .candidate_path
                .join("compiled-hook-generation.json"),
            generation_digest: artifact_digest,
            hook_binary_digest: prepared.receipt.hook_binary_digest.clone(),
            config_digest: prepared.receipt.config_digest.clone(),
            compiled_matcher_digest: prepared.receipt.compiled_matcher_digest.clone(),
            registry_digest: prepared.receipt.registry_digest.clone(),
        },
        current_path: current,
        lock_elapsed_micros,
        retention_policy: "current-and-previous",
        retained_generation_count,
    })
}

fn validate_candidate_path(
    candidate: &Path,
    expected: &HookGenerationReceipt,
) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(candidate).map_err(|error| {
        format!(
            "inspect duplicate HookGeneration candidate {}: {error}",
            candidate.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "duplicate HookGeneration candidate {} is not a regular directory",
            candidate.display()
        ));
    }
    let receipt_path = candidate.join(RECEIPT_FILE_NAME);
    let receipt_bytes = read_regular_candidate_file(&receipt_path)?;
    let stored: StoredHookGenerationReceipt = serde_json::from_slice(&receipt_bytes)
        .map_err(|error| format!("decode duplicate HookGeneration receipt: {error}"))?;
    if stored.schema_id != expected.schema_id
        || stored.schema_version != expected.schema_version
        || stored.hook_binary_file != HOOK_BINARY_FILE_NAME
        || stored.generation_file != COMPILED_GENERATION_FILE_NAME
        || stored.config_file != CONFIG_FILE_NAME
        || stored.registry_file != REGISTRY_FILE_NAME
    {
        return Err("duplicate HookGeneration candidate receipt binding mismatch".to_owned());
    }
    let hook_binary = read_regular_candidate_file(&candidate.join(HOOK_BINARY_FILE_NAME))?;
    let config = read_regular_candidate_file(&candidate.join(CONFIG_FILE_NAME))?;
    let compiled_matcher =
        read_regular_candidate_file(&candidate.join(COMPILED_GENERATION_FILE_NAME))?;
    let registry = read_regular_candidate_file(&candidate.join(REGISTRY_FILE_NAME))?;
    let actual = generation_digest(&hook_binary, &config, &compiled_matcher, &registry);
    if stored.generation_digest != expected.generation_digest
        || actual != expected.generation_digest
        || stored.hook_binary_digest != content_digest("hook-binary", &hook_binary)
        || stored.config_digest != content_digest("config", &config)
        || stored.compiled_matcher_digest != content_digest("compiled-matcher", &compiled_matcher)
        || stored.registry_digest != content_digest("registry", &registry)
    {
        return Err(format!(
            "duplicate HookGeneration candidate {} does not match prepared immutable bytes",
            candidate.display()
        ));
    }
    Ok(())
}

fn materialize_candidate(
    directory: &Path,
    hook_binary: &[u8],
    compiled_generation: &[u8],
    config: &[u8],
    registry: &[u8],
    receipt: &StoredHookGenerationReceipt,
) -> Result<(), String> {
    std::fs::create_dir(directory)
        .map_err(|error| format!("failed to create Hook candidate: {error}"))?;
    let hook_binary_path = directory.join(HOOK_BINARY_FILE_NAME);
    std::fs::write(&hook_binary_path, hook_binary)
        .map_err(|error| format!("failed to write Hook binary: {error}"))?;
    let mut permissions = std::fs::metadata(&hook_binary_path)
        .map_err(|error| format!("failed to inspect candidate Hook binary: {error}"))?
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&hook_binary_path, permissions)
        .map_err(|error| format!("failed to mark Hook binary executable: {error}"))?;
    std::fs::write(
        directory.join("compiled-hook-generation.json"),
        compiled_generation,
    )
    .map_err(|error| format!("failed to write compiled HookGeneration: {error}"))?;
    std::fs::File::open(&hook_binary_path)
        .and_then(|file| file.sync_all())
        .map_err(|error| format!("failed to sync Hook binary: {error}"))?;
    std::fs::File::open(directory.join("compiled-hook-generation.json"))
        .and_then(|file| file.sync_all())
        .map_err(|error| format!("failed to sync compiled HookGeneration: {error}"))?;
    let config_path = directory.join(CONFIG_FILE_NAME);
    fs::write(&config_path, config).map_err(|error| {
        format!(
            "write HookGeneration config {}: {error}",
            config_path.display()
        )
    })?;
    let registry_path = directory.join(REGISTRY_FILE_NAME);
    fs::write(&registry_path, registry).map_err(|error| {
        format!(
            "write HookGeneration registry {}: {error}",
            registry_path.display()
        )
    })?;
    let receipt_path = directory.join(RECEIPT_FILE_NAME);
    let receipt_bytes = serde_json::to_vec(receipt)
        .map_err(|error| format!("encode HookGeneration candidate receipt: {error}"))?;
    fs::write(&receipt_path, receipt_bytes).map_err(|error| {
        format!(
            "write HookGeneration candidate receipt {}: {error}",
            receipt_path.display()
        )
    })?;
    for path in [&config_path, &registry_path, &receipt_path] {
        fs::File::open(path)
            .and_then(|file| file.sync_all())
            .map_err(|error| format!("sync HookGeneration artifact {}: {error}", path.display()))?;
    }
    sync_directory(directory)?;
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), String> {
    std::fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("failed to sync Hook publication directory: {error}"))
}

fn generation_digest(
    hook_binary: &[u8],
    config: &[u8],
    compiled_matcher: &[u8],
    registry: &[u8],
) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"agent.semantic-protocols.hook-generation\0");
    hasher.update(&(hook_binary.len() as u64).to_le_bytes());
    hasher.update(hook_binary);
    for bytes in [config, compiled_matcher, registry] {
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    format!("blake3-256:{}", hasher.finalize().to_hex())
}

fn content_digest(domain: &str, bytes: &[u8]) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"agent.semantic-protocols.hook-generation-content\0");
    hasher.update(domain.as_bytes());
    hasher.update(b"\0");
    hasher.update(bytes);
    format!("blake3-256:{}", hasher.finalize().to_hex())
}

fn retain_current_and_previous(hooks_root: &Path) -> Result<usize, String> {
    let generations_root = hooks_root.join("generations").join("blake3-256");
    let current = std::fs::read_link(hooks_root.join("current"))
        .map_err(|error| format!("failed to read Hook current link: {error}"))?;
    let current = current
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Hook current link has no candidate digest")?;
    let mut candidates = std::fs::read_dir(&generations_root)
        .map_err(|error| format!("failed to inspect Hook generations: {error}"))?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
        .collect::<Vec<_>>();
    candidates.sort_by_key(|entry| {
        entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
    });
    candidates.reverse();
    let mut retained = 0usize;
    let mut retained_previous = false;
    for entry in candidates {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == current {
            retained += 1;
        } else if !retained_previous {
            retained += 1;
            retained_previous = true;
        } else {
            std::fs::remove_dir_all(entry.path())
                .map_err(|error| format!("failed to retire Hook candidate: {error}"))?;
        }
    }
    Ok(retained)
}
