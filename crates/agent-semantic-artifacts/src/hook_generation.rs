use std::fs;
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use fs2::FileExt;
use serde::{Deserialize, Serialize};

const EVALUATOR_FILE_NAME: &str = "asp-hook-evaluator";
const COMPILED_GENERATION_FILE_NAME: &str = "compiled-hook-generation.json";
const CONFIG_FILE_NAME: &str = "config.toml";
const REGISTRY_FILE_NAME: &str = "registry.bin";
const RECEIPT_FILE_NAME: &str = "hook-generation-receipt.json";

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredHookGenerationReceipt {
    schema_id: String,
    schema_version: u32,
    evaluator_file: String,
    generation_file: String,
    config_file: String,
    registry_file: String,
    generation_digest: String,
    evaluator_digest: String,
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
    let target = fs::read_link(&current).map_err(|error| {
        format!(
            "read current HookGeneration {}: {error}",
            current.display()
        )
    })?;
    let mut components = target.components();
    if components.next() != Some(std::path::Component::Normal("candidates".as_ref()))
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
    let stored: StoredHookGenerationReceipt = serde_json::from_slice(&receipt_bytes).map_err(|error| {
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
    if stored.evaluator_file != EVALUATOR_FILE_NAME
        || stored.generation_file != COMPILED_GENERATION_FILE_NAME
        || stored.config_file != CONFIG_FILE_NAME
        || stored.registry_file != REGISTRY_FILE_NAME
    {
        return Err(format!(
            "current HookGeneration receipt {} has invalid artifact bindings",
            receipt_path.display()
        ));
    }
    let evaluator_path = candidate.join(EVALUATOR_FILE_NAME);
    let generation_path = candidate.join(COMPILED_GENERATION_FILE_NAME);
    let evaluator = read_regular_candidate_file(&evaluator_path)?;
    let config = read_regular_candidate_file(&candidate.join(CONFIG_FILE_NAME))?;
    let compiled_matcher = read_regular_candidate_file(&generation_path)?;
    let registry = read_regular_candidate_file(&candidate.join(REGISTRY_FILE_NAME))?;
    let expected_evaluator = content_digest("evaluator", &evaluator);
    let expected_config = content_digest("config", &config);
    let expected_matcher = content_digest("compiled-matcher", &compiled_matcher);
    let expected_registry = content_digest("registry", &registry);
    let expected_generation = generation_digest(&evaluator, &config, &compiled_matcher, &registry);
    if stored.evaluator_digest != expected_evaluator
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
        evaluator_path,
        generation_path,
        generation_digest: stored.generation_digest,
        evaluator_digest: stored.evaluator_digest,
        config_digest: stored.config_digest,
        compiled_matcher_digest: stored.compiled_matcher_digest,
        registry_digest: stored.registry_digest,
    }))
}

