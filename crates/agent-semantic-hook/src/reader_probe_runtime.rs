//! Bounded syscall-entry probe for otherwise unknown Bash readers.

use super::{ReaderProbeAccess, ReaderProbeObservation};
#[cfg(target_os = "macos")]
use fs2::FileExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(target_os = "macos")]
use std::sync::{OnceLock, RwLock};
use std::time::{Duration, Instant};

// One cold observation owns a single end-to-end deadline covering secure cache
// preparation, dyld launch, syscall entry, termination, and reap. Cache hits do
// not enter this path and retain their sub-millisecond contract.
const PROBE_COLD_TIMEOUT: Duration = Duration::from_millis(100);
#[cfg(target_os = "macos")]
const DYNAMIC_CACHE_RECORD_MAGIC: &[u8; 8] = b"ASPRDR1\0";
#[cfg(target_os = "macos")]
const DYNAMIC_CACHE_SHARDS: u8 = 64;
#[cfg(target_os = "macos")]
const DYNAMIC_CACHE_MAX_RECORDS: usize = 256;
#[cfg(target_os = "macos")]
static MATERIALIZATION_NONCE: AtomicU64 = AtomicU64::new(0);
#[cfg(target_os = "macos")]
static DYNAMIC_CACHE_PROCESS_SHARDS: [std::sync::Mutex<()>; DYNAMIC_CACHE_SHARDS as usize] =
    [const { std::sync::Mutex::new(()) }; DYNAMIC_CACHE_SHARDS as usize];
#[cfg(target_os = "macos")]
static PROCESS_POSITIVE_CACHE: OnceLock<RwLock<std::collections::VecDeque<blake3::Hash>>> =
    OnceLock::new();

/// Observe one parser-normalized command under the bounded Reader probe.
#[derive(Clone, Debug)]
pub struct ReaderProbeRequest {
    pub command_tokens: Vec<String>,
    pub subject: String,
    pub reader_behavior_patterns: Vec<Vec<String>>,
    pub dynamic_cache_root: Option<PathBuf>,
}

pub fn observe(request: &ReaderProbeRequest) -> Option<ReaderProbeObservation> {
    Some(observe_one(
        request.command_tokens.as_slice(),
        request.subject.clone(),
        &request.reader_behavior_patterns,
        request.dynamic_cache_root.as_deref(),
    ))
}

