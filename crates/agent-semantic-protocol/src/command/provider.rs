//! Language provider command facade.

use super::document_provider;
use super::graph::GraphTurboReceiptRequest;
use agent_semantic_hook::{
    HookRuntime, default_activation_path, discover_activation_path, load_or_sync_activation,
    parse_hook_activation, runtime_profiles_for_runtime,
};
use agent_semantic_runtime::project_state_paths;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use super::client_backend_worker::run_client_backend_on_worker;
use super::protocol_version_line;
use super::provider_process::{
    provider_invocation_with_profile, provider_invocations, run_guide_command,
    run_owner_items_provider_command, run_provider_command,
};
use super::provider_roots::{
    activation_project_root, activation_storage_root, client_backend_cache_home,
    effective_project_root_and_args,
};
use super::query_direct_read::{
    is_asp_fast_direct_source_read, run_asp_fast_direct_source_read_command,
};
use super::query_owner::run_asp_fast_owner_query_command;
use super::search_config::AspConfig;
use super::search_pipe::{FastSearchContext, is_asp_fast_search, run_asp_fast_search_command};
use super::search_pipe_meta::run_asp_fast_search_meta_command;
use super::search_pipe_provider_facts::{ProviderGraphFactsContext, query_requests_semantic_facts};

const SUPPORTED_LANGUAGES: &[&str] = &[
    "rust",
    "typescript",
    "python",
    "julia",
    "gerbil-scheme",
    "org",
    "md",
];
const SUPPORTED_COMMANDS: &[&str] = &[
    "search",
    "query",
    "guide",
    "check",
    "cache",
    "info",
    "bench",
    "ast-patch",
    "evidence",
];

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

pub(crate) fn is_language_facade(language_id: &str) -> bool {
    SUPPORTED_LANGUAGES.contains(&language_id)
}

pub(crate) fn unsupported_language_facade_message(
    requested_facade: &str,
    command: Option<&str>,
    runtime: Option<&HookRuntime>,
) -> String {
    let known_facades = SUPPORTED_LANGUAGES.join("|");
    let active_facades = runtime
        .map(active_language_facades)
        .filter(|facades| !facades.is_empty());
    let suggested_facade = runtime.and_then(|runtime| {
        suggested_language_facade_for_request(requested_facade, &active_language_facades(runtime))
    });
    let mut lines = vec![
        format!("unsupported ASP language facade `{requested_facade}`."),
        "ASP facades are language IDs, not package or library names.".to_string(),
        format!("Known language facades: {known_facades}."),
    ];
    if let Some(active_facades) = active_facades.as_deref() {
        lines.push(format!("Active language facades: {active_facades}."));
    }
    if let Some(suggested_facade) = suggested_facade.as_deref() {
        lines.push(format!("Suggested matching facade: {suggested_facade}."));
    }
    lines.extend([String::new(), "## Run Next".to_string()]);
    if let Some(suggested_facade) = suggested_facade.as_deref() {
        let command = command.unwrap_or("guide");
        lines.push(format!("asp {suggested_facade} {command} ..."));
    } else {
        lines.extend([
            "asp providers".to_string(),
            "asp fd -query '<path-or-language-term>' '.'".to_string(),
            "asp rg -query '<feature-term>' '<bounded-scope>'".to_string(),
        ]);
    }
    lines.extend([
        String::new(),
        "## Rules".to_string(),
        "Only run `asp <language> search|query` when the facade is listed and matches the target language.".to_string(),
        "Do not switch to an unrelated active facade just because it is the only provider in this repository.".to_string(),
        "For unsupported target-language files, use provider-neutral finder commands or install/activate a matching provider.".to_string(),
    ]);
    lines.join("\n")
}

