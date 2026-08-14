//! Language provider command facade.

use super::provider_execution::take_frontier_receipt_request;
use super::provider_usage;

use super::graph::GraphTurboReceiptRequest;
use agent_semantic_hook::runtime_profiles_for_runtime;
use agent_semantic_runtime::project_state_paths;
use std::env;
use std::path::Path;

use super::client_backend_worker::run_client_backend_on_worker;
use super::gerbil_check_cache::try_replay_gerbil_check_cache;
use super::gerbil_deps::try_run_gerbil_deps_index_command;
use super::protocol_version_line;
use super::provider_fast_path::run_activated_owner_language_preflight;
use super::provider_process::{
    provider_invocation_with_profile, provider_invocations, run_guide_command, run_provider_command,
};
use super::provider_roots::{activation_project_root, effective_project_root_and_args};
pub(crate) use super::provider_selector::{
    is_language_facade, unsupported_language_facade_message,
};
use super::search_config::AspConfig;
use super::search_owner_items::{
    SearchOwnerItemsContext, is_search_owner_items_query, run_search_owner_items_query_command,
};
use provider_usage::{
    guide_usage, is_guide, provider_guide_args, provider_usage, validate_provider_command,
};

/// Observational target only; it never controls or cancels search execution.
const SEARCH_DIAGNOSTIC_SLOW_TARGET_MICROS: u64 = 500_000;

macro_rules! restore_env_var {
    ($name:expr, $previous:expr) => {
        match $previous {
            Some(value) => unsafe {
                env::set_var($name, value);
            },
            None => unsafe {
                env::remove_var($name);
            },
        }
    };
}

fn exact_query_trace(stage: &str, started: tokio::time::Instant) {
    if env::var_os("ASP_EXACT_QUERY_TRACE").is_some() {
        eprintln!(
            "[exact-query-trace] stage={stage} elapsedMicros={}",
            started.elapsed().as_micros()
        );
    }
}

pub(super) fn tree_sitter_runtime_profiles(
    project_root: &Path,
    runtime: &agent_semantic_hook::HookRuntime,
) -> agent_semantic_hook::RuntimeProfiles {
    runtime_profiles_for_runtime(project_root, runtime)
}