fn observe_one(
    tokens: &[String],
    subject: String,
    reader_behavior_patterns: &[Vec<String>],
    dynamic_cache_root_override: Option<&Path>,
) -> ReaderProbeObservation {
    let started = Instant::now();
    let terminal =
        |access, terminal: &str, backend: &str, probe_process_launched| ReaderProbeObservation {
            subject: subject.clone(),
            access,
            backend: backend.to_owned(),
            terminal: terminal.to_owned(),
            elapsed_micros: u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX),
            probe_process_launched,
            cleanup_verified: terminal != "cleanup-failed",
            cache_hit: false,
            behavior_key: None,
        };

    if static_reader_behavior_matches(tokens, reader_behavior_patterns) {
        return terminal(
            ReaderProbeAccess::Read,
            "reader-behavior-catalog-hit",
            "hook-generation-reader-catalog",
            false,
        );
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = tokens;
        return terminal(
            ReaderProbeAccess::Unknown,
            "unsupported-platform",
            "unavailable",
            false,
        );
    }

    #[cfg(target_os = "macos")]
    {
        let Some(executable) = tokens.first() else {
            return terminal(
                ReaderProbeAccess::Unknown,
                "missing-executable",
                "dyld-open",
                false,
            );
        };
        if started.elapsed() >= PROBE_COLD_TIMEOUT {
            return terminal(
                ReaderProbeAccess::Unknown,
                "probe-timeout",
                "dyld-open",
                false,
            );
        }
        let Ok((executable, executable_metadata)) = resolve_probe_executable(executable) else {
            return terminal(
                ReaderProbeAccess::Unknown,
                "executable-unavailable",
                "dyld-open",
                false,
            );
        };
        let behavior_key =
            dynamic_behavior_key(&executable, &executable_metadata, tokens, &subject).ok();
        // A verified in-process fact needs neither directory preparation nor
        // disk I/O. Resolve the logical State Home key first and defer secure
        // directory validation until the durable catalog is actually read.
        let cache_root = dynamic_cache_root_override
            .map(|root| Ok(root.to_owned()))
            .unwrap_or_else(dynamic_cache_root_path);
        if let Some(key) = behavior_key.as_ref()
            && let Ok(cache_root) = cache_root.as_ref()
            && process_positive_cache_hit(cache_root, key)
        {
            let mut observation = terminal(
                ReaderProbeAccess::Read,
                "reader-behavior-cache-hit",
                "process-memory-reader-catalog",
                false,
            );
            observation.cache_hit = true;
            observation.behavior_key = Some(key.to_hex().to_string());
            return observation;
        }
        let resolved_cache_root = cache_root.and_then(|root| {
            prepare_dynamic_cache_root(&root)?;
            Ok(root)
        });
        if let Some(key) = behavior_key.as_ref()
            && let Ok(cache_root) = resolved_cache_root.as_ref()
            && dynamic_cache_hit(&cache_root, key)
        {
            publish_process_positive_cache(cache_root, key);
            let mut observation = terminal(
                ReaderProbeAccess::Read,
                "reader-behavior-cache-hit",
                "state-home-reader-catalog",
                false,
            );
            observation.cache_hit = true;
            observation.behavior_key = Some(key.to_hex().to_string());
            return observation;
        }
        let cache_root = resolved_cache_root.ok();
        let cache_lock = behavior_key
            .as_ref()
            .zip(cache_root.as_ref())
            .and_then(|(key, root)| acquire_cache_shard(root, key));
        if let Some(key) = behavior_key.as_ref()
            && let Some(root) = cache_root.as_ref()
            && cache_lock.is_some()
            && dynamic_cache_hit(root, key)
        {
            publish_process_positive_cache(root, key);
            let mut observation = terminal(
                ReaderProbeAccess::Read,
                "reader-behavior-cache-hit",
                "state-home-reader-catalog",
                false,
            );
            observation.cache_hit = true;
            observation.behavior_key = Some(key.to_hex().to_string());
            return observation;
        }
        if behavior_key.is_some() && cache_root.is_some() && cache_lock.is_none() {
            let mut observation = terminal(
                ReaderProbeAccess::Unknown,
                "reader-behavior-cache-busy",
                "state-home-reader-catalog",
                false,
            );
            observation.behavior_key = behavior_key.map(|key| key.to_hex().to_string());
            return observation;
        }
        if started.elapsed() >= PROBE_COLD_TIMEOUT {
            return terminal(
                ReaderProbeAccess::Unknown,
                "probe-timeout",
                "dyld-open",
                false,
            );
        }
        let Ok(probe_root) = materialize_probe_root() else {
            return terminal(
                ReaderProbeAccess::Unknown,
                "probe-root-failed",
                "dyld-open",
                false,
            );
        };
        if started.elapsed() >= PROBE_COLD_TIMEOUT {
            return terminal(
                ReaderProbeAccess::Unknown,
                "probe-timeout",
                "dyld-open",
                false,
            );
        }
        let Ok(interposer) = materialize_interposer(&probe_root) else {
            return terminal(
                ReaderProbeAccess::Unknown,
                "interposer-failed",
                "dyld-open",
                false,
            );
        };
        if started.elapsed() >= PROBE_COLD_TIMEOUT {
            return terminal(
                ReaderProbeAccess::Unknown,
                "probe-timeout",
                "dyld-open",
                false,
            );
        }
        let Ok(sentinel) = materialize_profile_sentinel(&probe_root, &subject) else {
            return terminal(
                ReaderProbeAccess::Unknown,
                "profile-sentinel-failed",
                "dyld-open",
                false,
            );
        };
        let sentinel_text = sentinel.to_string_lossy().into_owned();
        let argv = tokens
            .iter()
            .skip(1)
            .map(|token| {
                if token == &subject {
                    sentinel_text.clone()
                } else {
                    token.clone()
                }
            })
            .collect::<Vec<_>>();
        if !argv.iter().any(|token| token == &sentinel_text) {
            return terminal(
                ReaderProbeAccess::Unknown,
                "subject-not-replaced",
                "dyld-open",
                false,
            );
        }
        let Some((read_fd, write_fd)) = create_observation_pipe() else {
            return terminal(
                ReaderProbeAccess::Unknown,
                "pipe-failed",
                "dyld-open",
                false,
            );
        };
        if started.elapsed() >= PROBE_COLD_TIMEOUT {
            unsafe { libc::close(read_fd) };
            unsafe { libc::close(write_fd) };
            return terminal(
                ReaderProbeAccess::Unknown,
                "probe-timeout",
                "dyld-open",
                false,
            );
        }
        let child = spawn_probe(build_probe_command(
            &probe_root,
            &executable,
            &argv,
            &interposer,
            &sentinel_text,
            write_fd,
        ));
        let Ok(mut child) = child else {
            unsafe {
                libc::close(read_fd);
            }
            return terminal(
                ReaderProbeAccess::Unknown,
                "spawn-failed",
                "dyld-open",
                false,
            );
        };
        match wait_for_observation(read_fd, &mut child, started) {
            ProbeOutcome::Observed(flags) => {
                let access = super::classify_open_access_mode(flags);
                let cache_publish_failed = if access == ReaderProbeAccess::Read
                    && let Some(key) = behavior_key.as_ref()
                    && let Some(root) = cache_root.as_ref()
                    && cache_lock.is_some()
                {
                    publish_dynamic_cache_record(root, key).is_err()
                        || !dynamic_cache_hit(root, key)
                } else {
                    false
                };
                let terminal_kind = if cache_publish_failed {
                    "open-entry-observed-cache-publish-failed"
                } else {
                    if access == ReaderProbeAccess::Read
                        && let Some(key) = behavior_key.as_ref()
                        && let Some(root) = cache_root.as_ref()
                    {
                        publish_process_positive_cache(root, key);
                    }
                    "open-entry-observed"
                };
                let mut observation = terminal(access, terminal_kind, "dyld-open", true);
                observation.behavior_key = behavior_key.map(|key| key.to_hex().to_string());
                observation
            }
            ProbeOutcome::Exited(code) => terminal(
                ReaderProbeAccess::Unknown,
                &format!("probe-exited-before-open:{}", code.map_or(-1, |code| code)),
                "dyld-open",
                true,
            ),
            ProbeOutcome::TimedOut => terminal(
                ReaderProbeAccess::Unknown,
                "probe-timeout",
                "dyld-open",
                true,
            ),
            ProbeOutcome::WaitFailed => terminal(
                ReaderProbeAccess::Unknown,
                "probe-wait-failed",
                "dyld-open",
                true,
            ),
            ProbeOutcome::CleanupFailed => terminal(
                ReaderProbeAccess::Unknown,
                "cleanup-failed",
                "dyld-open",
                true,
            ),
        }
    }
}

