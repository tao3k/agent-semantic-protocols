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
