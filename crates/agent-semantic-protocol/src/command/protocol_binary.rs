//! PATH-visible `asp` binary installation helpers.

#[path = "protocol_binary_identity.rs"]
mod protocol_binary_identity;

#[cfg(test)]
pub(crate) use protocol_binary_identity::protocol_binary_digest_from_canonical_artifact_path;
pub(crate) use protocol_binary_identity::{
    canonical_protocol_binary_artifact_digest, protocol_binary_artifact_path_digest,
};
use protocol_binary_identity::{
    is_digest_addressed_protocol_binary, protocol_binary_artifact_digest,
};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

pub(super) const SEMANTIC_AGENT_PROTOCOL_BIN: &str = "asp";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeBinaryIdentityV1 {
    name: std::ffi::OsString,
}

impl RuntimeBinaryIdentityV1 {
    pub(crate) fn asp_bootstrap() -> Self {
        Self {
            name: SEMANTIC_AGENT_PROTOCOL_BIN.into(),
        }
    }

    pub(crate) fn from_registered_provider(binary: &str) -> Result<Self, String> {
        let name = Path::new(binary);
        if binary.is_empty()
            || name.components().count() != 1
            || name.file_name().and_then(|value| value.to_str()) != Some(binary)
        {
            return Err(format!(
                "ProviderRegistry binary must be one executable name, got `{binary}`"
            ));
        }
        Ok(Self {
            name: binary.into(),
        })
    }

    fn name(&self) -> &std::ffi::OsStr {
        &self.name
    }
}
static PROTOCOL_BINARY_PUBLISH_SEQUENCE: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

fn next_protocol_binary_publish_sequence() -> u64 {
    PROTOCOL_BINARY_PUBLISH_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}
const SEMANTIC_AGENT_BIN_DIR_ENV: &str = "SEMANTIC_AGENT_BIN_DIR";
#[derive(Debug)]
pub(crate) struct ProtocolBinaryInstall {
    pub(crate) path: PathBuf,
    pub(crate) status: &'static str,
    pub(crate) artifact_digest: String,
    pub(crate) latest: PathBuf,
    pub(crate) stable_entry: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProtocolBinaryShellProbe {
    pub(crate) shell: PathBuf,
    pub(crate) path: Option<PathBuf>,
    pub(crate) status: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProtocolBinaryPathProbe {
    pub(crate) candidate: Option<PathBuf>,
    pub(crate) path: Option<PathBuf>,
    pub(crate) status: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProtocolBinaryInstallPlan {
    current_exe: PathBuf,
    target: PathBuf,
    artifact_root: PathBuf,
    managed_path_aliases: Vec<PathBuf>,
    binary_identity: RuntimeBinaryIdentityV1,
}

/// Process-wide guard for the mutable global binary pointer and its active
/// artifact receipt. Every writer must hold this guard across both commits so
/// concurrent installers cannot publish a receipt for another binary.
pub(crate) struct ProtocolBinaryReconciliationGuard {
    file: std::fs::File,
}

impl ProtocolBinaryReconciliationGuard {
    pub(crate) fn acquire(protocol_home: &Path) -> Result<Self, String> {
        let lock_dir = protocol_home.join("runtime").join("locks");
        fs::create_dir_all(&lock_dir)
            .map_err(|error| format!("failed to create {}: {error}", lock_dir.display()))?;
        let lock_path = lock_dir.join("global-reconciliation.v1.lock");
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)
            .map_err(|error| {
                format!(
                    "failed to open global reconciliation lock {}: {error}",
                    lock_path.display()
                )
            })?;
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;

            let status = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
            if status != 0 {
                return Err(format!(
                    "global ASP reconciliation is already active: lock={}",
                    lock_path.display()
                ));
            }
        }
        Ok(Self { file })
    }
}

impl Drop for ProtocolBinaryReconciliationGuard {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;

