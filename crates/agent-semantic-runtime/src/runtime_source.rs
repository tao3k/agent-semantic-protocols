//! Runtime-source checkout management for ASP-managed language source facts.

use std::{
    env, fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::Command,
};

use crate::project_runtime_state;

/// Source checkout request derived from a provider-owned runtime-source packet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSourceSpec {
    pub language_id: String,
    pub repository: String,
    pub checkout: String,
    pub state_namespace: String,
    pub index_owner: String,
}

/// ASP-managed checkout location for version-matched runtime source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSourceCheckout {
    pub language_id: String,
    pub repository: String,
    pub checkout: String,
    pub state_namespace: String,
    pub index_owner: String,
    pub checkout_dir: PathBuf,
}

/// Runtime-source identity prepared for source-index refresh.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSourceIndexContext {
    pub checkout_root: PathBuf,
    pub registry_fingerprint: String,
}

/// Runtime-source file prepared for source-index import.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSourceIndexFile {
    pub path: PathBuf,
    pub language_id: String,
    pub provider_id: String,
}

/// Resolve the ASP-managed checkout directory for a runtime source version.
pub fn runtime_source_checkout_dir(
    project_root: impl AsRef<Path>,
    state_namespace: &str,
    version_key: &str,
) -> Result<PathBuf, String> {
    let state = project_runtime_state(project_root)?;
    runtime_source_checkout_dir_in_client_cache(
        state.client_cache_dir,
        state_namespace,
        version_key,
    )
}

/// Resolve a runtime source checkout below an already-resolved client cache directory.
pub fn runtime_source_checkout_dir_in_client_cache(
    client_cache_dir: impl AsRef<Path>,
    state_namespace: &str,
    version_key: &str,
) -> Result<PathBuf, String> {
    let mut dir = client_cache_dir.as_ref().to_path_buf();
    for segment in state_namespace.split('/') {
        dir.push(safe_path_segment(segment)?);
    }
    dir.push(safe_path_segment(version_key)?);
    Ok(dir)
}

/// Clone or fetch a runtime source repository and checkout the requested version.
pub fn ensure_runtime_source_checkout(
    project_root: impl AsRef<Path>,
    spec: &RuntimeSourceSpec,
) -> Result<RuntimeSourceCheckout, String> {
    let checkout_dir =
        runtime_source_checkout_dir(project_root, &spec.state_namespace, &spec.checkout)?;
    ensure_runtime_source_checkout_at(checkout_dir, spec)
}

/// Clone or fetch a runtime source below an already-resolved client cache directory.
pub fn ensure_runtime_source_checkout_in_client_cache(
    client_cache_dir: impl AsRef<Path>,
    spec: &RuntimeSourceSpec,
) -> Result<RuntimeSourceCheckout, String> {
    let checkout_dir = runtime_source_checkout_dir_in_client_cache(
        client_cache_dir,
        &spec.state_namespace,
        &spec.checkout,
    )?;
    ensure_runtime_source_checkout_at(checkout_dir, spec)
}

/// Resolve runtime-source index identity under an ASP-managed client cache.
pub fn runtime_source_index_context(
    checkout_root: impl AsRef<Path>,
    client_cache_dir: impl AsRef<Path>,
    language_id: &str,
    provider_id: &str,
) -> Result<RuntimeSourceIndexContext, String> {
    let checkout_root = fs::canonicalize(checkout_root.as_ref()).map_err(|error| {
        format!(
            "failed to resolve runtime source checkout {}: {error}",
            checkout_root.as_ref().display()
        )
    })?;
    let canonical_cache_dir = fs::canonicalize(client_cache_dir.as_ref()).map_err(|error| {
        format!(
            "failed to resolve ASP client cache dir {}: {error}",
            client_cache_dir.as_ref().display()
        )
    })?;
    if !checkout_root.starts_with(&canonical_cache_dir) {
        return Err(format!(
            "runtime source checkout {} is outside ASP client cache {}",
            checkout_root.display(),
            canonical_cache_dir.display()
        ));
    }

    Ok(RuntimeSourceIndexContext {
        registry_fingerprint: runtime_source_registry_fingerprint(
            &checkout_root,
            language_id,
            provider_id,
        ),
        checkout_root,
    })
}

