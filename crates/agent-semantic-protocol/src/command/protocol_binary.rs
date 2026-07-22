//! PATH-visible `asp` binary installation helpers.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

const SEMANTIC_AGENT_PROTOCOL_BIN: &str = "asp";
const SEMANTIC_AGENT_BIN_DIR_ENV: &str = "SEMANTIC_AGENT_BIN_DIR";
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
    let artifact_digest = protocol_binary_artifact_digest(source).ok_or_else(|| {
        format!(
            "failed to derive BLAKE3 protocol artifact digest for {}",
            source.display()
        )
    })?;
    let artifact = digest_addressed_protocol_binary_path(&path, &artifact_digest)?;
    stage_digest_addressed_protocol_binary(source, &artifact)?;
    let mut status = "already-present";
    for target in targets {
        status = merge_install_status(
            status,
            install_protocol_binary_from_artifact(target, &artifact)?,
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

fn install_protocol_binary_from_artifact(
    target: &Path,
    artifact: &Path,
) -> Result<&'static str, String> {
    if fs::canonicalize(target)
        .ok()
        .zip(fs::canonicalize(artifact).ok())
        .is_some_and(|(target, artifact)| target == artifact)
    {
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
    let temp = temporary_protocol_binary_path(target);
    if temp.exists() {
        fs::remove_file(&temp)
            .map_err(|error| format!("failed to remove stale {}: {error}", temp.display()))?;
    }
    stage_active_protocol_entry(artifact, &temp)?;
    atomic_replace_protocol_entry(&temp, target)?;
    Ok(status)
}

pub(crate) fn protocol_binary_artifact_digest(path: &Path) -> Option<String> {
    let canonical = fs::canonicalize(path).ok()?;
    if let Some(digest) = digest_addressed_protocol_binary_digest(&canonical) {
        return Some(digest);
    }
    let bytes = fs::read(canonical).ok()?;
    Some(
        agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(&bytes)
            .as_str()
            .to_string(),
    )
}

fn digest_addressed_protocol_binary_digest(path: &Path) -> Option<String> {
    let parent = path.parent()?;
    let digest = parent.file_name()?.to_str()?;
    let algorithm = parent.parent()?.file_name()?.to_str()?;
    let artifacts = parent.parent()?.parent()?.file_name()?.to_str()?;
    (artifacts == ".asp-artifacts"
        && algorithm == "blake3-256"
        && digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
    .then(|| digest.to_string())
}

fn digest_addressed_protocol_binary_path(target: &Path, digest: &str) -> Result<PathBuf, String> {
    let parent = target
        .parent()
        .ok_or_else(|| format!("protocol binary target has no parent: {}", target.display()))?;
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("invalid BLAKE3 protocol artifact digest: {digest}"));
    }
    Ok(parent
        .join(".asp-artifacts")
        .join("blake3-256")
        .join(digest)
        .join(SEMANTIC_AGENT_PROTOCOL_BIN))
}

fn stage_digest_addressed_protocol_binary(source: &Path, artifact: &Path) -> Result<(), String> {
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

fn same_dir(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

#[cfg(test)]
#[path = "../../tests/unit/protocol_binary.rs"]
mod artifact_identity_tests;