            let _ = unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_UN) };
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/protocol_binary_reconciliation_lock.rs"]
mod protocol_binary_reconciliation_lock_tests;

impl ProtocolBinaryInstallPlan {
    pub(crate) fn capture(artifact_root: PathBuf) -> Result<Self, String> {
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
        }
        require_configured_protocol_bin_dir_on_path()?;
        let path_dirs = path_dirs();
        let target =
            resolve_protocol_binary_install_target(explicit_bin_dir.as_deref(), &artifact_root)?;
        let managed_path_aliases =
            managed_protocol_binary_path_aliases(&artifact_root, &target, &path_dirs)?;
        Ok(Self {
            current_exe,
            target,
            artifact_root,
            managed_path_aliases,
            binary_identity: RuntimeBinaryIdentityV1::asp_bootstrap(),
        })
    }

    pub(crate) fn capture_for_target(
        artifact_root: PathBuf,
        target: PathBuf,
    ) -> Result<Self, String> {
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
        // The runtime target owns publication; an explicit install target is only a managed alias.
        let canonical_target = resolve_protocol_binary_install_target(None, &artifact_root)?;
        let mut managed_path_aliases =
            managed_protocol_binary_path_aliases(&artifact_root, &canonical_target, &path_dirs())?;
        if target != canonical_target && !managed_path_aliases.contains(&target) {
            managed_path_aliases.push(target);
        }
        Ok(Self {
            current_exe,
            target: canonical_target,
            artifact_root,
            managed_path_aliases,
            binary_identity: RuntimeBinaryIdentityV1::asp_bootstrap(),
        })
    }
}

pub(crate) fn ensure_protocol_binary_installed(
    plan: &ProtocolBinaryInstallPlan,
) -> Result<ProtocolBinaryInstall, String> {
    for alias in &plan.managed_path_aliases {
        validate_protocol_entry_for_repair(alias, &plan.artifact_root)?;
    }
    let install = install_protocol_binary_target(
        &plan.current_exe,
        &plan.target,
        &plan.artifact_root,
        &plan.binary_identity,
    )?;
    for alias in &plan.managed_path_aliases {
        install_protocol_binary_alias(alias, &plan.target, &plan.artifact_root)?;
    }
    Ok(install)
}

fn install_protocol_binary_alias(
    alias: &Path,
    canonical_target: &Path,
    artifact_root: &Path,
) -> Result<(), String> {
    let expected = resolve_protocol_binary_artifact_entry(canonical_target, artifact_root)?;
    if protocol_binary_symlink_chain_loops(alias) {
        return Err(format!(
            "refusing to repair looping protocol binary alias {}",
            alias.display()
        ));
    }
    if fs::read_link(alias)
        .ok()
        .is_some_and(|target| target == canonical_target)
        && resolve_protocol_binary_artifact_entry(alias, artifact_root)
            .is_ok_and(|identity| identity == expected)
    {
        return Ok(());
    }
    if let Some(parent) = alias.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let temp = temporary_protocol_binary_path(alias);
    if fs::symlink_metadata(&temp).is_ok() {
        fs::remove_file(&temp)
            .map_err(|error| format!("failed to remove stale {}: {error}", temp.display()))?;
    }
    stage_active_protocol_entry(canonical_target, &temp)?;
    let staged = resolve_protocol_binary_artifact_entry(&temp, artifact_root)?;
    if staged != expected {
        let _ = fs::remove_file(&temp);
        return Err(format!(
            "staged protocol binary alias {} resolves to {}, expected {}",
            temp.display(),
            staged.display(),
            expected.display()
        ));
    }
    atomic_replace_protocol_entry(&temp, alias)?;
    let installed = resolve_protocol_binary_artifact_entry(alias, artifact_root)?;
    if installed != expected {
        return Err(format!(
            "protocol binary alias {} resolves to {}, expected {}",
            alias.display(),
            installed.display(),
            expected.display()
        ));
    }
    Ok(())
}

