//! Runs parser-owned query and search routes before provider activation.

use std::path::Path;

use agent_semantic_hook::HookRuntime;

use super::provider_roots::effective_project_root_and_args;
use super::search_pipe::is_asp_fast_search;

pub(super) fn search_owner_items_owner_path(args: &[String]) -> Option<&str> {
    if args.first().map(String::as_str) != Some("search")
        || args.get(1).map(String::as_str) != Some("owner")
        || args.get(3).map(String::as_str) != Some("items")
        || args.iter().any(|arg| arg == "--json")
    {
        return None;
    }
    args.get(2).map(String::as_str)
}

pub(super) fn run_pre_activation_dynamic_rust_owner_items_search(
    language_id: &str,
    command_args: &[String],
    invocation_root: &Path,
) -> Result<Option<Result<(), String>>, String> {
    if language_id != "rust"
        || !is_asp_fast_search(command_args)
        || search_owner_items_owner_path(command_args).is_none()
    {
        return Ok(None);
    }
    let (project_root, provider_args) = effective_project_root_and_args(
        language_id,
        command_args,
        invocation_root,
        invocation_root,
    )?;
    let config = AspConfig::load(invocation_root, &project_root);
    if !config.language_enabled(language_id) {
        return Ok(Some(Err(format!(
            "language `{language_id}` is disabled by asp.toml"
        ))));
    }
    if super::search_pipe_owner_items_fast::run_pre_activation_dynamic_rust_owner_items_search(
        &provider_args,
        &project_root,
        invocation_root,
    )? {
        return Ok(Some(Ok(())));
    }
    Ok(None)
}

pub(super) fn run_pre_activation_search_command_preflight(
    language_id: &str,
    command_args: &[String],
    invocation_root: &Path,
) -> Result<(), String> {
    let outcome =
        agent_semantic_search::search_command_preflight::preflight_search_command_args_at_invocation_root(
        language_id,
        command_args,
        invocation_root,
    );
    match outcome {
        agent_semantic_search::search_command_preflight::SearchCommandPreflightOutcome::Rejected(
            error,
        ) => Err(error),
        agent_semantic_search::search_command_preflight::SearchCommandPreflightOutcome::Passed
        | agent_semantic_search::search_command_preflight::SearchCommandPreflightOutcome::NotApplicable => {
            Ok(())
        }
    }
}

pub(super) fn run_activated_owner_language_preflight(
    language_id: &str,
    command_args: &[String],
    project_root: &Path,
    expected_extensions: &[String],
    runtime: &HookRuntime,
) -> Result<(), String> {
    let suggested_language = command_args
        .get(2)
        .and_then(|owner| Path::new(owner).extension())
        .and_then(|extension| extension.to_str())
        .and_then(|extension| {
            let mut matches = runtime
                .providers
                .iter()
                .filter(|provider| {
                    agent_semantic_search::search_command_preflight::source_extension_is_declared(
                        extension,
                        &provider.source_extensions,
                    )
                })
                .map(|provider| provider.language_id.as_str());
            let language = matches.next()?;
            matches.next().is_none().then_some(language)
        });
    let outcome = agent_semantic_search::search_command_preflight::
        preflight_search_command_args_with_owner_language_admission(
            language_id,
            command_args,
            project_root,
            agent_semantic_search::search_command_preflight::OwnerItemsLanguageAdmission::new(
                expected_extensions,
                suggested_language,
            ),
        );
    match outcome {
        agent_semantic_search::search_command_preflight::SearchCommandPreflightOutcome::Rejected(
            error,
        ) => Err(error),
        agent_semantic_search::search_command_preflight::SearchCommandPreflightOutcome::Passed
        | agent_semantic_search::search_command_preflight::SearchCommandPreflightOutcome::NotApplicable => {
            Ok(())
        }
    }
}
use super::search_config::AspConfig;
