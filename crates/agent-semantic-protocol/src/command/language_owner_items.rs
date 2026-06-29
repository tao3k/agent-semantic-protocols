use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::provider_process::{provider_invocation_with_profile, run_provider_command_with_stdin};
use super::search_config::AspConfig;
use super::search_pipe_provider_facts::ProviderGraphFactsContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LanguageOwnerItemsDispatchResult {
    Handled,
    Unsupported,
}

pub(super) struct LanguageOwnerItemsDispatchRequest<'a> {
    pub(super) language_id: &'a str,
    pub(super) args: &'a [String],
    pub(super) owner: &'a Path,
    pub(super) project_root: &'a Path,
    pub(super) cache_home: &'a Path,
    pub(super) config: &'a AspConfig,
    pub(super) provider_context: Option<&'a ProviderGraphFactsContext<'a>>,
}

pub(super) fn dispatch_language_owner_items(
    request: LanguageOwnerItemsDispatchRequest<'_>,
) -> Result<LanguageOwnerItemsDispatchResult, String> {
    let Some(context) = request.provider_context else {
        return Ok(LanguageOwnerItemsDispatchResult::Unsupported);
    };
    let existing_owner_path = language_owner_path_exists(request.project_root, request.owner);
    let invocation = provider_invocation_with_profile(
        context.profiles,
        context.provider,
        request.args,
        request.project_root,
        request.config,
    )?;
    if let Some(cached) = read_owner_items_cache(&request, &invocation)? {
        io::stdout()
            .write_all(cached.as_ref())
            .map_err(|error| format!("failed to write cached provider stdout: {error}"))?;
        return Ok(LanguageOwnerItemsDispatchResult::Handled);
    }
    let output = run_provider_command_with_stdin(
        request.language_id,
        context.provider,
        &invocation,
        request.project_root,
        request.cache_home,
        Vec::new(),
    )?;
    if !output.status.success() {
        if !existing_owner_path {
            return Ok(LanguageOwnerItemsDispatchResult::Unsupported);
        }
        return Err(provider_owner_items_failure(
            "provider-owned owner-items failed",
            request.owner,
            output.stderr.as_ref(),
            existing_owner_path,
        ));
    }
    if output
        .stdout
        .iter()
        .all(|byte| byte.is_ascii_whitespace() || *byte == 0)
    {
        if !existing_owner_path {
            return Ok(LanguageOwnerItemsDispatchResult::Unsupported);
        }
        return Err(provider_owner_items_failure(
            "provider-owned owner-items produced empty output",
            request.owner,
            output.stderr.as_ref(),
            existing_owner_path,
        ));
    }
    io::stderr()
        .write_all(output.stderr.as_ref())
        .map_err(|error| format!("failed to write provider stderr: {error}"))?;
    let stdout = compact_provider_owner_items_stdout(output.stdout.as_ref());
    write_owner_items_cache(&request, &invocation, stdout.as_ref())?;
    io::stdout()
        .write_all(stdout.as_ref())
        .map_err(|error| format!("failed to write provider stdout: {error}"))?;
    Ok(LanguageOwnerItemsDispatchResult::Handled)
}

pub(super) fn language_owner_path_exists(project_root: &Path, owner: &Path) -> bool {
    fs::metadata(language_owner_source_path(project_root, owner)).is_ok()
}

fn read_owner_items_cache(
    request: &LanguageOwnerItemsDispatchRequest<'_>,
    invocation: &[String],
) -> Result<Option<Vec<u8>>, String> {
    let Some(path) = owner_items_cache_path(request, invocation)? else {
        return Ok(None);
    };
    match fs::read(path) {
        Ok(bytes) if !bytes.is_empty() => Ok(Some(bytes)),
        Ok(_) => Ok(None),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("failed to read owner-items cache: {error}")),
    }
}

fn write_owner_items_cache(
    request: &LanguageOwnerItemsDispatchRequest<'_>,
    invocation: &[String],
    stdout: &[u8],
) -> Result<(), String> {
    let Some(path) = owner_items_cache_path(request, invocation)? else {
        return Ok(());
    };
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create owner-items cache dir: {error}"))?;
    fs::write(path, stdout).map_err(|error| format!("failed to write owner-items cache: {error}"))
}

fn owner_items_cache_path(
    request: &LanguageOwnerItemsDispatchRequest<'_>,
    invocation: &[String],
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
    for arg in invocation {
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

fn language_owner_source_path(project_root: &Path, owner: &Path) -> PathBuf {
    if owner.is_absolute() {
        owner.to_path_buf()
    } else {
        project_root.join(owner)
    }
}

fn compact_provider_owner_items_stdout(stdout: &[u8]) -> Vec<u8> {
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

fn provider_owner_items_failure(
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