pub(super) fn ensure_runtime_protocol_binary_alias(
    protocol_home: &Path,
    alias: &Path,
) -> Result<(), String> {
    let runtime_root = protocol_home.join("runtime");
    let artifact_root = runtime_root.join("artifacts");
    let canonical_target = runtime_root.join("bin").join(SEMANTIC_AGENT_PROTOCOL_BIN);
    let expected = resolve_protocol_binary_artifact_entry(&canonical_target, &artifact_root)?;

    match fs::symlink_metadata(alias) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "failed to inspect ASP runtime PATH entry {}: {error}",
                alias.display()
            ));
        }
        Ok(metadata) if metadata.file_type().is_symlink() => {
            resolve_protocol_binary_artifact_entry(alias, &artifact_root)?;
        }
        Ok(_) => {
            return Err(format!(
                "refusing to replace unmanaged ASP runtime PATH entry {}",
                alias.display()
            ));
        }
    }

    install_protocol_binary_alias(alias, &canonical_target, &artifact_root)?;
    let installed = resolve_protocol_binary_artifact_entry(alias, &artifact_root)?;
    if installed != expected {
        return Err(format!(
            "ASP runtime PATH entry {} resolves to {}, expected {}",
            alias.display(),
            installed.display(),
            expected.display()
        ));
    }
    Ok(())
}

pub(crate) fn protocol_binary_on_path() -> Option<PathBuf> {
    protocol_binary_path_probe().path
}

pub(crate) fn protocol_binary_path_probe() -> ProtocolBinaryPathProbe {
    for candidate in path_dirs()
        .iter()
        .map(|dir| dir.join(SEMANTIC_AGENT_PROTOCOL_BIN))
    {
        let Ok(link_metadata) = std::fs::symlink_metadata(&candidate) else {
            continue;
        };
        match std::fs::metadata(&candidate) {
            Ok(metadata) if metadata.is_file() => {
                return ProtocolBinaryPathProbe {
                    candidate: Some(candidate.clone()),
                    path: Some(candidate),
                    status: "found",
                };
            }
            Ok(_) => {
                return ProtocolBinaryPathProbe {
                    candidate: Some(candidate),
                    path: None,
                    status: "not-file",
                };
            }
            Err(_) => {
                let status = if link_metadata.file_type().is_symlink()
                    && protocol_binary_symlink_chain_loops(&candidate)
                {
                    "symlink-loop"
                } else if link_metadata.file_type().is_symlink() {
                    "broken-symlink"
                } else {
                    "unreadable"
                };
                return ProtocolBinaryPathProbe {
                    candidate: Some(candidate),
                    path: None,
                    status,
                };
            }
        }
    }
    ProtocolBinaryPathProbe {
        candidate: None,
        path: None,
        status: "missing",
    }
}

fn protocol_binary_symlink_chain_loops(candidate: &Path) -> bool {
    let mut current = candidate.to_path_buf();
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..64 {
        if !seen.insert(current.clone()) {
            return true;
        }
        let Ok(metadata) = std::fs::symlink_metadata(&current) else {
            return false;
        };
        if !metadata.file_type().is_symlink() {
            return false;
        }
        let Ok(target) = std::fs::read_link(&current) else {
            return false;
        };
        current = if target.is_absolute() {
            target
        } else {
            current
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(target)
        };
    }
    true
}