#[cfg(target_os = "macos")]
fn resolve_probe_executable(executable: &str) -> Result<(PathBuf, std::fs::Metadata), String> {
    use std::os::unix::fs::PermissionsExt as _;

    let path = Path::new(executable);
    let resolved = if path.components().count() > 1 {
        std::fs::canonicalize(path).map_err(|error| error.to_string())?
    } else {
        which::which(executable).map_err(|error| error.to_string())?
    };
    let metadata = std::fs::metadata(&resolved).map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
        return Err(format!(
            "Reader probe executable is not an executable regular file: {}",
            resolved.display()
        ));
    }
    Ok((resolved, metadata))
}

fn static_reader_behavior_matches(tokens: &[String], patterns: &[Vec<String>]) -> bool {
    let Some(executable) = tokens
        .first()
        .and_then(|token| Path::new(token).file_name())
        .and_then(|name| name.to_str())
    else {
        return false;
    };
    patterns.iter().any(|pattern| {
        pattern.first().is_some_and(|name| name == executable)
            && tokens
                .get(1..pattern.len())
                .is_some_and(|actual| actual == &pattern[1..])
    })
}

#[cfg(target_os = "macos")]
fn dynamic_behavior_key(
    executable: &Path,
    metadata: &std::fs::Metadata,
    tokens: &[String],
    subject: &str,
) -> Result<blake3::Hash, String> {
    use std::os::unix::fs::MetadataExt as _;

    let mut hasher = blake3::Hasher::new();
    hasher.update(b"agent.semantic-protocols.reader-behavior-key\0schema=1\0");
    hasher.update(executable.as_os_str().as_encoded_bytes());
    for fact in [
        metadata.dev(),
        metadata.ino(),
        metadata.size(),
        u64::try_from(metadata.mtime()).unwrap_or_default(),
        u64::try_from(metadata.mtime_nsec()).unwrap_or_default(),
    ] {
        hasher.update(&fact.to_le_bytes());
    }
    for token in tokens.iter().skip(1) {
        hasher.update(&[0]);
        if token == subject {
            hasher.update(b"{registered-source}");
        } else {
            hasher.update(token.as_bytes());
        }
    }
    hasher.update(&[0]);
    hasher.update(
        Path::new(subject)
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("source")
            .as_bytes(),
    );
    Ok(hasher.finalize())
}

