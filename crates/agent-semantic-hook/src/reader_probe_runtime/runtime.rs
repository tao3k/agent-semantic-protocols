// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Bounded permission-differential probe for otherwise unknown Bash readers.

use crate::ReaderProbeAccess;
use crate::ReaderProbeObservation;
#[cfg(target_os = "macos")]
use fs2::FileExt as _;
use std::path::Path;
use std::path::PathBuf;
#[cfg(target_os = "macos")]
use std::sync::Arc;
#[cfg(target_os = "macos")]
use std::sync::OnceLock;
#[cfg(target_os = "macos")]
use std::sync::atomic::AtomicU64;
#[cfg(target_os = "macos")]
use std::sync::atomic::Ordering;
#[cfg(target_os = "macos")]
use std::time::Duration;
use std::time::Instant;

#[cfg(target_os = "macos")]
use super::filesystem::ensure_secure_directory;
#[cfg(target_os = "macos")]
use super::process::ProbeOutcome;
#[cfg(target_os = "macos")]
use super::process::exit_signature;
#[cfg(target_os = "macos")]
use super::process::materialize_probe_root;
#[cfg(target_os = "macos")]
use super::process::materialize_profile_sentinels;
#[cfg(target_os = "macos")]
use super::process::replace_subject_operand;
#[cfg(target_os = "macos")]
use super::process::run_permission_candidate;

// One cold observation owns a single end-to-end deadline covering secure cache
// preparation, candidate launch, permission observation, termination, and reap. Cache hits do
// not enter this path and retain their sub-millisecond contract.
// Interpreter-backed command façades (including shell and Python launchers)
// require one extra process boundary before their actual Reader behavior can
// be observed. Keep a strict bounded cold path while allowing that supported
// Host shape to complete; warm catalog hits never enter this budget.
#[cfg(target_os = "macos")]
const PROBE_COLD_TIMEOUT: Duration = Duration::from_secs(1);
#[cfg(target_os = "macos")]
const CACHE_WAIT_PARK: Duration = Duration::from_micros(250);
#[cfg(target_os = "macos")]
const DYNAMIC_CACHE_RECORD_MAGIC: &[u8; 8] = b"ASPRDR1\0";
#[cfg(target_os = "macos")]
const DYNAMIC_CACHE_SHARDS: u8 = 64;
#[cfg(target_os = "macos")]
const DYNAMIC_CACHE_MAX_RECORDS: usize = 256;
#[cfg(target_os = "macos")]
static MATERIALIZATION_NONCE: AtomicU64 = AtomicU64::new(0);
// A cold probe starts arbitrary caller-selected software. It is therefore a
// bounded observation resource, never a queue: a busy slot yields Unknown and
// lets the Hook allow rather than accumulating processes or delaying PreTool.
#[cfg(target_os = "macos")]
static COLD_PROBE_IN_FLIGHT: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
#[cfg(target_os = "macos")]
static DYNAMIC_CACHE_PROCESS_SHARDS: [std::sync::Mutex<()>; DYNAMIC_CACHE_SHARDS as usize] =
    [const { std::sync::Mutex::new(()) }; DYNAMIC_CACHE_SHARDS as usize];
#[cfg(target_os = "macos")]
static PROCESS_POSITIVE_CACHE: OnceLock<arc_swap::ArcSwap<Vec<(blake3::Hash, blake3::Hash)>>> =
    OnceLock::new();

/// Observe one parser-normalized command under the bounded Reader probe.
#[derive(Clone, Debug)]
pub struct ReaderProbeRequest {
    pub command_tokens: Vec<String>,
    pub subject: String,
    pub wrapped_command: bool,
    pub reader_behavior_patterns: Vec<Vec<String>>,
    pub dynamic_cache_root: Option<PathBuf>,
}

pub fn observe(request: &ReaderProbeRequest) -> Option<ReaderProbeObservation> {
    Some(observe_one_with_wrapped(
        request.command_tokens.as_slice(),
        request.subject.clone(),
        request.wrapped_command,
        &request.reader_behavior_patterns,
        request.dynamic_cache_root.as_deref(),
    ))
}