pub(crate) fn protocol_binary_in_codex_hook_shell() -> ProtocolBinaryShellProbe {
    #[cfg(unix)]
    {
        let shell = env::var_os("SHELL")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/bin/sh"));
        probe_protocol_binary_in_login_shell(&shell)
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
fn probe_protocol_binary_in_login_shell(shell: &Path) -> ProtocolBinaryShellProbe {
    let mut command = std::process::Command::new(shell);
    // Codex's command Hook runner invokes the configured shell with `-lc` and
    // inherits the host environment.  Doctor must exercise that exact surface;
    // a synthetic non-login PATH probe can report a false exit-127 failure.
    command.args(["-lc", "command -v asp"]);
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
    explicit_bin_dir: Option<&Path>,
    artifact_root: &Path,
) -> Result<PathBuf, String> {
    if let Some(bin_dir) = explicit_bin_dir {
        return Ok(bin_dir.join(SEMANTIC_AGENT_PROTOCOL_BIN));
    }
    if artifact_root.file_name().and_then(|name| name.to_str()) != Some("artifacts") {
        return Err(format!(
            "protocol artifact root must end in `artifacts`: {}",
            artifact_root.display()
        ));
    }
    let runtime_root = artifact_root.parent().ok_or_else(|| {
        format!(
            "protocol artifact root has no runtime parent: {}",
            artifact_root.display()
        )
    })?;
    Ok(runtime_root.join("bin").join(SEMANTIC_AGENT_PROTOCOL_BIN))
}

fn managed_protocol_binary_path_aliases(
    artifact_root: &Path,
    primary_target: &Path,
    path_dirs: &[PathBuf],
) -> Result<Vec<PathBuf>, String> {
    let mut aliases = Vec::new();
    for candidate in path_dirs
        .iter()
        .map(|dir| dir.join(SEMANTIC_AGENT_PROTOCOL_BIN))
        .filter(|candidate| candidate != primary_target && candidate.is_file())
    {
        let identity = fs::canonicalize(&candidate).map_err(|error| {
            format!(
                "failed to resolve PATH protocol binary identity {}: {error}",
                candidate.display()
            )
        })?;
        if is_digest_addressed_protocol_binary(&identity, artifact_root)?
            && !aliases.contains(&candidate)
        {
            aliases.push(candidate);
        }
    }

    Ok(aliases)
}

pub(crate) fn install_protocol_binary_target(
    source: &Path,
    target: &Path,
    artifact_root: &Path,
    binary_identity: &RuntimeBinaryIdentityV1,
) -> Result<ProtocolBinaryInstall, String> {
    validate_protocol_entry_for_repair(target, artifact_root)?;
    let binary_name = binary_identity.name();
    if target.file_name() != Some(binary_name) {
        return Err(format!(
            "runtime binary target {} does not match declared binary identity `{}`",
            target.display(),
            binary_name.to_string_lossy()
        ));
    }
    let path = target.to_path_buf();
    let artifact_digest = protocol_binary_artifact_digest(source).ok_or_else(|| {
        format!(
            "failed to derive BLAKE3 protocol artifact digest for {}",
            source.display()
        )
    })?;
    let artifact =
        digest_addressed_protocol_binary_path(artifact_root, &artifact_digest, binary_name)?;
    stage_digest_addressed_protocol_binary(source, &artifact)?;
    let latest =
        publish_latest_protocol_binary(artifact_root, &artifact_digest, binary_name, &artifact)?;
    let stable_entry = target.to_path_buf();
    let status = install_protocol_binary_from_artifact(target, &latest, &artifact, artifact_root)?;
    Ok(ProtocolBinaryInstall {
        path,
        status,
        artifact_digest,
        latest,
        stable_entry,
    })
}

fn install_protocol_binary_from_artifact(
    target: &Path,
    latest: &Path,
    artifact: &Path,
    artifact_root: &Path,
) -> Result<&'static str, String> {
    let expected = resolve_protocol_binary_artifact_entry(artifact, artifact_root)?;
    let published = resolve_protocol_binary_artifact_entry(latest, artifact_root)?;
    if published != expected {
        return Err(format!(
            "latest protocol binary {} resolves to {}, expected {}",
            latest.display(),
            published.display(),
            expected.display()
        ));
    }
    if protocol_binary_symlink_chain_loops(target) {
        return Err(format!(
            "refusing to repair looping protocol binary entry {}",
            target.display()
        ));
    }
    let status = if fs::symlink_metadata(target).is_ok() {
        "updated"
    } else {
        "installed"
    };
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let canonical_target = stable_protocol_binary_link_target(target, artifact_root, latest)?;
    if fs::read_link(target)
        .ok()
        .is_some_and(|link| link == canonical_target)
        && resolve_protocol_binary_artifact_entry(target, artifact_root)
            .is_ok_and(|identity| identity == expected)
    {
        return Ok("already-present");
    }
    let temp = temporary_protocol_binary_path(target);
    if fs::symlink_metadata(&temp).is_ok() {
        fs::remove_file(&temp)
            .map_err(|error| format!("failed to remove stale {}: {error}", temp.display()))?;
    }
    stage_active_protocol_entry(&canonical_target, &temp)?;
    let staged = resolve_protocol_binary_artifact_entry(&temp, artifact_root)?;
    if staged != expected {
        let _ = fs::remove_file(&temp);
        return Err(format!(
            "staged stable protocol entry {} resolves to {}, expected {}",
            temp.display(),
            staged.display(),
            expected.display()
        ));
    }
    atomic_replace_protocol_entry(&temp, target)?;
    let installed = resolve_protocol_binary_artifact_entry(target, artifact_root)?;
    if installed != expected {
        return Err(format!(
            "stable protocol entry {} resolves to {}, expected {}",
            target.display(),
            installed.display(),
            expected.display()
        ));
    }
    Ok(status)
}

fn digest_addressed_protocol_binary_path(
    artifact_root: &Path,
    digest: &str,
    binary_name: &std::ffi::OsStr,
) -> Result<PathBuf, String> {
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("invalid BLAKE3 protocol artifact digest: {digest}"));
    }
    Ok(artifact_root
        .join("blake3-256")
        .join(digest)
        .join(binary_name))
}

