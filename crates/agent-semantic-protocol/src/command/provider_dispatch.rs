//! Language provider command facade.

use super::provider_execution::take_frontier_receipt_request;
use super::provider_usage;

use super::document_language_facade;
use super::graph::GraphTurboReceiptRequest;
use agent_semantic_hook::{
    runtime_profiles_for_runtime,
};
use agent_semantic_runtime::project_state_paths;
use std::env;
use std::path::Path;

use super::client_backend_worker::run_client_backend_on_worker;
use super::gerbil_check_cache::try_replay_gerbil_check_cache;
use super::gerbil_deps::try_run_gerbil_deps_index_command;
use super::protocol_version_line;
use super::provider_fast_path::{
    run_activated_owner_language_preflight, run_pre_activation_dynamic_rust_owner_items_search,
    run_pre_activation_search_command_preflight, search_owner_items_owner_path,
};
use super::provider_fast_search::fast_search_needs_provider_context;
use super::provider_process::{
    provider_invocation_with_profile, provider_invocations, run_guide_command, run_provider_command,
};
use super::provider_roots::{
    activation_project_root, client_backend_cache_home, effective_project_root_and_args,
    validate_explicit_workspace_project_root,
};
pub(crate) use super::provider_selector::{
    is_language_facade, unsupported_language_facade_message,
};
use super::query_direct_read::{
    is_asp_fast_direct_source_read, run_asp_fast_direct_source_read_command,
};
use super::query_owner::run_asp_fast_owner_query_command;
use super::search_config::AspConfig;
use super::search_dependency_seed::{
    is_search_dependency_seed, run_search_dependency_seed_command,
};
use super::search_pipe::{FastSearchContext, is_asp_fast_search, run_asp_fast_search_command};
use super::search_pipe_meta::run_asp_fast_search_meta_command;
use super::search_pipe_provider_facts::ProviderGraphFactsContext;
use provider_usage::{
    guide_usage, is_guide, provider_guide_args, provider_usage, validate_provider_command,
};

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

