//! PATH-visible `asp` binary installation helpers.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const SEMANTIC_AGENT_PROTOCOL_BIN: &str = "asp";
const SEMANTIC_AGENT_BIN_DIR_ENV: &str = "SEMANTIC_AGENT_BIN_DIR";
const ARTIFACT_IDENTITY_SCHEMA: &str = "agent.semantic-protocols.binary-artifact-identity.v1";

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProtocolBinaryArtifactIdentity {
    schema_id: String,
    length: u64,
    modified_nanos: u128,
    artifact_digest: String,
}

pub(crate) struct ProtocolBinaryInstall {
    pub(crate) path: PathBuf,
    pub(crate) paths: Vec<PathBuf>,
    pub(crate) status: &'static str,
    pub(crate) artifact_digest: String,
}

pub(crate) fn ensure_protocol_binary_installed_for_path() -> Result<ProtocolBinaryInstall, String> {
    let current_exe = env::current_exe()
        .map_err(|error| format!("failed to resolve current protocol binary: {error}"))?;
    let current_name = current_exe
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if current_name.trim_end_matches(".exe") != SEMANTIC_AGENT_PROTOCOL_BIN {
        return Err(format!(
            "semantic hook setup must run through `{SEMANTIC_AGENT_PROTOCOL_BIN}` so generated hooks can resolve the same binary on PATH"
        ));
    }

    if let Some(bin_dir) = env::var_os(SEMANTIC_AGENT_BIN_DIR_ENV).filter(|value| !value.is_empty())
    {
        let bin_dir = PathBuf::from(bin_dir);
        fs::create_dir_all(&bin_dir)
            .map_err(|error| format!("failed to create {}: {error}", bin_dir.display()))?;
        require_path_contains_dir(&bin_dir)?;
        let target = bin_dir.join(SEMANTIC_AGENT_PROTOCOL_BIN);
        return install_protocol_binary_targets(&current_exe, &[target]);
    }

    let existing = protocol_binaries_on_path();
    if !existing.is_empty() {
        return install_protocol_binary_targets(&current_exe, &existing);
    }

    let target_dir = first_writable_path_dir().ok_or_else(|| {
        format!(
            "`{SEMANTIC_AGENT_PROTOCOL_BIN}` is not on PATH and no writable PATH directory was found; set {SEMANTIC_AGENT_BIN_DIR_ENV} to a PATH directory and rerun install"
        )
    })?;
    let target = target_dir.join(SEMANTIC_AGENT_PROTOCOL_BIN);
    install_protocol_binary_targets(&current_exe, &[target])
}

pub(crate) fn protocol_binary_on_path() -> Option<PathBuf> {
    protocol_binaries_on_path().into_iter().next()
}

fn protocol_binaries_on_path() -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    path_dirs()
        .into_iter()
        .map(|dir| dir.join(SEMANTIC_AGENT_PROTOCOL_BIN))
        .filter(|candidate| candidate.is_file() && seen.insert(candidate.clone()))
        .collect()
}

pub(crate) fn install_protocol_binary_targets(
    source: &Path,
    targets: &[PathBuf],
) -> Result<ProtocolBinaryInstall, String> {
    let path = targets
        .first()
        .cloned()
        .ok_or_else(|| "protocol binary installation requires at least one target".to_string())?;
    let artifact_digest = sha256_file(source)?;
    write_protocol_binary_artifact_identity(source, &artifact_digest)?;
    let mut status = "already-present";
    for target in targets {
        status = merge_install_status(
            status,
            install_protocol_binary_with_digest(source, target, &artifact_digest)?,
        );
    }
    Ok(ProtocolBinaryInstall {
        path,
        paths: targets.to_vec(),
        status,
        artifact_digest,
    })
}

fn merge_install_status(current: &'static str, next: &'static str) -> &'static str {
    if current == "updated" || next == "updated" {
        "updated"
    } else if current == "installed" || next == "installed" {
        "installed"
    } else {
        "already-present"
    }
}

