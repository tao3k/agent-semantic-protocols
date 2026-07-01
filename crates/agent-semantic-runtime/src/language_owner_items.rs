//! Runtime dispatch, cache, and compact render policy for language harness `owner-items`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Outcome of one owner-items adapter attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageOwnerItemsAttempt {
    /// The adapter wrote or returned the owner-items response.
    Handled,
    /// The adapter is not available for this request.
    Unsupported,
}

/// Ordered owner-items dispatch request owned by the runtime layer.
pub struct LanguageOwnerItemsDispatchPlan<'a, Provider>
where
    Provider: FnOnce() -> Result<LanguageOwnerItemsAttempt, String>,
{
    pub language_id: &'a str,
    pub owner: &'a Path,
    pub project_root: &'a Path,
    pub provider: Provider,
}

/// Request describing one language harness `owner-items` cache entry.
pub struct LanguageOwnerItemsCacheRequest<'a> {
    pub language_id: &'a str,
    pub args: &'a [String],
    pub invocation: &'a [String],
    pub owner: &'a Path,
    pub project_root: &'a Path,
    pub cache_home: &'a Path,
}

/// Provider process output projected into runtime-owned owner-items policy.
pub struct LanguageOwnerItemsProviderOutput<'a> {
    pub status_success: bool,
    pub stdout: &'a [u8],
    pub stderr: &'a [u8],
}

/// Runtime decision after applying owner-items cache, compaction, and
/// fail-closed policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LanguageOwnerItemsRuntimeOutcome {
    Handled {
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        cache_hit: bool,
    },
    Unsupported,
    Failed(String),
}

/// Runtime-owned performance receipt for one owner-items resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageOwnerItemsRuntimeReceipt {
    pub outcome: String,
    pub provider_process_count: usize,
    pub stdout_bytes: usize,
    pub stderr_bytes: usize,
    pub cache_hit: bool,
    pub fallback_reason: String,
    pub elapsed_ms: u128,
}

/// Run owner-items dispatch through the shared provider-owned policy.
///
/// The command layer supplies the provider adapter, but the runtime owns the
/// language-agnostic policy: existing owner check, provider execution, and
/// fail-closed messaging when no owner-items interface is available.
pub fn run_language_owner_items_dispatch_plan<Provider>(
    plan: LanguageOwnerItemsDispatchPlan<'_, Provider>,
) -> Result<LanguageOwnerItemsAttempt, String>
where
    Provider: FnOnce() -> Result<LanguageOwnerItemsAttempt, String>,
{
    if !language_owner_path_exists(plan.project_root, plan.owner) {
        return Err(format!(
            "{} search owner items requires an existing owner path `{}`; no provider executed",
            plan.language_id,
            plan.owner.display()
        ));
    }
    if (plan.provider)()? == LanguageOwnerItemsAttempt::Handled {
        return Ok(LanguageOwnerItemsAttempt::Handled);
    }
    Err(format!(
        "{} search owner items requires a language-harness owner-items interface for `{}`; ASP will not synthesize language items from source text",
        plan.language_id,
        plan.owner.display()
    ))
}

/// Return whether an `owner-items` owner path exists below the project root.
pub fn language_owner_path_exists(project_root: &Path, owner: &Path) -> bool {
    fs::metadata(language_owner_source_path(project_root, owner)).is_ok()
}

/// Resolve an `owner-items` source path relative to the project root.
#[must_use]
pub fn language_owner_source_path(project_root: &Path, owner: &Path) -> PathBuf {
    if owner.is_absolute() {
        owner.to_path_buf()
    } else {
        project_root.join(owner)
    }
}

/// Read a cached language harness `owner-items` response when present.
pub fn read_language_owner_items_cache(
    request: &LanguageOwnerItemsCacheRequest<'_>,
) -> Result<Option<Vec<u8>>, String> {
    let Some(path) = owner_items_cache_path(request)? else {
        return Ok(None);
    };
    match fs::read(path) {
        Ok(bytes) if !bytes.is_empty() => Ok(Some(bytes)),
        Ok(_) => Ok(None),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("failed to read owner-items cache: {error}")),
    }
}

/// Write a successful language harness `owner-items` response to the cache.
pub fn write_language_owner_items_cache(
    request: &LanguageOwnerItemsCacheRequest<'_>,
    stdout: &[u8],
) -> Result<(), String> {
    let Some(path) = owner_items_cache_path(request)? else {
        return Ok(());
    };
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create owner-items cache dir: {error}"))?;
    fs::write(path, stdout).map_err(|error| format!("failed to write owner-items cache: {error}"))
}