pub(crate) fn run_language_command(language_id: &str, args: &[String]) -> Result<(), String> {
    fn uses_client_backend(language_id: &str, args: &[String]) -> bool {
        (args.first().is_some_and(|command| command == "search")
            && args.get(1).is_none_or(|subcommand| subcommand != "guide"))
            || matches!(args.first().map(String::as_str), Some("check"))
            || matches!(args.first().map(String::as_str), Some("cache"))
            || (language_id != "gerbil-scheme"
                && !document_language_facade::is_document_language(language_id)
                && args.first().is_some_and(|command| command == "query")
                && args.get(1).is_none_or(|subcommand| subcommand != "guide")
                && !is_provider_owned_structural_code_query(language_id, args))
    }

    fn is_document_owner_items_search(language_id: &str, args: &[String]) -> bool {
        document_language_facade::is_document_language(language_id)
            && is_asp_fast_search(args)
            && search_owner_items_owner_path(args).is_some()
    }

    fn provider_invokes_asp_facade(
        language_id: &str,
        provider: &agent_semantic_hook::ActivatedProvider,
        config: &AspConfig,
    ) -> bool {
        let binary = config
            .provider_bin(language_id)
            .unwrap_or(provider.binary.as_str());
        Path::new(binary)
            .file_name()
            .is_some_and(|name| name.to_string_lossy() == "asp")
    }

    fn run_client_backend_command(
        language_id: &str,
        args: &[String],
        project_root: &Path,
        activation_path: &Path,
        cache_home: &Path,
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
        let previous_prj_cache_home = env::var_os("PRJ_CACHE_HOME");
        let previous_activation_path = env::var_os("ASP_PROVIDER_ACTIVATION_PATH");
        let previous_activation_refresh = env::var_os("ASP_PROVIDER_ACTIVATION_REFRESH");
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
            env::set_var("PRJ_CACHE_HOME", cache_home);
            env::set_var("ASP_PROVIDER_ACTIVATION_PATH", activation_path);
            env::set_var("ASP_PROVIDER_ACTIVATION_REFRESH", "0");
            env::set_var("ASP_RUNTIME_BIN_DIR", &runtime_bin);
            env::set_var("SEMANTIC_AGENT_PROTOCOL_BIN", &protocol_bin);
            if let Some(path) = runtime_path.as_deref() {
                env::set_var("PATH", path);
            }
        }
        let result =
            run_client_backend_on_worker(language_id, client_args, project_root.to_path_buf());
        restore_env_var!("PRJ_CACHE_HOME", previous_prj_cache_home);
        restore_env_var!("ASP_PROVIDER_ACTIVATION_PATH", previous_activation_path);
        restore_env_var!(
            "ASP_PROVIDER_ACTIVATION_REFRESH",
            previous_activation_refresh
        );
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
    let frontier_receipt = take_frontier_receipt_request(&mut command_args)?;
    if frontier_receipt.is_some()
        && command_args
            .first()
            .is_none_or(|command| command != "search")
    {
        return Err("--frontier-receipt-out is supported only for search commands".to_string());
    }

    if document_language_facade::is_document_language(language_id) && is_help(&command_args) {
        return document_language_facade::run_document_language_help(language_id, &command_args);
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
    let document_owner_items_search = is_document_owner_items_search(language_id, &command_args);
    validate_provider_command(&command_args)?;
    if is_guide_help(&command_args) {
        println!("{}", guide_usage(language_id));
        return Ok(());
    }
    if try_run_gerbil_deps_index_command(language_id, &command_args)? {
        return Ok(());
    }
    if run_asp_fast_search_meta_command(language_id, &command_args) {
        return Ok(());
    }
    if let Some(result) = run_pre_activation_dynamic_rust_owner_items_search(
        language_id,
        &command_args,
        &invocation_root,
    )? {
        return result;
    }
    run_pre_activation_search_command_preflight(language_id, &command_args, &invocation_root)?;
    reject_search_file_workspace(&command_args, &invocation_root)?;
    validate_explicit_workspace_project_root(language_id, &command_args, &invocation_root)?;
    reject_manifest_source_selector_query_code(language_id, &command_args)?;
    let activation_path = provider_activation_path(&invocation_root);
    let runtime = load_activation(&activation_path, &invocation_root)?;
    let activation_root = activation_project_root(&activation_path, &runtime.project_root);
    let config = AspConfig::load(&invocation_root, &activation_root);
    let (project_root, provider_args) = effective_project_root_and_args(
        language_id,
        &command_args,
        &invocation_root,
        &activation_root,
    )?;

    if !config.language_enabled(language_id) {
        return Err(format!("language `{language_id}` is disabled by asp.toml"));
    }

    if is_asp_fast_direct_source_read(&provider_args) {
        return run_asp_fast_direct_source_read_command(
            &provider_args,
            &project_root,
            &invocation_root,
        );
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
    reject_registered_source_selector_query(language_id, &command_args, provider)?;

    if !is_provider_owned_structural_code_query(language_id, &provider_args)
        && run_asp_fast_owner_query_command(
            language_id,
            &provider_args,
            &project_root,
            &invocation_root,
        )?
    {
        return Ok(());
    }

    let cache_home = client_backend_cache_home(&activation_root, &project_root)?;
    if let Some(request) =
        super::language_projection_import::LanguageProjectionImportRequest::parse(&provider_args)?
    {
        let runtime_profiles = runtime_profiles_for_runtime(&project_root, &runtime);
        let invocation = provider_invocation_with_profile(
            &runtime_profiles,
            provider,
            &request.provider_args(&project_root),
            &project_root,
            &config,
        )?;
        let output = super::provider_process::run_provider_command_with_stdin(
            language_id,
            provider,
            &invocation,
            &project_root,
            &cache_home,
            Vec::new(),
        )?;
        if !output.status.success() {
            return Err(request.provider_failure(output.status.code(), output.stderr.as_ref()));
        }
        return request.import_output(language_id, &project_root, output.stdout.as_ref());
    }
    if is_search_dependency_seed(&provider_args) {
        if !provider.search_capabilities.dependency_topology {
            return run_search_dependency_seed_command(
                language_id,
                &provider_args,
                &project_root,
                &cache_home,
                &config,
                None,
            );
        }
        let runtime_profiles = runtime_profiles_for_runtime(&project_root, &runtime);
        let provider_context = ProviderGraphFactsContext {
            provider,
            profiles: &runtime_profiles,
            cache_home: &cache_home,
        };
        return run_search_dependency_seed_command(
            language_id,
            &provider_args,
            &project_root,
            &cache_home,
            &config,
            Some(&provider_context),
        );
    }
    if is_asp_fast_search(&provider_args) {
        let current_snapshot =
            agent_semantic_client::source_index::current_source_index_snapshot(&project_root)?;
        let provider_context_allowed = (!document_owner_items_search
            || !provider_invokes_asp_facade(language_id, provider, &config))
            && !(language_id == "gerbil-scheme"
                && search_owner_items_owner_path(&provider_args).is_some());
        if provider_context_allowed && fast_search_needs_provider_context(&provider_args, provider)?
        {
            let runtime_profiles = runtime_profiles_for_runtime(&project_root, &runtime);
            let provider_context = ProviderGraphFactsContext {
                provider,
                profiles: &runtime_profiles,
                cache_home: &cache_home,
            };
            return run_asp_fast_search_command(
                &provider_args,
                FastSearchContext {
                    language_id,
                    project_root: &project_root,
                    locator_root: &invocation_root,
                    cache_home: &cache_home,
                    config: &config,
                    provider_context: Some(&provider_context),
                    frontier_receipt: frontier_receipt.as_ref(),
                    source_snapshot: &current_snapshot.source_snapshot,
                },
            );
        }
        return run_asp_fast_search_command(
            &provider_args,
            FastSearchContext {
                language_id,
                project_root: &project_root,
                locator_root: &invocation_root,
                cache_home: &cache_home,
                config: &config,
                provider_context: None,
                frontier_receipt: frontier_receipt.as_ref(),
                source_snapshot: &current_snapshot.source_snapshot,
            },
        );
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
    if uses_client_backend(language_id, &command_args) {
        return run_client_backend_command(
            language_id,
            &provider_args,
            &project_root,
            &activation_path,
            &cache_home,
            frontier_receipt.as_ref(),
        );
    }

    let runtime_profiles = runtime_profiles_for_runtime(&project_root, &runtime);
    if is_guide(&command_args) {
        let guide_args = provider_guide_args(language_id, &provider_args);
        let invocation = provider_invocation_with_profile(
            &runtime_profiles,
            provider,
            &guide_args,
            &project_root,
            &config,
        )?;
        return run_guide_command(
            language_id,
            provider,
            &invocation,
            &project_root,
            &cache_home,
        );
    }
    for invocation in provider_invocations(
        provider,
        &provider_process_args(&provider_args),
        &project_root,
        &runtime_profiles,
        &config,
    )? {
        run_provider_command(
            language_id,
            provider,
            &invocation,
            &project_root,
            &cache_home,
            document_language_facade::is_document_language(language_id)
                && command_args
                    .first()
                    .is_some_and(|command| command == "query")
                && command_args.iter().any(|arg| arg == "--json"),
        )?;
    }
    Ok(())
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
use super::provider_activation::{
    load_activation, load_activation_for_language_message, provider_activation_path,
};
use super::provider_execution::provider_process_args;
use super::provider_selector::{
    is_provider_owned_structural_code_query,
    reject_manifest_source_selector_query_code, reject_registered_source_selector_query,
    reject_search_file_workspace,
};