fn active_language_facades(runtime: &HookRuntime) -> String {
    runtime
        .providers
        .iter()
        .map(|provider| provider.language_id.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join("|")
}

fn suggested_language_facade_for_request(
    requested_facade: &str,
    active_facades: &str,
) -> Option<String> {
    if requested_facade.eq_ignore_ascii_case("effect")
        && active_facades
            .split('|')
            .any(|facade| facade == "typescript")
    {
        return Some("typescript".to_string());
    }
    None
}

fn load_activation_for_language_message() -> Option<HookRuntime> {
    let cwd = env::current_dir().ok()?;
    let activation_path =
        discover_activation_path(&cwd).unwrap_or_else(|| default_activation_path(&cwd));
    let text = fs::read_to_string(activation_path).ok()?;
    parse_hook_activation(&text).ok()
}

pub(crate) fn run_language_command(language_id: &str, args: &[String]) -> Result<(), String> {
    fn uses_client_backend(language_id: &str, args: &[String]) -> bool {
        (args.first().is_some_and(|command| command == "search")
            && args.get(1).is_none_or(|subcommand| subcommand != "guide"))
            || matches!(args.first().map(String::as_str), Some("check"))
            || matches!(args.first().map(String::as_str), Some("cache"))
            || (language_id != "gerbil-scheme"
                && args.first().is_some_and(|command| command == "query")
                && args.get(1).is_none_or(|subcommand| subcommand != "guide"))
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

    if document_provider::is_document_language(language_id) && is_help(&command_args) {
        return document_provider::run_language_command(language_id, &command_args);
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
    let discovered_activation_path = discover_activation_path(&invocation_root);
    if document_provider::is_document_language(language_id) {
        let activation_root = discovered_activation_path
            .as_deref()
            .and_then(|path| {
                load_activation(path)
                    .ok()
                    .map(|runtime| activation_project_root(path, &runtime.project_root))
            })
            .unwrap_or_else(|| invocation_root.clone());
        let config = AspConfig::load(&invocation_root, &activation_root);
        if !config.language_enabled(language_id) {
            return Err(format!("language `{language_id}` is disabled by asp.toml"));
        }
        if is_search_pipe_command(&command_args) {
            let cache_home = client_backend_cache_home(&activation_root, &activation_root)?;
            return run_asp_fast_search_command(
                &command_args,
                FastSearchContext {
                    language_id,
                    project_root: &activation_root,
                    locator_root: &invocation_root,
                    cache_home: &cache_home,
                    config: &config,
                    provider_context: None,
                    frontier_receipt: frontier_receipt.as_ref(),
                },
            );
        }
        return document_provider::run_language_command_with_config(
            language_id,
            &command_args,
            &config,
        );
    }
    validate_provider_command(&command_args)?;
    if is_guide_help(&command_args) {
        println!("{}", guide_usage(language_id));
        return Ok(());
    }
    if run_asp_fast_search_meta_command(language_id, &command_args) {
        return Ok(());
    }
    if let Some(result) =
        run_pre_activation_fast_owner_query(language_id, &command_args, &invocation_root)?
    {
        return result;
    }

    let activation_path =
        discovered_activation_path.unwrap_or_else(|| default_activation_path(&invocation_root));
    let runtime = load_activation(&activation_path)?;
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

    if run_asp_fast_owner_query_command(
        language_id,
        &provider_args,
        &project_root,
        &invocation_root,
    )? {
        return Ok(());
    }

    let cache_home = client_backend_cache_home(&activation_root, &project_root)?;
    let provider = runtime
        .providers
        .iter()
        .find(|provider| provider.language_id == language_id)
        .ok_or_else(|| format!("no activated provider for language {language_id}"))?;
    if is_asp_fast_search(&provider_args) {
        if fast_search_needs_provider_context(&provider_args, provider) {
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
            },
        );
    }
    if frontier_receipt.is_none()
        && language_id == "gerbil-scheme"
        && let Some(owner_path) = search_owner_items_owner_path(&provider_args)
    {
        let runtime_profiles = runtime_profiles_for_runtime(&project_root, &runtime);
        for invocation in provider_invocations(
            provider,
            &provider_args,
            &project_root,
            &runtime_profiles,
            &config,
        )? {
            run_owner_items_provider_command(
                language_id,
                provider,
                &invocation,
                &project_root,
                &cache_home,
                owner_path,
            )?;
        }
        return Ok(());
    }
    if frontier_receipt
        .as_ref()
        .is_some_and(GraphTurboReceiptRequest::has_extra_args)
    {
        return Err(
            "--frontier-receipt-* fact flags require an ASP graph-turbo fast search".to_string(),
        );
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
        let invocation =
            provider_invocation_with_profile(&runtime_profiles, provider, &guide_args, &config)?;
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
        &provider_args,
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
        )?;
    }
    Ok(())
}

fn search_owner_items_owner_path(args: &[String]) -> Option<&str> {
    if args.first().map(String::as_str) != Some("search")
        || args.get(1).map(String::as_str) != Some("owner")
        || args.get(3).map(String::as_str) != Some("items")
        || args.iter().any(|arg| arg == "--json")
    {
        return None;
    }
    args.get(2).map(String::as_str)
}

fn run_pre_activation_fast_owner_query(
    language_id: &str,
    command_args: &[String],
    invocation_root: &Path,
) -> Result<Option<Result<(), String>>, String> {
    if command_args.first().map(String::as_str) != Some("query") {
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
    if run_asp_fast_owner_query_command(
        language_id,
        &provider_args,
        &project_root,
        invocation_root,
    )? {
        return Ok(Some(Ok(())));
    }
    Ok(None)
}

const FRONTIER_RECEIPT_FACT_FLAGS: &[(&str, &str)] = &[
    ("--frontier-receipt-follow-node", "--follow-node"),
    ("--frontier-receipt-read-selector", "--read-selector"),
    ("--frontier-receipt-read-kind", "--read-kind"),
    ("--frontier-receipt-read-owner", "--read-owner"),
    ("--frontier-receipt-test-argv-json", "--test-argv-json"),
    ("--frontier-receipt-test-status", "--test-status"),
    ("--frontier-receipt-test-summary", "--test-summary"),
    ("--frontier-receipt-test-exit-code", "--test-exit-code"),
    ("--frontier-receipt-test-workdir", "--test-workdir"),
    ("--frontier-receipt-test-fingerprint", "--test-fingerprint"),
    (
        "--frontier-receipt-commands-to-first-useful-locator",
        "--commands-to-first-useful-locator",
    ),
    (
        "--frontier-receipt-commands-to-validation",
        "--commands-to-validation",
    ),
];

fn take_frontier_receipt_request(
    args: &mut Vec<String>,
) -> Result<Option<GraphTurboReceiptRequest>, String> {
    let mut normalized = Vec::with_capacity(args.len());
    let mut frontier_receipt_out = None;
    let mut receipt_args = Vec::new();
    let mut seen_fact_flags = Vec::<&'static str>::new();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == "--frontier-receipt-out" {
            if frontier_receipt_out.is_some() {
                return Err("--frontier-receipt-out may be passed only once".to_string());
            }
            let value = args
                .get(index + 1)
                .ok_or_else(|| "--frontier-receipt-out requires a path".to_string())?;
            frontier_receipt_out = Some(PathBuf::from(value));
            index += 2;
        } else if let Some(value) = arg.strip_prefix("--frontier-receipt-out=") {
            if frontier_receipt_out.is_some() {
                return Err("--frontier-receipt-out may be passed only once".to_string());
            }
            if value.is_empty() {
                return Err("--frontier-receipt-out requires a path".to_string());
            }
            frontier_receipt_out = Some(PathBuf::from(value));
            index += 1;
        } else if let Some((target_flag, value, public_flag)) =
            frontier_receipt_fact_arg(arg, args.get(index + 1).map(String::as_str))
        {
            if seen_fact_flags.contains(&public_flag) {
                return Err(format!("{public_flag} may be passed only once"));
            }
            if value.is_empty() {
                return Err(format!("{public_flag} requires a value"));
            }
            seen_fact_flags.push(public_flag);
            receipt_args.push(target_flag.to_string());
            receipt_args.push(value.to_string());
            if arg == public_flag {
                index += 2;
            } else {
                index += 1;
            }
        } else {
            normalized.push(arg.clone());
            index += 1;
        }
    }
    *args = normalized;
    let Some(out_path) = frontier_receipt_out else {
        if receipt_args.is_empty() {
            return Ok(None);
        }
        return Err("--frontier-receipt-* fact flags require --frontier-receipt-out".to_string());
    };
    Ok(Some(GraphTurboReceiptRequest::new(out_path, receipt_args)))
}