#[cfg(test)]
fn observe_one(
    tokens: &[String],
    subject: String,
    reader_behavior_patterns: &[Vec<String>],
    dynamic_cache_root_override: Option<&Path>,
) -> ReaderProbeObservation {
    observe_one_with_wrapped(
        tokens,
        subject,
        false,
        reader_behavior_patterns,
        dynamic_cache_root_override,
    )
}

fn observe_one_with_wrapped(
    tokens: &[String],
    subject: String,
    wrapped_command: bool,
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

    if static_reader_behavior_matches(tokens, &subject, reader_behavior_patterns, wrapped_command) {
        return terminal(
            ReaderProbeAccess::Read,
            "reader-behavior-catalog-hit",
            "hook-policy-bundle-reader-catalog",
            false,
        );
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (tokens, dynamic_cache_root_override);
        return terminal(
            ReaderProbeAccess::Unknown,
            "unsupported-platform",
            "unavailable",
            false,
        );
    }

    #[cfg(target_os = "macos")]
    {
        let Some(_) = tokens.first() else {
            return terminal(
                ReaderProbeAccess::Unknown,
                "missing-executable",
                "permission-differential",
                false,
            );
        };
        if started.elapsed() >= PROBE_COLD_TIMEOUT {
            return terminal(
                ReaderProbeAccess::Unknown,
                "probe-timeout",
                "permission-differential",
                false,
            );
        }
        // A verified in-process fact needs neither executable metadata nor
        // directory I/O.  The Hook is a one-shot process, so this normalized
        // invocation key is valid for its lifetime; the durable catalog below
        // still uses executable identity before facts cross process boundaries.
        let cache_root = dynamic_cache_root_override
            .map(|root| Ok(root.to_owned()))
            .unwrap_or_else(dynamic_cache_root_path);
        let process_key = cache_root
            .as_ref()
            .ok()
            .map(|root| process_positive_cache_key(root, tokens, &subject));
        if let Some(key) = process_key.as_ref()
            && let Some(behavior_key) = process_positive_cache_hit(key)
        {
            let mut observation = terminal(
                ReaderProbeAccess::Read,
                "reader-behavior-cache-hit",
                "process-memory-reader-catalog",
                false,
            );
            observation.cache_hit = true;
            observation.behavior_key = Some(behavior_key.to_hex().to_string());
            return observation;
        }
        let Ok((envelope_executable, envelope_metadata)) =
            resolve_probe_executable(tokens.first().expect("checked nonempty tokens"))
        else {
            return terminal(
                ReaderProbeAccess::Unknown,
                "executable-unavailable",
                "permission-differential",
                false,
            );
        };
        let behavior_key =
            dynamic_behavior_key(&envelope_executable, &envelope_metadata, tokens, &subject).ok();
        let resolved_cache_root = cache_root.and_then(|root| {
            prepare_dynamic_cache_root(&root)?;
            Ok(root)
        });
        if let Some(key) = behavior_key.as_ref()
            && let Ok(cache_root) = resolved_cache_root.as_ref()
            && dynamic_cache_hit(cache_root, key)
        {
            if let Some(process_key) = process_key.as_ref() {
                publish_process_positive_cache(process_key, key);
            }
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
        let cache_authority = behavior_key
            .as_ref()
            .zip(cache_root.as_ref())
            .and_then(|(key, root)| acquire_cache_shard(root, key, started + PROBE_COLD_TIMEOUT));
        if let Some(DynamicCacheShardAcquire::Published) = cache_authority.as_ref()
            && let Some(key) = behavior_key.as_ref()
        {
            if let Some(process_key) = process_key.as_ref() {
                publish_process_positive_cache(process_key, key);
            }
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
        let cache_lock = match cache_authority {
            Some(DynamicCacheShardAcquire::Owner(guard)) => Some(guard),
            _ => None,
        };
        if let Some(key) = behavior_key.as_ref()
            && let Some(root) = cache_root.as_ref()
            && cache_lock.is_some()
            && dynamic_cache_hit(root, key)
        {
            if let Some(process_key) = process_key.as_ref() {
                publish_process_positive_cache(process_key, key);
            }
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
                "reader-behavior-cache-wait-timeout",
                "state-home-reader-catalog",
                false,
            );
            observation.behavior_key = behavior_key.map(|key| key.to_hex().to_string());
            return observation;
        }
        let Some(_cold_probe) = ColdProbeGuard::try_acquire() else {
            let mut observation = terminal(
                ReaderProbeAccess::Unknown,
                "probe-deferred",
                "permission-differential",
                false,
            );
            observation.behavior_key = behavior_key.map(|key| key.to_hex().to_string());
            return observation;
        };
        if started.elapsed() >= PROBE_COLD_TIMEOUT {
            return terminal(
                ReaderProbeAccess::Unknown,
                "probe-timeout",
                "permission-differential",
                false,
            );
        }
        let Ok(probe_root) = materialize_probe_root() else {
            return terminal(
                ReaderProbeAccess::Unknown,
                "probe-root-failed",
                "permission-differential",
                false,
            );
        };
        if started.elapsed() >= PROBE_COLD_TIMEOUT {
            return terminal(
                ReaderProbeAccess::Unknown,
                "probe-timeout",
                "permission-differential",
                false,
            );
        }
        let Ok(sentinels) = materialize_profile_sentinels(&probe_root, &subject) else {
            return terminal(
                ReaderProbeAccess::Unknown,
                "profile-sentinel-failed",
                "permission-differential",
                false,
            );
        };
        let readable_sentinel = sentinels.readable.to_string_lossy().into_owned();
        let denied_sentinel = sentinels.denied.to_string_lossy().into_owned();
        // Probe the resolved executable itself, including interpreter-backed
        // tools. Codex exposes many real reader commands through shell or
        // Python launchers; excluding shebang executables loses the actual
        // Host Read action and turns the dynamic probe into a native-binary
        // allowlist. The probe still runs only the normalized argv against
        // private permission sentinels, with a cleared environment, process
        // group cleanup, and the shared cold deadline.
        let invocations = resolve_probe_invocations(tokens, &subject, wrapped_command);
        let global_deadline = started + PROBE_COLD_TIMEOUT;
        let mut last_terminal = "no-native-candidate";
        for (executable, probe_tokens) in invocations {
            if Instant::now() >= global_deadline {
                return terminal(
                    ReaderProbeAccess::Unknown,
                    "probe-timeout",
                    "permission-differential",
                    true,
                );
            }
            let readable_argv =
                replace_subject_operand(&probe_tokens, &subject, readable_sentinel.as_str());
            let denied_argv =
                replace_subject_operand(&probe_tokens, &subject, denied_sentinel.as_str());
            if !readable_argv
                .iter()
                .any(|token| token == &readable_sentinel)
                || !denied_argv.iter().any(|token| token == &denied_sentinel)
            {
                continue;
            }
            let readable =
                run_permission_candidate(&probe_root, &executable, &readable_argv, global_deadline);
            let denied = match readable {
                ProbeOutcome::Exited(_) => run_permission_candidate(
                    &probe_root,
                    &executable,
                    &denied_argv,
                    global_deadline,
                ),
                ProbeOutcome::TimedOut => {
                    return terminal(
                        ReaderProbeAccess::Unknown,
                        "probe-timeout",
                        "permission-differential",
                        true,
                    );
                }
                ProbeOutcome::WaitFailed => {
                    last_terminal = "readable-wait-failed";
                    continue;
                }
                ProbeOutcome::CleanupFailed => {
                    return terminal(
                        ReaderProbeAccess::Unknown,
                        "cleanup-failed",
                        "permission-differential",
                        true,
                    );
                }
            };
            match (readable, denied) {
                (ProbeOutcome::Exited(readable), ProbeOutcome::Exited(denied))
                    if exit_signature(&readable) != exit_signature(&denied) =>
                {
                    let cache_publish_failed = if let Some(key) = behavior_key.as_ref()
                        && let Some(root) = cache_root.as_ref()
                        && cache_lock.is_some()
                    {
                        publish_dynamic_cache_record(root, key).is_err()
                            || !dynamic_cache_hit(root, key)
                    } else {
                        false
                    };
                    let terminal_kind = if cache_publish_failed {
                        "read-permission-observed-cache-publish-failed"
                    } else {
                        if let Some(process_key) = process_key.as_ref()
                            && let Some(behavior_key) = behavior_key.as_ref()
                        {
                            publish_process_positive_cache(process_key, behavior_key);
                        }
                        "read-permission-observed"
                    };
                    let mut observation = terminal(
                        ReaderProbeAccess::Read,
                        terminal_kind,
                        "permission-differential",
                        true,
                    );
                    observation.behavior_key = behavior_key.map(|key| key.to_hex().to_string());
                    return observation;
                }
                (_, ProbeOutcome::CleanupFailed) => {
                    return terminal(
                        ReaderProbeAccess::Unknown,
                        "cleanup-failed",
                        "permission-differential",
                        true,
                    );
                }
                (_, ProbeOutcome::TimedOut) => {
                    return terminal(
                        ReaderProbeAccess::Unknown,
                        "probe-timeout",
                        "permission-differential",
                        true,
                    );
                }
                (_, ProbeOutcome::WaitFailed) => last_terminal = "denied-wait-failed",
                (ProbeOutcome::Exited(_), ProbeOutcome::Exited(_)) => {
                    last_terminal = "permission-outcomes-equivalent"
                }
                _ => last_terminal = "permission-observation-incomplete",
            }
        }
        terminal(
            ReaderProbeAccess::Unknown,
            &format!("probe-candidates-exhausted:{last_terminal}"),
            "permission-differential",
            true,
        )
    }
}

#[cfg(target_os = "macos")]
struct ColdProbeGuard;

#[cfg(target_os = "macos")]
impl ColdProbeGuard {
    fn try_acquire() -> Option<Self> {
        COLD_PROBE_IN_FLIGHT
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
            )
            .is_ok()
            .then_some(Self)
    }
}

