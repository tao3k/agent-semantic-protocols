//! PATH-visible `asp` binary installation helpers.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

const SEMANTIC_AGENT_PROTOCOL_BIN: &str = "asp";
const SEMANTIC_AGENT_BIN_DIR_ENV: &str = "SEMANTIC_AGENT_BIN_DIR";
pub(crate) struct ProtocolBinaryInstall {
    pub(crate) path: PathBuf,
    pub(crate) status: &'static str,
    pub(crate) artifact_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProtocolBinaryShellProbe {
    pub(crate) shell: PathBuf,
    pub(crate) path: Option<PathBuf>,
    pub(crate) status: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProtocolBinaryInstallPlan {
    current_exe: PathBuf,
    target: PathBuf,
}

impl ProtocolBinaryInstallPlan {
    pub(crate) fn capture() -> Result<Self, String> {
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

        let explicit_bin_dir = env::var_os(SEMANTIC_AGENT_BIN_DIR_ENV)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        if let Some(bin_dir) = explicit_bin_dir.as_ref() {
            fs::create_dir_all(bin_dir)
                .map_err(|error| format!("failed to create {}: {error}", bin_dir.display()))?;
            require_path_contains_dir(bin_dir)?;
        }
        let target = resolve_protocol_binary_install_target(
            &current_exe,
            explicit_bin_dir.as_deref(),
            &path_dirs(),
        )?;
        Ok(Self {
            current_exe,
            target,
        })
    }
}

pub(crate) fn ensure_protocol_binary_installed(
    plan: &ProtocolBinaryInstallPlan,
) -> Result<ProtocolBinaryInstall, String> {
    install_protocol_binary_target(&plan.current_exe, &plan.target)
}

pub(crate) fn protocol_binary_on_path() -> Option<PathBuf> {
    path_dirs()
        .iter()
        .map(|dir| dir.join(SEMANTIC_AGENT_PROTOCOL_BIN))
        .find(|candidate| candidate.is_file())
}

pub(crate) fn protocol_binary_in_codex_hook_shell() -> ProtocolBinaryShellProbe {
    #[cfg(unix)]
    {
        let shell = env::var_os("SHELL")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/bin/sh"));
        let base_path = codex_hook_parent_path();
        probe_protocol_binary_in_non_login_shell(&shell, &base_path)
    }

    #[cfg(windows)]
    {
        let shell = env::var_os("COMSPEC")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("cmd.exe"));
        let path = protocol_binary_on_path();
        ProtocolBinaryShellProbe {
            shell,
            status: if path.is_some() { "found" } else { "missing" },
            path,
        }
    }
}

#[cfg(unix)]
fn codex_hook_parent_path() -> std::ffi::OsString {
    let mut dirs = Vec::new();
    if cfg!(target_os = "macos")
        && let Some(home) = env::var_os("HOME").filter(|value| !value.is_empty())
    {
        let runtime_bin = PathBuf::from(home)
            .join(".cache")
            .join("codex-runtimes")
            .join("codex-primary-runtime")
            .join("dependencies")
            .join("bin");
        dirs.push(runtime_bin.join("override"));
        dirs.extend([
            PathBuf::from("/usr/bin"),
            PathBuf::from("/bin"),
            PathBuf::from("/usr/sbin"),
            PathBuf::from("/sbin"),
        ]);
        dirs.push(runtime_bin.join("fallback"));
    } else {
        dirs.extend([
            PathBuf::from("/usr/bin"),
            PathBuf::from("/bin"),
            PathBuf::from("/usr/sbin"),
            PathBuf::from("/sbin"),
        ]);
    }
    env::join_paths(dirs).unwrap_or_else(|_| std::ffi::OsString::from("/usr/bin:/bin"))
}

#[cfg(unix)]
fn probe_protocol_binary_in_non_login_shell(
    shell: &Path,
    base_path: &std::ffi::OsStr,
) -> ProtocolBinaryShellProbe {
    let mut command = std::process::Command::new(shell);
    command
        .args(["-c", "command -v asp"])
        .env_clear()
        .env("PATH", base_path)
        .env("SHELL", shell);
    for key in ["HOME", "USER", "LOGNAME", "TMPDIR"] {
        if let Some(value) = env::var_os(key) {
            command.env(key, value);
        }
    }
    let output = match command.output() {
        Ok(output) => output,
        Err(_) => {
            return ProtocolBinaryShellProbe {
                shell: shell.to_path_buf(),
                path: None,
                status: "unavailable",
            };
        }
    };
    let path = output.status.success().then(|| {
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .unwrap_or_default()
            .trim()
            .to_string()
    });
    let path = path
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_file());
    ProtocolBinaryShellProbe {
        shell: shell.to_path_buf(),
        status: if path.is_some() { "found" } else { "missing" },
        path,
    }
}

fn resolve_protocol_binary_install_target(
    current_exe: &Path,
    explicit_bin_dir: Option<&Path>,
    path_dirs: &[PathBuf],
) -> Result<PathBuf, String> {
    if let Some(bin_dir) = explicit_bin_dir {
        return Ok(bin_dir.join(SEMANTIC_AGENT_PROTOCOL_BIN));
    }
    let current_identity = fs::canonicalize(current_exe).map_err(|error| {
        format!(
            "failed to resolve current protocol binary identity {}: {error}",
            current_exe.display()
        )
    })?;
    let target = path_dirs
        .iter()
        .map(|dir| dir.join(SEMANTIC_AGENT_PROTOCOL_BIN))
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| {
            format!(
                "`{SEMANTIC_AGENT_PROTOCOL_BIN}` is not resolvable on PATH; set {SEMANTIC_AGENT_BIN_DIR_ENV} to the single install directory"
            )
        })?;
    let target_identity = fs::canonicalize(&target).map_err(|error| {
        format!(
            "failed to resolve PATH protocol binary identity {}: {error}",
            target.display()
        )
    })?;
    if target_identity != current_identity {
        return Err(format!(
            "refusing to update unrelated PATH binary {}; set {SEMANTIC_AGENT_BIN_DIR_ENV} to the single install directory",
            target.display()
        ));
    }
    Ok(target)
}

pub(crate) fn install_protocol_binary_target(
    source: &Path,
    target: &Path,
) -> Result<ProtocolBinaryInstall, String> {
    let path = target.to_path_buf();
    let artifact_digest = protocol_binary_artifact_digest(source).ok_or_else(|| {
        format!(
            "failed to derive BLAKE3 protocol artifact digest for {}",
            source.display()
        )
    })?;
    let artifact = digest_addressed_protocol_binary_path(&path, &artifact_digest)?;
    stage_digest_addressed_protocol_binary(source, &artifact)?;
    let status = install_protocol_binary_from_artifact(target, &artifact)?;
    Ok(ProtocolBinaryInstall {
        path,
        status,
        artifact_digest,
    })
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
    let install_root = parent.parent().unwrap_or(parent);
    Ok(install_root
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