fn frontier_receipt_fact_arg<'a>(
    arg: &'a str,
    next: Option<&'a str>,
) -> Option<(&'static str, &'a str, &'static str)> {
    for (public_flag, target_flag) in FRONTIER_RECEIPT_FACT_FLAGS {
        if arg == *public_flag {
            return Some((*target_flag, next.unwrap_or(""), *public_flag));
        }
        let prefix = format!("{public_flag}=");
        if let Some(value) = arg.strip_prefix(&prefix) {
            return Some((*target_flag, value, *public_flag));
        }
    }
    None
}

fn load_activation(path: &Path) -> Result<HookRuntime, String> {
    load_or_sync_activation(path, &activation_storage_root(path))
}

fn validate_provider_command(args: &[String]) -> Result<(), String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(provider_usage());
    };
    let supported = if command == "agent" {
        args.get(1)
            .is_some_and(|subcommand| matches!(subcommand.as_str(), "doctor"))
    } else {
        SUPPORTED_COMMANDS.contains(&command)
    };
    if supported {
        Ok(())
    } else {
        Err(provider_usage())
    }
}

fn is_guide(args: &[String]) -> bool {
    args.first().is_some_and(|command| command == "guide")
}

fn provider_guide_args(language_id: &str, args: &[String]) -> Vec<String> {
    if matches!(language_id, "python" | "typescript") && is_guide(args) {
        let mut rewritten = vec!["agent".to_string(), "guide".to_string()];
        rewritten.extend(args.iter().skip(1).cloned());
        rewritten
    } else {
        args.to_vec()
    }
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

fn is_search_pipe_command(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("search"))
        && matches!(args.get(1).map(String::as_str), Some("pipe"))
}