#[cfg(target_os = "macos")]
impl Drop for ColdProbeGuard {
    fn drop(&mut self) {
        COLD_PROBE_IN_FLIGHT.store(false, std::sync::atomic::Ordering::Release);
    }
}

#[cfg(target_os = "macos")]
fn resolve_probe_executable(executable: &str) -> Result<(PathBuf, std::fs::Metadata), String> {
    use std::os::unix::fs::PermissionsExt as _;

    let path = Path::new(executable);
    let resolved = if path.is_absolute() {
        path.to_owned()
    } else if path.components().count() > 1 {
        std::env::current_dir()
            .map_err(|error| error.to_string())?
            .join(path)
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

#[cfg(target_os = "macos")]
fn resolve_probe_invocations(
    tokens: &[String],
    subject: &str,
    wrapped_command: bool,
) -> Vec<(PathBuf, Vec<String>)> {
    let candidate_count = if wrapped_command {
        tokens
            .iter()
            .position(|token| token == subject)
            .unwrap_or(tokens.len())
    } else {
        tokens.len().min(1)
    };
    (0..candidate_count)
        .filter_map(|index| {
            let candidate = &tokens[index];
            (!candidate.starts_with('-') && !candidate.contains('='))
                .then(|| resolve_probe_executable(candidate).ok())
                .flatten()
                .map(|(resolved, _)| (resolved, tokens[index..].to_vec()))
        })
        .collect()
}

fn static_reader_behavior_matches(
    tokens: &[String],
    subject: &str,
    patterns: &[Vec<String>],
    wrapped_command: bool,
) -> bool {
    patterns.iter().any(|pattern| {
        agent_semantic_shell_parser::command_tokens_match_argv_pattern(
            tokens,
            pattern,
            wrapped_command,
            subject,
        )
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
fn process_positive_cache_key(root: &Path, tokens: &[String], subject: &str) -> blake3::Hash {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"agent.semantic-protocols.reader-process-cache\0schema=1\0");
    hasher.update(root.as_os_str().as_encoded_bytes());
    for token in tokens {
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
    hasher.finalize()
}

#[cfg(target_os = "macos")]
fn process_positive_cache_hit(process_key: &blake3::Hash) -> Option<blake3::Hash> {
    PROCESS_POSITIVE_CACHE
        .get_or_init(|| arc_swap::ArcSwap::from_pointee(Vec::new()))
        .load()
        .iter()
        .find_map(|(cached_process_key, behavior_key)| {
            (cached_process_key == process_key).then_some(*behavior_key)
        })
}

#[cfg(target_os = "macos")]
fn publish_process_positive_cache(process_key: &blake3::Hash, behavior_key: &blake3::Hash) {
    PROCESS_POSITIVE_CACHE
        .get_or_init(|| arc_swap::ArcSwap::from_pointee(Vec::new()))
        .rcu(|current| {
            let mut next = current.as_ref().clone();
            if let Some(index) = next
                .iter()
                .position(|(cached_process_key, _)| cached_process_key == process_key)
            {
                next.remove(index);
            }
            next.push((*process_key, *behavior_key));
            if next.len() > DYNAMIC_CACHE_MAX_RECORDS {
                next.drain(0..next.len() - DYNAMIC_CACHE_MAX_RECORDS);
            }
            Arc::new(next)
        });
}

#[cfg(all(target_os = "macos", test))]
fn clear_process_positive_cache() {
    if let Some(cache) = PROCESS_POSITIVE_CACHE.get() {
        cache.store(Arc::new(Vec::new()));
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
    Ok(agent_semantic_artifacts::StateHomeLayout::new(state_home)
        .cache()
        .reader_behavior())
}

#[cfg(target_os = "macos")]
fn prepare_dynamic_cache_root(root: &Path) -> Result<(), String> {
    ensure_secure_directory(root)?;
    ensure_secure_directory(&root.join("locks"))?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn dynamic_cache_record_path(root: &Path, key: &blake3::Hash) -> PathBuf {
    root.join(format!("{}.bin", key.to_hex()))
}

#[cfg(target_os = "macos")]
fn dynamic_cache_hit(root: &Path, key: &blake3::Hash) -> bool {
    use std::io::Read as _;
    use std::os::unix::fs::MetadataExt as _;
    use std::os::unix::fs::OpenOptionsExt as _;

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
enum DynamicCacheShardAcquire {
    Owner(DynamicCacheShardGuard),
    Published,
}

#[cfg(target_os = "macos")]
fn acquire_cache_shard(
    root: &Path,
    key: &blake3::Hash,
    deadline: Instant,
) -> Option<DynamicCacheShardAcquire> {
    use std::os::unix::fs::OpenOptionsExt as _;

    let shard = key.as_bytes()[0] % DYNAMIC_CACHE_SHARDS;
    // One invocation owns the cold observation. Contenders wait only within
    // the same end-to-end Hook deadline, then consume the committed positive
    // record instead of launching another candidate process.
    let process_guard = loop {
        match DYNAMIC_CACHE_PROCESS_SHARDS[usize::from(shard)].try_lock() {
            Ok(guard) => break guard,
            Err(std::sync::TryLockError::Poisoned(error)) => break error.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => {
                if dynamic_cache_hit(root, key) {
                    return Some(DynamicCacheShardAcquire::Published);
                }
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return None;
                }
                std::thread::park_timeout(std::cmp::min(CACHE_WAIT_PARK, remaining));
            }
        }
    };
    if dynamic_cache_hit(root, key) {
        return Some(DynamicCacheShardAcquire::Published);
    }
    let lock_path = root.join("locks").join(format!("{shard:02x}.lock"));
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(lock_path)
        .ok()?;
    loop {
        match file.try_lock_exclusive() {
            Ok(()) => {
                return Some(DynamicCacheShardAcquire::Owner(DynamicCacheShardGuard {
                    _process: process_guard,
                    _file: file,
                }));
            }
            Err(_) => {
                if dynamic_cache_hit(root, key) {
                    return Some(DynamicCacheShardAcquire::Published);
                }
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return None;
                }
                std::thread::park_timeout(std::cmp::min(CACHE_WAIT_PARK, remaining));
            }
        }
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

#[cfg(test)]
#[path = "../../tests/unit/reader_probe_runtime.rs"]
mod tests;