fn install_protocol_binary_with_digest(
    source: &Path,
    target: &Path,
    digest: &str,
) -> Result<&'static str, String> {
    if same_file(source, target) {
        return Ok("already-present");
    }
    let status = if target.is_file() {
        "updated"
    } else {
        "installed"
    };
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let artifact = versioned_protocol_binary_path(target, digest)?;
    stage_versioned_protocol_binary(source, &artifact)?;
    let temp = temporary_protocol_binary_path(target);
    if temp.exists() {
        fs::remove_file(&temp)
            .map_err(|error| format!("failed to remove stale {}: {error}", temp.display()))?;
    }
    stage_active_protocol_entry(&artifact, &temp)?;
    atomic_replace_protocol_entry(&temp, target)?;
    Ok(status)
}

pub(crate) fn protocol_binary_artifact_digest(path: &Path) -> Option<String> {
    let canonical = fs::canonicalize(path).ok()?;
    if let Some(digest) = versioned_protocol_binary_digest(&canonical) {
        return Some(digest);
    }
    let identity_path = protocol_binary_artifact_identity_path(path)?;
    let identity: ProtocolBinaryArtifactIdentity =
        serde_json::from_slice(&fs::read(identity_path).ok()?).ok()?;
    let (length, modified_nanos) = protocol_binary_file_stamp(path)?;
    (identity.schema_id == ARTIFACT_IDENTITY_SCHEMA
        && identity.length == length
        && identity.modified_nanos == modified_nanos)
        .then_some(identity.artifact_digest)
}

fn versioned_protocol_binary_digest(path: &Path) -> Option<String> {
    let parent = path.parent()?;
    let digest = parent.file_name()?.to_str()?;
    let versions = parent.parent()?.file_name()?.to_str()?;
    (versions == ".asp-versions"
        && digest.len() == 64
        && digest.bytes().all(|byte| byte.is_ascii_hexdigit()))
    .then(|| format!("sha256:{digest}"))
}

fn write_protocol_binary_artifact_identity(path: &Path, digest: &str) -> Result<(), String> {
    let identity_path = protocol_binary_artifact_identity_path(path)
        .ok_or_else(|| format!("protocol binary has no identity path: {}", path.display()))?;
    let (length, modified_nanos) = protocol_binary_file_stamp(path)
        .ok_or_else(|| format!("failed to stat protocol binary: {}", path.display()))?;
    let identity = ProtocolBinaryArtifactIdentity {
        schema_id: ARTIFACT_IDENTITY_SCHEMA.to_string(),
        length,
        modified_nanos,
        artifact_digest: digest.to_string(),
    };
    let bytes = serde_json::to_vec(&identity)
        .map_err(|error| format!("failed to serialize protocol artifact identity: {error}"))?;
    let temporary = identity_path.with_extension(format!("tmp-{}", process::id()));
    fs::write(&temporary, bytes)
        .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;
    match fs::rename(&temporary, &identity_path) {
        Ok(()) => Ok(()),
        Err(_) if identity_path.exists() => {
            fs::remove_file(&identity_path).map_err(|error| {
                format!("failed to replace {}: {error}", identity_path.display())
            })?;
            fs::rename(&temporary, &identity_path).map_err(|error| {
                format!(
                    "failed to activate protocol artifact identity {}: {error}",
                    identity_path.display()
                )
            })
        }
        Err(error) => Err(format!(
            "failed to activate protocol artifact identity {}: {error}",
            identity_path.display()
        )),
    }
}

fn protocol_binary_artifact_identity_path(path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?.to_str()?;
    Some(
        path.parent()?
            .join(format!(".{name}.artifact-identity.json")),
    )
}

fn protocol_binary_file_stamp(path: &Path) -> Option<(u64, u128)> {
    let metadata = path.metadata().ok()?;
    let modified_nanos = metadata
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    Some((metadata.len(), modified_nanos))
}

fn versioned_protocol_binary_path(target: &Path, digest: &str) -> Result<PathBuf, String> {
    let parent = target
        .parent()
        .ok_or_else(|| format!("protocol binary target has no parent: {}", target.display()))?;
    let digest = digest
        .strip_prefix("sha256:")
        .ok_or_else(|| format!("invalid protocol artifact digest: {digest}"))?;
    Ok(parent
        .join(".asp-versions")
        .join(digest)
        .join(SEMANTIC_AGENT_PROTOCOL_BIN))
}