#[cfg(target_os = "macos")]
fn process_positive_cache_key(root: &Path, key: &blake3::Hash) -> blake3::Hash {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"agent.semantic-protocols.reader-process-cache\0schema=1\0");
    hasher.update(root.as_os_str().as_encoded_bytes());
    hasher.update(&[0]);
    hasher.update(key.as_bytes());
    hasher.finalize()
}

#[cfg(target_os = "macos")]
fn process_positive_cache_hit(root: &Path, key: &blake3::Hash) -> bool {
    let process_key = process_positive_cache_key(root, key);
    PROCESS_POSITIVE_CACHE
        .get_or_init(|| RwLock::new(std::collections::VecDeque::new()))
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .contains(&process_key)
}

#[cfg(target_os = "macos")]
fn publish_process_positive_cache(root: &Path, key: &blake3::Hash) {
    let process_key = process_positive_cache_key(root, key);
    let mut cache = PROCESS_POSITIVE_CACHE
        .get_or_init(|| RwLock::new(std::collections::VecDeque::new()))
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(index) = cache.iter().position(|cached| cached == &process_key) {
        cache.remove(index);
    }
    cache.push_back(process_key);
    while cache.len() > DYNAMIC_CACHE_MAX_RECORDS {
        cache.pop_front();
    }
}

#[cfg(all(target_os = "macos", test))]
fn clear_process_positive_cache() {
    if let Some(cache) = PROCESS_POSITIVE_CACHE.get() {
        cache
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }
}

#[cfg(target_os = "macos")]
fn dynamic_cache_root_path() -> Result<PathBuf, String> {
    let state_home = std::env::var_os("ASP_STATE_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|value| !value.is_empty())
                .map(|home| PathBuf::from(home).join(".agent-semantic-protocols"))
        })
        .ok_or("Reader behavior cache requires ASP_STATE_HOME or HOME")?;
    Ok(state_home
        .join("hooks")
        .join("reader-behavior")
        .join("dynamic-catalog"))
}