fn read_regular_candidate_file(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        format!("inspect HookGeneration artifact {}: {error}", path.display())
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
    pub evaluator_binary: &'a Path,
    pub config: &'a [u8],
    pub compiled_matcher: &'a [u8],
    pub registry: &'a [u8],
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookGenerationReceipt {
    pub schema_id: &'static str,
    pub schema_version: u32,
    pub evaluator_path: PathBuf,
    pub generation_path: PathBuf,
    pub generation_digest: String,
    pub evaluator_digest: String,
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
    let evaluator_metadata = std::fs::symlink_metadata(candidate.evaluator_binary)
        .map_err(|error| format!("failed to inspect Hook evaluator: {error}"))?;
    if !evaluator_metadata.file_type().is_file()
        || evaluator_metadata.file_type().is_symlink()
        || evaluator_metadata.permissions().mode() & 0o111 == 0
    {
        return Err("Hook evaluator must be a regular executable file".to_owned());
    }
    let evaluator_bytes = std::fs::read(candidate.evaluator_binary)
        .map_err(|error| format!("failed to read Hook evaluator: {error}"))?;
    let artifact_digest = generation_digest(
        &evaluator_bytes,
        candidate.config,
        candidate.compiled_matcher,
        candidate.registry,
    );
    let evaluator_digest = content_digest("evaluator", &evaluator_bytes);
    let config_digest = content_digest("config", candidate.config);
    let compiled_matcher_digest = content_digest("compiled-matcher", candidate.compiled_matcher);
    let registry_digest = content_digest("registry", candidate.registry);
    let digest_name = artifact_digest.trim_start_matches("blake3-256:");
    let hooks_root = state_home.join("hooks");
    let candidates_root = hooks_root.join("candidates");
    let bin_root = hooks_root.join("bin");
    std::fs::create_dir_all(&candidates_root)
        .map_err(|error| format!("failed to create Hook candidates: {error}"))?;
    std::fs::create_dir_all(&bin_root)
        .map_err(|error| format!("failed to create Hook bin: {error}"))?;

    let nonce = PUBLICATION_NONCE.fetch_add(1, Ordering::Relaxed);
    let staging = candidates_root.join(format!(".candidate-{}-{nonce}", std::process::id()));
    let stored_receipt = StoredHookGenerationReceipt {
        schema_id: "agent.semantic-protocols.hook-generation-candidate-receipt".to_owned(),
        schema_version: 1,
        evaluator_file: EVALUATOR_FILE_NAME.to_owned(),
        generation_file: COMPILED_GENERATION_FILE_NAME.to_owned(),
        config_file: CONFIG_FILE_NAME.to_owned(),
        registry_file: REGISTRY_FILE_NAME.to_owned(),
        generation_digest: artifact_digest.clone(),
        evaluator_digest: evaluator_digest.clone(),
        config_digest: config_digest.clone(),
        compiled_matcher_digest: compiled_matcher_digest.clone(),
        registry_digest: registry_digest.clone(),
    };
    materialize_candidate(
        &staging,
        &evaluator_bytes,
        candidate.compiled_matcher,
        candidate.config,
        candidate.registry,
        &stored_receipt,
    )?;
    let staging_evaluator = staging.join("asp-hook-evaluator");
    let staging_generation = staging.join("compiled-hook-generation.json");
    let candidate = candidates_root.join(digest_name);
    Ok(PreparedHookGeneration {
        receipt: HookGenerationReceipt {
            schema_id: "agent.semantic-protocols.hook-generation-candidate-receipt",
            schema_version: 1,
            evaluator_path: staging_evaluator,
            generation_path: staging_generation,
            generation_digest: artifact_digest,
            evaluator_digest,
            config_digest,
            compiled_matcher_digest,
            registry_digest,
        },
        staging_path: staging,
        candidate_path: candidate,
    })
}

pub fn commit_hook_generation(
    state_home: &Path,
    prepared: &PreparedHookGeneration,
) -> Result<HookGenerationPublicationReceipt, String> {
    let hooks_root = state_home.join("hooks");
    let bin_root = hooks_root.join("bin");
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
    lock.lock_exclusive()
        .map_err(|error| format!("failed to lock Hook publication: {error}"))?;
    if prepared.candidate_path.exists() {
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
    let stable_evaluator = bin_root.join("asp-hook-evaluator");
    ensure_stable_evaluator_link(&stable_evaluator)?;
    let next_current = hooks_root.join(format!(".current-{}-{nonce}", std::process::id()));
    symlink(PathBuf::from("candidates").join(digest_name), &next_current)
        .map_err(|error| format!("failed to stage Hook current link: {error}"))?;
    let current = hooks_root.join("current");
    if let Err(error) = std::fs::rename(&next_current, &current) {
        let _ = std::fs::remove_file(&next_current);
        let _ = FileExt::unlock(&lock);
        return Err(format!("failed to atomically publish Hook current: {error}"));
    }
    sync_directory(&hooks_root)?;
    let retained_generation_count = retain_current_and_previous(&hooks_root)?;
    let lock_elapsed_micros = lock_started.elapsed().as_micros() as u64;
    FileExt::unlock(&lock).map_err(|error| format!("failed to unlock Hook publication: {error}"))?;

    Ok(HookGenerationPublicationReceipt {
        schema_id: "agent.semantic-protocols.hook-generation-publication-receipt",
        schema_version: 1,
        generation: HookGenerationReceipt {
            schema_id: prepared.receipt.schema_id,
            schema_version: prepared.receipt.schema_version,
            evaluator_path: prepared.candidate_path.join("asp-hook-evaluator"),
            generation_path: prepared
                .candidate_path
                .join("compiled-hook-generation.json"),
            generation_digest: artifact_digest,
            evaluator_digest: prepared.receipt.evaluator_digest.clone(),
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

fn materialize_candidate(
    directory: &Path,
    evaluator: &[u8],
    compiled_generation: &[u8],
    config: &[u8],
    registry: &[u8],
    receipt: &StoredHookGenerationReceipt,
) -> Result<(), String> {
    std::fs::create_dir(directory)
        .map_err(|error| format!("failed to create Hook candidate: {error}"))?;
    let evaluator_path = directory.join("asp-hook-evaluator");
    std::fs::write(&evaluator_path, evaluator)
        .map_err(|error| format!("failed to write Hook evaluator: {error}"))?;
    let mut permissions = std::fs::metadata(&evaluator_path)
        .map_err(|error| format!("failed to inspect candidate evaluator: {error}"))?
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&evaluator_path, permissions)
        .map_err(|error| format!("failed to mark Hook evaluator executable: {error}"))?;
    std::fs::write(
        directory.join("compiled-hook-generation.json"),
        compiled_generation,
    )
    .map_err(|error| format!("failed to write compiled HookGeneration: {error}"))?;
    std::fs::File::open(&evaluator_path)
        .and_then(|file| file.sync_all())
        .map_err(|error| format!("failed to sync Hook evaluator: {error}"))?;
    std::fs::File::open(directory.join("compiled-hook-generation.json"))
        .and_then(|file| file.sync_all())
        .map_err(|error| format!("failed to sync compiled HookGeneration: {error}"))?;
    let config_path = directory.join(CONFIG_FILE_NAME);
    fs::write(&config_path, config).map_err(|error| {
        format!("write HookGeneration config {}: {error}", config_path.display())
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

fn ensure_stable_evaluator_link(path: &Path) -> Result<(), String> {
    let expected = Path::new("../current/asp-hook-evaluator");
    match std::fs::read_link(path) {
        Ok(target) if target == expected => Ok(()),
        Ok(_) => Err("stable Hook evaluator link targets another authority".to_owned()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => symlink(expected, path)
            .map_err(|error| format!("failed to publish stable Hook evaluator link: {error}")),
        Err(error) => Err(format!("failed to inspect stable Hook evaluator link: {error}")),
    }
}

fn generation_digest(
    evaluator: &[u8],
    config: &[u8],
    compiled_matcher: &[u8],
    registry: &[u8],
) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"agent.semantic-protocols.hook-generation\0");
    hasher.update(&(evaluator.len() as u64).to_le_bytes());
    hasher.update(evaluator);
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
    let candidates_root = hooks_root.join("candidates");
    let current = std::fs::read_link(hooks_root.join("current"))
        .map_err(|error| format!("failed to read Hook current link: {error}"))?;
    let current = current
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Hook current link has no candidate digest")?;
    let mut candidates = std::fs::read_dir(&candidates_root)
        .map_err(|error| format!("failed to inspect Hook candidates: {error}"))?
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