fn publish_latest_protocol_binary(
    artifact_root: &Path,
    digest: &str,
    binary_name: &std::ffi::OsStr,
    artifact: &Path,
) -> Result<PathBuf, String> {
    let expected = resolve_protocol_binary_artifact_entry(artifact, artifact_root)?;
    let algorithm_root = artifact_root.join("blake3-256");
    let latest_root = algorithm_root.join("latest");
    match fs::symlink_metadata(&latest_root) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => {
            return Err(format!(
                "per-binary latest root must be a real directory: {}",
                latest_root.display()
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(&latest_root)
                .map_err(|error| format!("failed to create {}: {error}", latest_root.display()))?;
        }
        Err(error) => {
            return Err(format!(
                "failed to inspect per-binary latest root {}: {error}",
                latest_root.display()
            ));
        }
    }
    let latest = latest_root.join(binary_name);
    if protocol_binary_symlink_chain_loops(&latest) {
        return Err(format!(
            "refusing to replace looping latest protocol artifact link {}",
            latest.display()
        ));
    }
    if fs::read_link(&latest)
        .ok()
        .is_some_and(|target| target == Path::new("..").join(digest).join(binary_name))
        && resolve_protocol_binary_artifact_entry(&latest, artifact_root)
            .is_ok_and(|identity| identity == expected)
    {
        return Ok(latest);
    }
    let staged = temporary_protocol_binary_path(&latest);
    if fs::symlink_metadata(&staged).is_ok() {
        fs::remove_file(&staged)
            .map_err(|error| format!("failed to remove stale {}: {error}", staged.display()))?;
    }
    let versioned_target = Path::new("..").join(digest).join(binary_name);
    stage_active_protocol_entry(&versioned_target, &staged)?;
    let staged_identity = resolve_protocol_binary_artifact_entry(&staged, artifact_root)?;
    if staged_identity != expected {
        let _ = fs::remove_file(&staged);
        return Err(format!(
            "staged latest protocol artifact {} resolves to {}, expected {}",
            staged.display(),
            staged_identity.display(),
            expected.display()
        ));
    }
    atomic_replace_protocol_entry(&staged, &latest)?;
    let installed = resolve_protocol_binary_artifact_entry(&latest, artifact_root)?;
    if installed != expected {
        return Err(format!(
            "latest protocol artifact {} resolves to {}, expected {}",
            latest.display(),
            installed.display(),
            expected.display()
        ));
    }
    Ok(latest)
}

fn stable_protocol_binary_link_target(
    target: &Path,
    artifact_root: &Path,
    latest_artifact: &Path,
) -> Result<PathBuf, String> {
    let conventional_bin = artifact_root
        .parent()
        .map(|runtime_root| runtime_root.join("bin"));
    if conventional_bin.as_deref() == target.parent() {
        let binary_name = latest_artifact.file_name().ok_or_else(|| {
            format!(
                "latest protocol artifact has no binary name: {}",
                latest_artifact.display()
            )
        })?;
        return Ok(PathBuf::from("../artifacts/blake3-256/latest").join(binary_name));
    }
    Ok(latest_artifact.to_path_buf())
}

fn resolve_protocol_binary_artifact_entry(
    entry: &Path,
    artifact_root: &Path,
) -> Result<PathBuf, String> {
    if protocol_binary_symlink_chain_loops(entry) {
        return Err(format!(
            "protocol binary symlink chain loops at {}",
            entry.display()
        ));
    }
    let identity = fs::canonicalize(entry).map_err(|error| {
        format!(
            "failed to resolve protocol binary entry {}: {error}",
            entry.display()
        )
    })?;
    if !is_digest_addressed_protocol_binary(&identity, artifact_root)? {
        return Err(format!(
            "protocol binary entry {} escapes immutable artifact root {}",
            entry.display(),
            artifact_root.display()
        ));
    }
    Ok(identity)
}

fn validate_protocol_entry_for_repair(entry: &Path, artifact_root: &Path) -> Result<(), String> {
    if protocol_binary_symlink_chain_loops(entry) {
        return Err(format!(
            "protocol binary symlink chain loops at {}",
            entry.display()
        ));
    }
    let Ok(metadata) = fs::symlink_metadata(entry) else {
        return Ok(());
    };
    if metadata.file_type().is_symlink() {
        resolve_protocol_binary_artifact_entry(entry, artifact_root)?;
        return Ok(());
    }
    if metadata.is_file() {
        return Ok(());
    }
    Err(format!(
        "refusing to replace non-file protocol binary entry {}",
        entry.display()
    ))
}

#[cfg(all(test, unix))]
#[path = "../../tests/unit/protocol_binary_latest_publication.rs"]
mod protocol_binary_latest_publication_tests;

fn stage_digest_addressed_protocol_binary(source: &Path, artifact: &Path) -> Result<(), String> {
    if artifact.is_file() {
        return Ok(());
    }
    let parent = artifact
        .parent()
        .ok_or_else(|| format!("protocol artifact has no parent: {}", artifact.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let staged = artifact.with_extension(format!(
        "stage-{}-{}",
        process::id(),
        next_protocol_binary_publish_sequence()
    ));
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
    match fs::rename(&staged, artifact) {
        Ok(()) => Ok(()),
        Err(_) if artifact.is_file() => {
            let _ = fs::remove_file(&staged);
            Ok(())
        }
        Err(error) => Err(format!(
            "failed to publish versioned protocol artifact {}: {error}",
            artifact.display()
        )),
    }
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
    target.with_file_name(format!(
        ".{file_name}.{}.{}.tmp",
        process::id(),
        next_protocol_binary_publish_sequence()
    ))
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

pub(crate) fn require_configured_protocol_bin_dir_on_path() -> Result<(), String> {
    if let Some(bin_dir) = env::var_os(SEMANTIC_AGENT_BIN_DIR_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    {
        require_path_contains_dir(&bin_dir)?;
    }
    Ok(())
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