#[cfg(target_os = "macos")]
fn prepare_dynamic_cache_root(root: &Path) -> Result<(), String> {
    ensure_secure_directory(&root)?;
    ensure_secure_directory(&root.join("locks"))?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn ensure_secure_directory(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

    match std::fs::symlink_metadata(path) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir_all(path).map_err(|error| error.to_string())?;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
                .map_err(|error| error.to_string())?;
        }
        Err(error) => return Err(error.to_string()),
    }
    let metadata = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !metadata.file_type().is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
    {
        return Err(format!(
            "Reader behavior cache directory is not a private same-UID directory: {}",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn dynamic_cache_record_path(root: &Path, key: &blake3::Hash) -> PathBuf {
    root.join(format!("{}.bin", key.to_hex()))
}

#[cfg(target_os = "macos")]
fn dynamic_cache_hit(root: &Path, key: &blake3::Hash) -> bool {
    use std::io::Read as _;
    use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};

    let path = dynamic_cache_record_path(root, key);
    let Ok(mut file) = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
    else {
        return false;
    };
    let Ok(metadata) = file.metadata() else {
        return false;
    };
    if !metadata.file_type().is_file()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
    {
        return false;
    }
    let mut record = [0_u8; 40];
    file.read_exact(&mut record).is_ok()
        && record[..8] == DYNAMIC_CACHE_RECORD_MAGIC[..]
        && record[8..] == key.as_bytes()[..]
        && file.read(&mut [0_u8; 1]).ok() == Some(0)
}

#[cfg(target_os = "macos")]
struct DynamicCacheShardGuard {
    _process: std::sync::MutexGuard<'static, ()>,
    _file: std::fs::File,
}

#[cfg(target_os = "macos")]
fn acquire_cache_shard(root: &Path, key: &blake3::Hash) -> Option<DynamicCacheShardGuard> {
    use std::os::unix::fs::OpenOptionsExt as _;

    let shard = key.as_bytes()[0] % DYNAMIC_CACHE_SHARDS;
    // One invocation owns the cold observation. Contenders return Unknown
    // immediately and never steal scheduler time from the only process that
    // can publish the reusable Reader fact.
    let process_guard = match DYNAMIC_CACHE_PROCESS_SHARDS[usize::from(shard)].try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
        Err(std::sync::TryLockError::WouldBlock) => return None,
    };
    let lock_path = root.join("locks").join(format!("{shard:02x}.lock"));
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(lock_path)
        .ok()?;
    match file.try_lock_exclusive() {
        Ok(()) => Some(DynamicCacheShardGuard {
            _process: process_guard,
            _file: file,
        }),
        Err(_) => None,
    }
}

#[cfg(target_os = "macos")]
fn publish_dynamic_cache_record(root: &Path, key: &blake3::Hash) -> Result<(), String> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;

    let path = dynamic_cache_record_path(root, key);
    let nonce = MATERIALIZATION_NONCE.fetch_add(1, Ordering::Relaxed);
    let candidate = root.join(format!(
        ".record-{}-{nonce}-{}",
        std::process::id(),
        key.to_hex()
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&candidate)
        .map_err(|error| error.to_string())?;
    file.write_all(DYNAMIC_CACHE_RECORD_MAGIC)
        .and_then(|()| file.write_all(key.as_bytes()))
        .and_then(|()| file.sync_all())
        .map_err(|error| error.to_string())?;
    drop(file);
    std::fs::rename(&candidate, &path).map_err(|error| {
        let _ = std::fs::remove_file(&candidate);
        error.to_string()
    })?;
    prune_dynamic_cache(root);
    Ok(())
}

#[cfg(target_os = "macos")]
fn prune_dynamic_cache(root: &Path) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let mut records = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            (path.extension().and_then(|extension| extension.to_str()) == Some("bin"))
                .then(|| {
                    entry
                        .metadata()
                        .ok()
                        .and_then(|metadata| metadata.modified().ok())
                        .map(|modified| (modified, path))
                })
                .flatten()
        })
        .collect::<Vec<_>>();
    if records.len() <= DYNAMIC_CACHE_MAX_RECORDS {
        return;
    }
    records.sort_by_key(|(modified, _)| *modified);
    let remove_count = records.len() - DYNAMIC_CACHE_MAX_RECORDS;
    for (_, path) in records.into_iter().take(remove_count) {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(target_os = "macos")]