/// Build the stable registry fingerprint for ASP-managed runtime source facts.
pub fn runtime_source_registry_fingerprint(
    checkout_root: &Path,
    language_id: &str,
    provider_id: &str,
) -> String {
    format!(
        "runtimeSource\ngenerationRoot={}\nlanguage={}\nprovider={}",
        checkout_root.display(),
        language_id,
        provider_id
    )
}

/// Collect source files from an ASP-managed runtime source checkout.
pub fn collect_runtime_source_index_files(
    checkout_root: impl AsRef<Path>,
    language_id: &str,
    provider_id: &str,
    limit: usize,
) -> Result<Vec<RuntimeSourceIndexFile>, String> {
    let mut files = Vec::new();
    collect_runtime_source_index_files_from_dir(
        checkout_root.as_ref(),
        language_id,
        provider_id,
        limit,
        &mut files,
    )?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    files.truncate(limit);
    Ok(files)
}

fn collect_runtime_source_index_files_from_dir(
    dir: &Path,
    language_id: &str,
    provider_id: &str,
    limit: usize,
    files: &mut Vec<RuntimeSourceIndexFile>,
) -> Result<(), String> {
    if files.len() >= limit {
        return Ok(());
    }
    let mut entries = fs::read_dir(dir)
        .map_err(|error| {
            format!(
                "failed to read runtime source dir {}: {error}",
                dir.display()
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to read runtime source dir entry: {error}"))?;
    entries.sort_by_key(std::fs::DirEntry::path);
    for entry in entries {
        if files.len() >= limit {
            break;
        }
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            format!(
                "failed to inspect runtime source file type {}: {error}",
                path.display()
            )
        })?;
        if file_type.is_dir() {
            if !runtime_source_dir_is_skipped(&path) {
                collect_runtime_source_index_files_from_dir(
                    &path,
                    language_id,
                    provider_id,
                    limit,
                    files,
                )?;
            }
        } else if file_type.is_file() && runtime_source_file_matches(language_id, &path) {
            files.push(RuntimeSourceIndexFile {
                path,
                language_id: language_id.to_string(),
                provider_id: provider_id.to_string(),
            });
        }
    }
    Ok(())
}

fn runtime_source_dir_is_skipped(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| matches!(name, ".git" | ".hg" | ".svn"))
}

fn runtime_source_file_matches(language_id: &str, path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return false;
    };
    let extension = extension.to_ascii_lowercase();
    match language_id {
        "gerbil-scheme" => matches!(extension.as_str(), "ss" | "scm" | "sld" | "sch" | "scheme"),
        "julia" => extension == "jl",
        "python" => extension == "py",
        "rust" => extension == "rs",
        "typescript" => matches!(
            extension.as_str(),
            "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs"
        ),
        _ => false,
    }
}

fn ensure_runtime_source_checkout_at(
    checkout_dir: PathBuf,
    spec: &RuntimeSourceSpec,
) -> Result<RuntimeSourceCheckout, String> {
    let parent = checkout_dir.parent().ok_or_else(|| {
        format!(
            "runtime source path has no parent: {}",
            checkout_dir.display()
        )
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "failed to create runtime source namespace {}: {error}",
            parent.display()
        )
    })?;

    if checkout_dir.join(".git").is_dir() {
        ensure_matching_remote(&checkout_dir, &spec.repository)?;
        run_git(&checkout_dir, ["fetch", "--tags", "--force", "origin"])?;
    } else {
        let args = [
            "clone",
            "--no-checkout",
            spec.repository.as_str(),
            checkout_dir
                .to_str()
                .ok_or_else(|| format!("non-utf8 checkout path: {}", checkout_dir.display()))?,
        ];
        run_git(parent, args)?;
    }
    run_git(
        &checkout_dir,
        ["checkout", "--force", spec.checkout.as_str()],
    )?;

    Ok(RuntimeSourceCheckout {
        language_id: spec.language_id.clone(),
        repository: spec.repository.clone(),
        checkout: spec.checkout.clone(),
        state_namespace: spec.state_namespace.clone(),
        index_owner: spec.index_owner.clone(),
        checkout_dir,
    })
}