/// Resolve cached or fresh provider output through runtime-owned owner-items
/// policy. Command layers provide process output and perform final I/O only.
pub fn resolve_language_owner_items_runtime_outcome(
    request: &LanguageOwnerItemsCacheRequest<'_>,
    existing_owner_path: bool,
    provider_output: Option<LanguageOwnerItemsProviderOutput<'_>>,
) -> Result<LanguageOwnerItemsRuntimeOutcome, String> {
    if let Some(cached) = read_language_owner_items_cache(request)? {
        return Ok(LanguageOwnerItemsRuntimeOutcome::Handled {
            stdout: cached,
            stderr: Vec::new(),
            cache_hit: true,
        });
    }
    let Some(output) = provider_output else {
        return Ok(LanguageOwnerItemsRuntimeOutcome::Unsupported);
    };
    if !output.status_success {
        if !existing_owner_path {
            return Ok(LanguageOwnerItemsRuntimeOutcome::Unsupported);
        }
        return Ok(LanguageOwnerItemsRuntimeOutcome::Failed(
            language_owner_items_failure(
                "provider-owned owner-items failed",
                request.owner,
                output.stderr,
                existing_owner_path,
            ),
        ));
    }
    if output
        .stdout
        .iter()
        .all(|byte| byte.is_ascii_whitespace() || *byte == 0)
    {
        if !existing_owner_path {
            return Ok(LanguageOwnerItemsRuntimeOutcome::Unsupported);
        }
        return Ok(LanguageOwnerItemsRuntimeOutcome::Failed(
            language_owner_items_failure(
                "provider-owned owner-items produced empty output",
                request.owner,
                output.stderr,
                existing_owner_path,
            ),
        ));
    }
    let stdout = compact_language_owner_items_stdout(output.stdout);
    write_language_owner_items_cache(request, stdout.as_ref())?;
    Ok(LanguageOwnerItemsRuntimeOutcome::Handled {
        stdout,
        stderr: output.stderr.to_vec(),
        cache_hit: false,
    })
}

/// Build the runtime-owned performance receipt for owner-items policy.
#[must_use]
pub fn language_owner_items_runtime_receipt(
    outcome: &LanguageOwnerItemsRuntimeOutcome,
    provider_process_count: usize,
    elapsed_ms: u128,
) -> LanguageOwnerItemsRuntimeReceipt {
    match outcome {
        LanguageOwnerItemsRuntimeOutcome::Handled {
            stdout,
            stderr,
            cache_hit,
        } => LanguageOwnerItemsRuntimeReceipt {
            outcome: "handled".to_string(),
            provider_process_count,
            stdout_bytes: stdout.len(),
            stderr_bytes: stderr.len(),
            cache_hit: *cache_hit,
            fallback_reason: "none".to_string(),
            elapsed_ms,
        },
        LanguageOwnerItemsRuntimeOutcome::Unsupported => LanguageOwnerItemsRuntimeReceipt {
            outcome: "unsupported".to_string(),
            provider_process_count,
            stdout_bytes: 0,
            stderr_bytes: 0,
            cache_hit: false,
            fallback_reason: "unsupported-owner-items-interface".to_string(),
            elapsed_ms,
        },
        LanguageOwnerItemsRuntimeOutcome::Failed(message) => LanguageOwnerItemsRuntimeReceipt {
            outcome: "failed".to_string(),
            provider_process_count,
            stdout_bytes: 0,
            stderr_bytes: message.len(),
            cache_hit: false,
            fallback_reason: "fail-closed-no-fallback".to_string(),
            elapsed_ms,
        },
    }
}

/// Compact language harness stdout into the bounded `owner-items` cache shape.
pub fn compact_language_owner_items_stdout(stdout: &[u8]) -> Vec<u8> {
    String::from_utf8_lossy(stdout)
        .lines()
        .filter(|line| !default_search_internal_line(line))
        .fold(String::new(), |mut rendered, line| {
            rendered.push_str(line);
            rendered.push('\n');
            rendered
        })
        .into_bytes()
}

/// Render a compact failure packet for language harness `owner-items`.
pub fn language_owner_items_failure(
    message: &str,
    owner: &Path,
    stderr: &[u8],
    existing_owner_path: bool,
) -> String {
    let owner_state = if existing_owner_path {
        "existing owner path"
    } else {
        "owner"
    };
    let mut failure = format!(
        "{message} for {owner_state} `{}`; no fallback executed",
        owner.display()
    );
    let provider_stderr = String::from_utf8_lossy(stderr).trim().to_string();
    if !provider_stderr.is_empty() {
        failure.push_str(": ");
        failure.push_str(&provider_stderr);
    }
    failure
}

fn owner_items_cache_path(
    request: &LanguageOwnerItemsCacheRequest<'_>,
) -> Result<Option<PathBuf>, String> {
    let owner_path = language_owner_source_path(request.project_root, request.owner);
    let Ok(owner_bytes) = fs::read(&owner_path) else {
        return Ok(None);
    };
    let mut hasher = Sha256::new();
    hasher.update(b"language-owner-items-cache-v1");
    hasher.update(request.language_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(owner_path.to_string_lossy().as_bytes());
    hasher.update(b"\0");
    hasher.update(&owner_bytes);
    hasher.update(b"\0");
    for arg in request.invocation {
        hasher.update(arg.as_bytes());
        hasher.update(b"\0");
    }
    for arg in request.args {
        hasher.update(arg.as_bytes());
        hasher.update(b"\0");
    }
    let digest = format!("{:x}", hasher.finalize());
    Ok(Some(
        request
            .cache_home
            .join("search")
            .join("language-owner-items")
            .join(format!("{digest}.stdout")),
    ))
}

fn default_search_internal_line(line: &str) -> bool {
    matches!(
        line,
        "actionFrontier=" | "recommendedNext=" | "rankedEvidence=" | "evidenceFrontier="
    ) || line.starts_with("actionFrontier=")
        || line.starts_with("recommendedNext=")
        || line.starts_with("rankedEvidence=")
        || line.starts_with("evidenceFrontier=")
        || line.starts_with("commandHandles=")
        || line.starts_with("treeSitterHandles=")
        || line.starts_with("[graph-frontier]")
        || line.starts_with("[route-graph]")
}