enum ProbeOutcome {
    Observed(i32),
    Exited(Option<i32>),
    TimedOut,
    WaitFailed,
    CleanupFailed,
}

#[cfg(target_os = "macos")]
fn build_probe_command(
    current_dir: &Path,
    executable: &Path,
    argv: &[String],
    interposer: &Path,
    sentinel: &str,
    write_fd: i32,
) -> Command {
    use std::os::fd::FromRawFd;

    let mut command = Command::new(executable);
    command
        .current_dir(current_dir)
        .args(argv)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("DYLD_INSERT_LIBRARIES", interposer)
        .env("ASP_READER_PROBE_TARGET", sentinel)
        .env("ASP_READER_PROBE_FD", "1")
        .stdin(Stdio::null())
        .stdout(unsafe { Stdio::from_raw_fd(write_fd) })
        .stderr(Stdio::null());
    command
}

#[cfg(target_os = "macos")]
fn spawn_probe(mut command: Command) -> std::io::Result<std::process::Child> {
    use std::os::unix::process::CommandExt;
    // Keep `Command` eligible for the platform's `posix_spawn` path. A
    // `pre_exec` closure forces a fork in a multithreaded Hook process and can
    // leave the child stalled before `exec`, which turns bounded concurrent
    // Reader observations into false `probe-timeout` terminals.
    command.process_group(0);
    command.spawn()
}

#[cfg(target_os = "macos")]
fn materialize_probe_root() -> Result<PathBuf, String> {
    let root =
        std::env::temp_dir().join(format!("asp-reader-probe-{}", unsafe { libc::geteuid() }));
    ensure_secure_directory(&root)?;
    Ok(root)
}

#[cfg(target_os = "macos")]
fn materialize_interposer(root: &Path) -> Result<PathBuf, String> {
    use std::io::{Read as _, Write as _};
    use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};

    let bytes = super::reader_probe_interposer_bytes();
    let expected_digest = blake3::hash(bytes);
    let digest = expected_digest.to_hex();
    let path = root.join(format!("interposer-{digest}.dylib"));
    if !path.try_exists().map_err(|error| error.to_string())? {
        let nonce = MATERIALIZATION_NONCE.fetch_add(1, Ordering::Relaxed);
        let candidate = root.join(format!(
            ".interposer-{}-{nonce}-{digest}",
            std::process::id()
        ));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o400)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&candidate)
            .map_err(|error| error.to_string())?;
        file.write_all(bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        drop(file);
        std::fs::set_permissions(&candidate, std::fs::Permissions::from_mode(0o400))
            .map_err(|error| error.to_string())?;
        match std::fs::hard_link(&candidate, &path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                let _ = std::fs::remove_file(&candidate);
                return Err(error.to_string());
            }
        }
        std::fs::remove_file(&candidate).map_err(|error| error.to_string())?;
    }
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&path)
        .map_err(|error| error.to_string())?;
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    let mut actual = Vec::new();
    file.read_to_end(&mut actual)
        .map_err(|error| error.to_string())?;
    if !metadata.file_type().is_file()
        || metadata.uid() != unsafe { libc::geteuid() }
        || blake3::hash(&actual) != expected_digest
    {
        return Err(format!(
            "Reader probe interposer authority mismatch at {}",
            path.display()
        ));
    }
    file.set_permissions(std::fs::Permissions::from_mode(0o400))
        .map_err(|error| error.to_string())?;
    Ok(path)
}

