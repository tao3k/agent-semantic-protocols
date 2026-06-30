use std::io::{self, Write};
use std::path::Path;

use super::provider_process::{provider_invocation_with_profile, run_provider_command_with_stdin};
use super::search_config::AspConfig;
use super::search_pipe_provider_facts::ProviderGraphFactsContext;
use agent_semantic_runtime::{
    LanguageOwnerItemsAttempt, LanguageOwnerItemsCacheRequest, compact_language_owner_items_stdout,
    language_owner_items_failure, language_owner_path_exists, read_language_owner_items_cache,
    write_language_owner_items_cache,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LanguageOwnerItemsDispatchResult {
    Handled,
    Unsupported,
}

impl From<LanguageOwnerItemsDispatchResult> for LanguageOwnerItemsAttempt {
    fn from(value: LanguageOwnerItemsDispatchResult) -> Self {
        match value {
            LanguageOwnerItemsDispatchResult::Handled => LanguageOwnerItemsAttempt::Handled,
            LanguageOwnerItemsDispatchResult::Unsupported => LanguageOwnerItemsAttempt::Unsupported,
        }
    }
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
    let cache_request = LanguageOwnerItemsCacheRequest {
        language_id: request.language_id,
        args: request.args,
        invocation: &invocation,
        owner: request.owner,
        project_root: request.project_root,
        cache_home: request.cache_home,
    };
    if let Some(cached) = read_language_owner_items_cache(&cache_request)? {
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
        return Err(language_owner_items_failure(
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
        return Err(language_owner_items_failure(
            "provider-owned owner-items produced empty output",
            request.owner,
            output.stderr.as_ref(),
            existing_owner_path,
        ));
    }
    io::stderr()
        .write_all(output.stderr.as_ref())
        .map_err(|error| format!("failed to write provider stderr: {error}"))?;
    let stdout = compact_language_owner_items_stdout(output.stdout.as_ref());
    write_language_owner_items_cache(&cache_request, stdout.as_ref())?;
    io::stdout()
        .write_all(stdout.as_ref())
        .map_err(|error| format!("failed to write provider stdout: {error}"))?;
    Ok(LanguageOwnerItemsDispatchResult::Handled)
}
