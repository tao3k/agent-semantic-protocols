//! PATH-visible `asp` binary installation helpers.

#[path = "protocol_binary_identity.rs"]
mod protocol_binary_identity;
#[path = "protocol_binary_retention.rs"]
mod protocol_binary_retention;

pub(crate) use protocol_binary_retention::prune_runtime_binary_artifacts;

use protocol_binary_identity::is_digest_addressed_protocol_binary;
pub(crate) use protocol_binary_identity::protocol_binary_digest_from_canonical_artifact_path;
pub(crate) use protocol_binary_identity::{
    canonical_protocol_binary_artifact_digest, protocol_binary_artifact_path_digest,
};

use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::time::Duration;

pub(crate) const SEMANTIC_AGENT_PROTOCOL_BIN: &str = "asp";
#[cfg(not(test))]
const DOCTOR_PROCESS_PROBE_TIMEOUT: Duration = Duration::from_secs(2);
#[cfg(test)]
const DOCTOR_PROCESS_PROBE_TIMEOUT: Duration = Duration::from_millis(500);

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
        if !same_protocol_binary_entry(&target, &canonical_target)
            && !managed_path_aliases.contains(&target)
        {
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

pub(crate) fn protocol_binary_switch_required(
    plan: &ProtocolBinaryInstallPlan,
) -> Result<bool, String> {
    if !plan.target.is_file() {
        return Ok(true);
    }
    let source_digest = agent_semantic_content_identity::file_content_digest_v1(&plan.current_exe)
        .map_err(|error| {
            format!(
                "failed to derive candidate ASP binary digest for {}: {error}",
                plan.current_exe.display()
            )
        })?;
    let target_digest = agent_semantic_content_identity::file_content_digest_v1(&plan.target)
        .map_err(|error| {
            format!(
                "failed to derive installed ASP binary digest for {}: {error}",
                plan.target.display()
            )
        })?;
    Ok(source_digest != target_digest)
}

fn install_protocol_binary_alias(
    alias: &Path,
    canonical_target: &Path,
    _artifact_root: &Path,
) -> Result<(), String> {
    if same_protocol_binary_entry(alias, canonical_target) {
        return Ok(());
    }
    let expected = fs::canonicalize(canonical_target).map_err(|error| {
        format!(
            "failed to resolve current Lattice profile {}: {error}",
            canonical_target.display()
        )
    })?;
    if protocol_binary_symlink_chain_loops(alias) {
        return Err(format!(
            "refusing to repair looping protocol binary alias {}",
            alias.display()
        ));
    }
    if fs::read_link(alias)
        .ok()
        .is_some_and(|target| target == canonical_target)
        && fs::canonicalize(alias).is_ok_and(|identity| identity == expected)
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
    let staged = fs::canonicalize(&temp)
        .map_err(|error| format!("failed to resolve staged alias {}: {error}", temp.display()))?;
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
    let installed = fs::canonicalize(alias).map_err(|error| {
        format!(
            "failed to resolve installed alias {}: {error}",
            alias.display()
        )
    })?;
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

pub(crate) fn protocol_binary_contract_fingerprint(path: &Path) -> Option<String> {
    let args = [OsString::from("--contract-fingerprint")];
    let output = bounded_doctor_process(path, &args).ok()?;
    if !output.status.success() {
        return None;
    }
    let fingerprint = String::from_utf8(output.stdout).ok()?;
    let fingerprint = fingerprint.trim();
    (!fingerprint.is_empty()).then(|| fingerprint.to_string())
}

fn bounded_doctor_process(
    executable: &Path,
    args: &[OsString],
) -> Result<std::process::Output, String> {
    if tokio::runtime::Handle::try_current().is_ok() {
        return Err(
            "doctor process probe cannot synchronously block an active Tokio runtime".into(),
        );
    }
    let runtime =
        agent_semantic_client_db::runtime_server_runtime::RuntimeServerClientExecutor::get()?;
    runtime.block_on(
        agent_semantic_runtime::hook_process_runtime::run_hook_process(
            agent_semantic_runtime::hook_process_runtime::HookProcessRequest {
                executable,
                args,
                stdin: &[],
                timeout: DOCTOR_PROCESS_PROBE_TIMEOUT,
                current_dir: None,
                environment: None,
                discard_stdout: false,
            },
        ),
    )
}

#[cfg(unix)]
fn probe_protocol_binary_in_login_shell(shell: &Path) -> ProtocolBinaryShellProbe {
    // Codex's command Hook runner invokes the configured shell with `-lc` and
    // inherits the host environment.  Doctor must exercise that exact surface;
    // a synthetic non-login PATH probe can report a false exit-127 failure.
    let args = [OsString::from("-lc"), OsString::from("command -v asp")];
    let output = match bounded_doctor_process(shell, &args) {
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
    let runtime_root = artifact_root.parent().ok_or_else(|| {
        format!(
            "protocol artifact root has no runtime parent: {}",
            artifact_root.display()
        )
    })?;
    let protocol_home = runtime_root.parent().ok_or_else(|| {
        format!(
            "protocol runtime root has no State Home parent: {}",
            runtime_root.display()
        )
    })?;
    let state_home_alias = protocol_home.join(".bin").join(SEMANTIC_AGENT_PROTOCOL_BIN);
    let mut aliases = if same_protocol_binary_entry(&state_home_alias, primary_target) {
        Vec::new()
    } else {
        vec![state_home_alias]
    };
    for candidate in path_dirs
        .iter()
        .map(|dir| dir.join(SEMANTIC_AGENT_PROTOCOL_BIN))
        .filter(|candidate| {
            !same_protocol_binary_entry(candidate, primary_target) && candidate.is_file()
        })
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
    let artifact_digest =
        agent_semantic_content_identity::file_content_digest_v1(source).map_err(|error| {
            format!(
                "failed to derive BLAKE3 protocol artifact digest for {}: {error}",
                source.display()
            )
        })?;
    let artifact_path = artifact_root
        .join("blake3-256")
        .join(&artifact_digest)
        .join(binary_name);
    if !artifact_path.is_file() {
        let artifact_parent = artifact_path.parent().ok_or_else(|| {
            format!(
                "digest-addressed protocol artifact has no parent: {}",
                artifact_path.display()
            )
        })?;
        fs::create_dir_all(artifact_parent)
            .map_err(|error| format!("failed to create {}: {error}", artifact_parent.display()))?;
        let candidate = temporary_protocol_binary_path(&artifact_path);
        if fs::symlink_metadata(&candidate).is_ok() {
            fs::remove_file(&candidate).map_err(|error| {
                format!("failed to remove stale {}: {error}", candidate.display())
            })?;
        }
        fs::copy(source, &candidate).map_err(|error| {
            format!(
                "failed to stage digest-addressed runtime artifact {}: {error}",
                candidate.display()
            )
        })?;
        let permissions = fs::metadata(source)
            .map_err(|error| format!("failed to inspect {}: {error}", source.display()))?
            .permissions();
        fs::set_permissions(&candidate, permissions)
            .map_err(|error| format!("failed to chmod {}: {error}", candidate.display()))?;
        let candidate_digest = agent_semantic_content_identity::file_content_digest_v1(&candidate)
            .map_err(|error| {
                format!(
                    "failed to verify staged runtime artifact {}: {error}",
                    candidate.display()
                )
            })?;
        if candidate_digest != artifact_digest {
            let _ = fs::remove_file(&candidate);
            return Err(format!(
                "digest-addressed runtime artifact drift: expected={artifact_digest} actual={candidate_digest}"
            ));
        }
        atomic_replace_protocol_entry(&candidate, &artifact_path)?;
    }
    let artifact_identity = fs::canonicalize(&artifact_path).map_err(|error| {
        format!(
            "failed to resolve digest-addressed runtime artifact {}: {error}",
            artifact_path.display()
        )
    })?;
    if protocol_binary_digest_from_canonical_artifact_path(&artifact_identity).as_deref()
        != Some(artifact_digest.as_str())
    {
        return Err(format!(
            "runtime artifact path does not encode installed digest: path={} digest={artifact_digest}",
            artifact_identity.display()
        ));
    }
    let target_is_current = fs::canonicalize(target)
        .ok()
        .is_some_and(|current| current == artifact_identity);
    let status = if target_is_current {
        "already-present"
    } else {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
        let candidate = temporary_protocol_binary_path(target);
        if fs::symlink_metadata(&candidate).is_ok() {
            fs::remove_file(&candidate).map_err(|error| {
                format!("failed to remove stale {}: {error}", candidate.display())
            })?;
        }
        stage_active_protocol_entry(&artifact_identity, &candidate)?;
        let status = if fs::symlink_metadata(target).is_ok() {
            "updated"
        } else {
            "installed"
        };
        atomic_replace_protocol_entry(&candidate, target)?;
        status
    };
    Ok(ProtocolBinaryInstall {
        path: target.to_path_buf(),
        status,
        artifact_digest,
    })
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
        match fs::metadata(entry) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                return Err(format!(
                    "failed to inspect protocol binary entry {}: {error}",
                    entry.display()
                ));
            }
        }
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

fn same_protocol_binary_entry(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (left.parent(), right.parent()) {
        (Some(left_parent), Some(right_parent)) => {
            same_dir(left_parent, right_parent) && left.file_name() == right.file_name()
        }
        _ => false,
    }
}