fn ensure_matching_remote(checkout_dir: &Path, repository: &str) -> Result<(), String> {
    let output = git_output(checkout_dir, ["remote", "get-url", "origin"])?;
    let actual = output.trim();
    if actual == repository {
        Ok(())
    } else {
        Err(format!(
            "runtime source checkout remote mismatch: expected {repository}, found {actual}"
        ))
    }
}

fn safe_path_segment(segment: &str) -> Result<&str, String> {
    if segment.is_empty() || segment == "." || segment == ".." {
        return Err(format!("invalid runtime source path segment: {segment:?}"));
    }
    let valid = segment
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'));
    if valid {
        Ok(segment)
    } else {
        Err(format!("invalid runtime source path segment: {segment:?}"))
    }
}

fn run_git<const N: usize>(cwd: &Path, args: [&str; N]) -> Result<(), String> {
    let output = git_output_bytes(cwd, args)?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(format!(
        "git failed in {} with status {}: {}",
        cwd.display(),
        output.status,
        stderr.trim()
    ))
}

fn git_output<const N: usize>(cwd: &Path, args: [&str; N]) -> Result<String, String> {
    let output = git_output_bytes(cwd, args)?;
    if output.status.success() {
        String::from_utf8(output.stdout)
            .map_err(|error| format!("git output was not utf8: {error}"))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "git failed in {} with status {}: {}",
            cwd.display(),
            output.status,
            stderr.trim()
        ))
    }
}

fn git_output_bytes<const N: usize>(
    cwd: &Path,
    args: [&str; N],
) -> Result<std::process::Output, String> {
    let mut not_found = Vec::new();
    for git in git_command_candidates() {
        let output = match Command::new(&git).args(args).current_dir(cwd).output() {
            Ok(output) => output,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                not_found.push(format!("{git}: {error}"));
                continue;
            }
            Err(error) => {
                return Err(format!("failed to run {git}: {error}"));
            }
        };
        if output.status.success() || !looks_like_tool_resolution_failure(&output.stderr) {
            return Ok(output);
        }
        not_found.push(format!(
            "{git}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Err(format!("failed to find git: {}", not_found.join("; ")))
}

fn git_command_candidates() -> Vec<String> {
    let mut candidates = Vec::new();
    push_git_candidate_from_env(&mut candidates, "ASP_GIT_BIN");
    candidates.push("git".to_string());
    for path in [
        "/usr/bin/git",
        "/opt/homebrew/bin/git",
        "/usr/local/bin/git",
        "/run/current-system/sw/bin/git",
    ] {
        push_git_candidate(&mut candidates, path);
    }
    if let Ok(user) = env::var("USER") {
        push_git_candidate(
            &mut candidates,
            &format!("/etc/profiles/per-user/{user}/bin/git"),
        );
    }
    if let Ok(home) = env::var("HOME") {
        push_git_candidate(&mut candidates, &format!("{home}/.nix-profile/bin/git"));
    }
    candidates
}

fn push_git_candidate_from_env(candidates: &mut Vec<String>, key: &str) {
    if let Ok(value) = env::var(key) {
        push_git_candidate(candidates, value.trim());
    }
}

fn push_git_candidate(candidates: &mut Vec<String>, path: &str) {
    if !path.is_empty() && !candidates.iter().any(|candidate| candidate == path) {
        candidates.push(path.to_string());
    }
}

fn looks_like_tool_resolution_failure(stderr: &[u8]) -> bool {
    let stderr = String::from_utf8_lossy(stderr);
    stderr.contains("tool 'git' not found") || stderr.contains("tool `git` not found")
}

#[cfg(test)]
#[path = "../tests/unit/runtime_source.rs"]
mod runtime_source_tests;