fn fast_search_needs_provider_context(
    args: &[String],
    provider: &agent_semantic_hook::ActivatedProvider,
) -> bool {
    if matches!(args.get(1).map(String::as_str), Some("pipe" | "fzf")) {
        return provider.search_capabilities.semantic_facts
            && args
                .get(2)
                .is_some_and(|query| query_requests_semantic_facts(query));
    }
    if matches!(args.first().map(String::as_str), Some("search"))
        && matches!(args.get(1).map(String::as_str), Some("owner"))
        && matches!(args.get(3).map(String::as_str), Some("items"))
    {
        return provider.search_capabilities.owner_items;
    }
    if matches!(args.get(1).map(String::as_str), Some("ingest")) {
        return provider.search_capabilities.semantic_facts
            && provider_flag_value(args, "--query").is_some_and(query_requests_semantic_facts);
    }
    false
}

fn provider_flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|window| window.first().is_some_and(|value| value == flag))
        .and_then(|window| window.get(1))
        .map(String::as_str)
}

fn is_guide_help(args: &[String]) -> bool {
    is_guide(args)
        && args
            .iter()
            .skip(1)
            .any(|arg| arg == "--help" || arg == "-h")
}

fn provider_usage() -> String {
    format!(
        "usage: asp <{}> [--help|--version] <guide|search|query|check|cache|info|bench|agent doctor|ast-patch|evidence> ...",
        SUPPORTED_LANGUAGES.join("|")
    )
}

fn guide_usage(language_id: &str) -> String {
    format!(
        "usage: asp {language_id} guide [--help] [--workspace <root>]\n\nPrints the low-frequency provider-owned agent tool map.\nUse `asp {language_id} search guide --workspace .`, `asp {language_id} query guide --workspace .`, or `asp {language_id} query guide treesitter --workspace .` for focused reference guides."
    )
}