fn stage_versioned_protocol_binary(source: &Path, artifact: &Path) -> Result<(), String> {
    if artifact.is_file() {
        return Ok(());
    }
    let parent = artifact
        .parent()
        .ok_or_else(|| format!("protocol artifact has no parent: {}", artifact.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let staged = artifact.with_extension(format!("stage-{}", process::id()));
    if staged.exists() {
        fs::remove_file(&staged)
            .map_err(|error| format!("failed to remove stale {}: {error}", staged.display()))?;
    }
    fs::copy(source, &staged).map_err(|error| {
        format!(
            "failed to stage {SEMANTIC_AGENT_PROTOCOL_BIN} artifact at {}: {error}",
            staged.display()
        )
    })?;
    let permissions = fs::metadata(source)
        .map_err(|error| format!("failed to inspect {}: {error}", source.display()))?
        .permissions();
    fs::set_permissions(&staged, permissions)
        .map_err(|error| format!("failed to chmod {}: {error}", staged.display()))?;
    fs::rename(&staged, artifact).map_err(|error| {
        format!(
            "failed to publish versioned protocol artifact {}: {error}",
            artifact.display()
        )
    })
}

#[cfg(unix)]
fn stage_active_protocol_entry(artifact: &Path, staged_entry: &Path) -> Result<(), String> {
    std::os::unix::fs::symlink(artifact, staged_entry).map_err(|error| {
        format!(
            "failed to stage protocol binary link {} -> {}: {error}",
            staged_entry.display(),
            artifact.display()
        )
    })
}

#[cfg(not(unix))]
fn stage_active_protocol_entry(artifact: &Path, staged_entry: &Path) -> Result<(), String> {
    fs::copy(artifact, staged_entry)
        .map(|_| ())
        .map_err(|error| {
            format!(
                "failed to stage protocol binary entry {}: {error}",
                staged_entry.display()
            )
        })
}

fn atomic_replace_protocol_entry(staged_entry: &Path, target: &Path) -> Result<(), String> {
    match fs::rename(staged_entry, target) {
        Ok(()) => Ok(()),
        #[cfg(not(unix))]
        Err(_) if target.exists() => {
            fs::remove_file(target)
                .map_err(|error| format!("failed to replace {}: {error}", target.display()))?;
            fs::rename(staged_entry, target).map_err(|error| {
                format!(
                    "failed to install {SEMANTIC_AGENT_PROTOCOL_BIN} to {}: {error}",
                    target.display()
                )
            })
        }
        Err(error) => Err(format!(
            "failed to atomically install {SEMANTIC_AGENT_PROTOCOL_BIN} to {}: {error}",
            target.display()
        )),
    }
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path)
        .map_err(|error| format!("failed to open {} for sha256: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("failed to read {} for sha256: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn temporary_protocol_binary_path(target: &Path) -> PathBuf {
    let file_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(SEMANTIC_AGENT_PROTOCOL_BIN);
    target.with_file_name(format!(".{file_name}.{}.tmp", process::id()))
}

fn require_path_contains_dir(dir: &Path) -> Result<(), String> {
    if path_dirs().iter().any(|path_dir| same_dir(path_dir, dir)) {
        Ok(())
    } else {
        Err(format!(
            "{SEMANTIC_AGENT_BIN_DIR_ENV}={} is not present in PATH; generated hooks use bare `{SEMANTIC_AGENT_PROTOCOL_BIN}`",
            dir.display()
        ))
    }
}

fn first_writable_path_dir() -> Option<PathBuf> {
    path_dirs()
        .into_iter()
        .find(|dir| dir.is_dir() && directory_is_writable(dir))
}

fn directory_is_writable(dir: &Path) -> bool {
    let probe = dir.join(format!(
        ".agent-semantic-protocol-install-check-{}",
        std::process::id()
    ));
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = fs::remove_file(probe);
            true
        }
        Err(_) => false,
    }
}

fn path_dirs() -> Vec<PathBuf> {
    env::var_os("PATH")
        .map(|path| env::split_paths(&path).collect())
        .unwrap_or_default()
}

fn same_file(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn same_dir(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

#[cfg(test)]
#[path = "../../tests/unit/protocol_binary.rs"]
mod artifact_identity_tests;