pub(crate) async fn run_language_command(
    language_id: &str,
    args: &[String],
    process_started: tokio::time::Instant,
) -> Result<(), String> {
    let exact_query_started = process_started;
    fn uses_client_backend(args: &[String]) -> bool {
        (args.first().is_some_and(|command| command == "search")
            && args.get(1).is_none_or(|subcommand| subcommand != "guide"))
            || matches!(args.first().map(String::as_str), Some("check"))
            || matches!(args.first().map(String::as_str), Some("cache"))
    }

    async fn run_client_backend_command(
        language_id: &str,
        args: &[String],
        project_root: &Path,
        frontier_receipt: Option<&GraphTurboReceiptRequest>,
    ) -> Result<(), String> {
        let mut client_args = args.to_vec();
        if let Some(receipt) = frontier_receipt {
            if receipt.has_extra_args() {
                return Err(
                    "--frontier-receipt-* fact flags require an ASP graph-turbo fast search"
                        .to_string(),
                );
            }
            client_args.extend([
                "--frontier-receipt-out".to_string(),
                receipt.out_path.display().to_string(),
            ]);
        }
        let previous_runtime_bin = env::var_os("ASP_RUNTIME_BIN_DIR");
        let previous_protocol_bin = env::var_os("SEMANTIC_AGENT_PROTOCOL_BIN");
        let previous_path = env::var_os("PATH");
        let protocol_bin = env::current_exe()
            .map_err(|error| format!("failed to resolve current protocol binary: {error}"))?;
        let runtime_bin = project_state_paths(project_root)?.runtime_bin_dir;
        let mut path_entries = vec![runtime_bin.clone()];
        if let Some(path) = previous_path.as_deref() {
            path_entries.extend(env::split_paths(path));
        }
        let runtime_path = env::join_paths(path_entries).ok();
        unsafe {
            env::set_var("ASP_RUNTIME_BIN_DIR", &runtime_bin);
            env::set_var("SEMANTIC_AGENT_PROTOCOL_BIN", &protocol_bin);
            if let Some(path) = runtime_path.as_deref() {
                env::set_var("PATH", path);
            }
        }
        let result =
            run_client_backend_on_worker(language_id, client_args, project_root.to_path_buf())
                .await;
        restore_env_var!("ASP_RUNTIME_BIN_DIR", previous_runtime_bin);
        restore_env_var!("SEMANTIC_AGENT_PROTOCOL_BIN", previous_protocol_bin);
        restore_env_var!("PATH", previous_path);
        result
    }

    if !is_language_facade(language_id) {
        let runtime = load_activation_for_language_message();
        return Err(unsupported_language_facade_message(
            language_id,
            args.first().map(String::as_str),
            runtime.as_ref(),
        ));
    }
    let mut command_args = args.to_vec();
    let diagnostic_options =
        agent_semantic_search::command_diagnostics::take_search_command_diagnostic_options(
            &mut command_args,
        )?;
    let frontier_receipt = take_frontier_receipt_request(&mut command_args)?;
    let mut command_diagnostics = diagnostic_options
        .is_enabled()
        .then(|| {
            agent_semantic_search::command_diagnostics::SearchCommandDiagnostics::start(
                language_id,
                &command_args,
                diagnostic_options,
                process_started.into_std(),
            )
        })
        .flatten();
    let result: Result<(), String> = async {
    if frontier_receipt.is_some()
        && command_args
            .first()
            .is_none_or(|command| command != "search")
    {
        return Err("--frontier-receipt-out is supported only for search commands".to_string());
    }

    if is_help(&command_args) {
        println!("{}", provider_usage());
        return Ok(());
    }
    if is_version(&command_args) {
        println!("{}", protocol_version_line());
        return Ok(());
    }
    let invocation_root =
        env::current_dir().map_err(|error| format!("failed to read current directory: {error}"))?;
    validate_provider_command(&command_args)?;
    if is_guide_help(&command_args) {
        println!("{}", guide_usage(language_id));
        return Ok(());
    }
    if try_run_gerbil_deps_index_command(language_id, &command_args)? {
        return Ok(());
    }
    if is_provider_owned_structural_selector_query(language_id, &command_args)
        && agent_semantic_hook::registered_provider_kind(language_id)?
            == agent_semantic_hook::RegisteredProviderKind::ProgrammingLanguage
    {
        if let Some(diagnostics) = command_diagnostics.as_mut() {
            diagnostics.mark_stage("exact-query-resident-dispatch");
        }
        let (exact_project_root, exact_provider_args) =
            super::provider_roots::explicit_workspace_project_root(
                language_id,
                &command_args,
                &invocation_root,
            )?
            .unwrap_or_else(|| (invocation_root.clone(), command_args.clone()));
        // Exact projection is terminal at the admitted Runtime generation.
        // Missing authority must never fall through to public provider argv.
        return super::provider_resident_exact::run_resident_exact_query(
            language_id,
            &exact_provider_args,
            &exact_project_root,
            exact_query_started,
        )
        .await;
    }
    if is_search_owner_items_query(&command_args) {
        let (owner_project_root, owner_args) =
            super::provider_roots::explicit_workspace_project_root(
                language_id,
                &command_args,
                &invocation_root,
            )?
            .unwrap_or_else(|| (invocation_root.clone(), command_args.clone()));
        return run_search_owner_items_query_command(
            &owner_args,
            SearchOwnerItemsContext {
                language_id,
                project_root: &owner_project_root,
                locator_root: &invocation_root,
                frontier_receipt: frontier_receipt.as_ref(),
            },
        )
        .await;
    }
    if uses_client_backend(&command_args) {
        let (project_root, provider_args) =
            super::provider_roots::explicit_workspace_project_root(
                language_id,
                &command_args,
                &invocation_root,
            )?
            .unwrap_or_else(|| (invocation_root.clone(), command_args.clone()));
        if super::workspace_tree_sitter_query::try_run_workspace_tree_sitter_query(
            language_id,
            &provider_args,
            &project_root,
        )
        .await?
        {
            return Ok(());
        }
        return run_client_backend_command(
            language_id,
            &provider_args,
            &project_root,
            frontier_receipt.as_ref(),
        )
        .await;
    }
    let canonical_activation_path = provider_activation_path(&invocation_root);
    let activation_path = canonical_activation_path.clone();
    let runtime = super::provider_activation::load_activation_for_language(
        &activation_path,
        &invocation_root,
        language_id,
    )
    .await?;
    exact_query_trace("activation-loaded", exact_query_started);
    if let Some(diagnostics) = command_diagnostics.as_mut() {
        diagnostics.mark_stage("activation-loaded");
    }
    let activation_path = canonical_activation_path;
    let activation_root = activation_project_root(&activation_path, &runtime.project_root);
    let config = AspConfig::load(&invocation_root, &activation_root).await;
    let (project_root, provider_args) = effective_project_root_and_args(
        language_id,
        &command_args,
        &invocation_root,
        &activation_root,
    )?;
    exact_query_trace("workspace-resolved", exact_query_started);
    if let Some(diagnostics) = command_diagnostics.as_mut() {
        diagnostics.mark_stage("workspace-resolved");
    }
    if !config.language_enabled(language_id) {
        return Err(format!("language `{language_id}` is disabled by asp.toml"));
    }

    let provider = runtime
        .providers
        .iter()
        .find(|provider| provider.language_id == language_id)
        .ok_or_else(|| {
            let active_languages = runtime
                .providers
                .iter()
                .map(|provider| provider.language_id.as_str())
                .collect::<Vec<_>>()
                .join("|");
            format!(
                "no activated provider for language {language_id}; activation={}; activeLanguages={}",
                activation_path.display(),
                if active_languages.is_empty() {
                    "none".to_string()
                } else {
                    active_languages
                }
            )
        })?;
    run_activated_owner_language_preflight(
        language_id,
        &provider_args,
        &project_root,
        &provider.source_extensions,
        &runtime,
    )?;
    exact_query_trace("owner-preflight-complete", exact_query_started);
    if let Some(diagnostics) = command_diagnostics.as_mut() {
        diagnostics.mark_stage("owner-preflight-complete");
    }
    if frontier_receipt
        .as_ref()
        .is_some_and(GraphTurboReceiptRequest::has_extra_args)
    {
        return Err(
            "--frontier-receipt-* fact flags require an ASP graph-turbo fast search".to_string(),
        );
    }
    if try_replay_gerbil_check_cache(language_id, &provider_args, &project_root)? {
        return Ok(());
    }
    let runtime_profiles = runtime_profiles_for_runtime(&project_root, &runtime);
    if is_guide(&command_args) {
        let guide_args = provider_guide_args(language_id, &provider_args);
        let invocation =
            provider_invocation_with_profile(&runtime_profiles, provider, &guide_args)?;
        return run_guide_command(language_id, provider, &invocation, &project_root).await;
    }
    let provider_argv = provider_process_args(&provider_args);
    for invocation in
        provider_invocations(provider, &provider_argv, &project_root, &runtime_profiles)?
    {
        run_provider_command(language_id, provider, &invocation, &project_root, false).await?;
    }
    Ok(())
    }
    .await;
    if let Some(diagnostics) = command_diagnostics.take() {
        let receipt = diagnostics.finish(
            SEARCH_DIAGNOSTIC_SLOW_TARGET_MICROS,
            result.as_ref().err().map(String::as_str),
        );
        eprintln!(
            "{}",
            serde_json::to_string(&receipt)
                .map_err(|error| format!("failed to encode search diagnostics receipt: {error}"))?
        );
    }
    result
}

fn is_help(args: &[String]) -> bool {
    matches!(
        args.first().map(String::as_str),
        Some("help" | "--help" | "-h")
    )
}

fn is_version(args: &[String]) -> bool {
    matches!(
        args.first().map(String::as_str),
        Some("version" | "--version" | "-V")
    )
}

fn is_guide_help(args: &[String]) -> bool {
    is_guide(args)
        && args
            .iter()
            .skip(1)
            .any(|arg| arg == "--help" || arg == "-h")
}
use super::provider_activation::{load_activation_for_language_message, provider_activation_path};
use super::provider_execution::provider_process_args;
use super::provider_selector::is_provider_owned_structural_selector_query;