#[cfg(target_os = "macos")]
fn materialize_profile_sentinel(root: &Path, subject: &str) -> Result<PathBuf, String> {
    use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
    let extension = Path::new(subject)
        .extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| !extension.is_empty())
        .unwrap_or("source");
    let path = root.join(format!("sentinel.{extension}"));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o400)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&path)
    {
        Ok(file) => file.sync_all().map_err(|error| error.to_string())?,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.to_string()),
    }
    let metadata = std::fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.len() != 0
    {
        return Err(format!(
            "Reader probe profile sentinel is not an empty regular file: {}",
            path.display()
        ));
    }
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400))
        .map_err(|error| error.to_string())?;
    Ok(path)
}

#[cfg(target_os = "macos")]
fn create_observation_pipe() -> Option<(i32, i32)> {
    let mut fds = [-1; 2];
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return None;
    }
    unsafe {
        libc::fcntl(fds[0], libc::F_SETFL, libc::O_NONBLOCK);
    }
    Some((fds[0], fds[1]))
}

#[cfg(target_os = "macos")]
fn wait_for_observation(
    read_fd: i32,
    child: &mut std::process::Child,
    started: Instant,
) -> ProbeOutcome {
    let mut bytes = [0_u8; std::mem::size_of::<i32>()];
    let mut observed = 0_usize;
    let mut exited = None;
    let mut timed_out = false;
    loop {
        let read = unsafe {
            libc::read(
                read_fd,
                bytes[observed..].as_mut_ptr().cast(),
                bytes.len() - observed,
            )
        };
        if read > 0 {
            let Ok(read) = usize::try_from(read) else {
                unsafe { libc::close(read_fd) };
                return ProbeOutcome::WaitFailed;
            };
            observed += read;
            if observed == bytes.len() {
                break;
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                exited = Some(status.code());
                break;
            }
            Ok(None) => {}
            Err(_) => {
                unsafe { libc::close(read_fd) };
                return ProbeOutcome::WaitFailed;
            }
        }
        if started.elapsed() >= PROBE_COLD_TIMEOUT {
            timed_out = true;
            break;
        }
        let remaining = PROBE_COLD_TIMEOUT.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            timed_out = true;
            break;
        }
        let timeout_millis = remaining.as_millis().clamp(1, i32::MAX as u128) as i32;
        let mut descriptor = libc::pollfd {
            fd: read_fd,
            events: libc::POLLIN | libc::POLLHUP | libc::POLLERR,
            revents: 0,
        };
        // SAFETY: descriptor refers to the owned observation pipe and remains valid
        // until the unified cleanup block closes it below.
        let poll_result = unsafe { libc::poll(&mut descriptor, 1, timeout_millis) };
        if poll_result < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            break;
        }
    }
    unsafe { libc::close(read_fd) };
    let mut cleanup_verified = true;
    if exited.is_none() && child.try_wait().ok().flatten().is_none() {
        let mut process_group_terminated = false;
        if let Ok(pid) = i32::try_from(child.id()) {
            process_group_terminated = unsafe { libc::kill(-pid, libc::SIGKILL) } == 0
                || std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH);
        }
        // `process_group(0)` is the descendant cleanup authority, while the
        // direct kill is a mandatory fallback if the platform did not publish
        // the process group before the deadline. Always reap the direct child.
        if !process_group_terminated {
            let _ = child.kill();
        }
        cleanup_verified &= child.wait().is_ok();
    }
    if let Ok(pid) = i32::try_from(child.id()) {
        cleanup_verified &= unsafe { libc::kill(-pid, 0) } != 0
            && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH);
    }
    if !cleanup_verified {
        return ProbeOutcome::CleanupFailed;
    }
    if observed == bytes.len() {
        ProbeOutcome::Observed(i32::from_ne_bytes(bytes))
    } else if let Some(code) = exited {
        ProbeOutcome::Exited(code)
    } else if timed_out {
        ProbeOutcome::TimedOut
    } else {
        ProbeOutcome::WaitFailed
    }
}

#[cfg(test)]
#[path = "../tests/unit/reader_probe_runtime.rs"]
mod tests;
