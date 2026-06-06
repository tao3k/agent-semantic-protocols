//! CLI dispatcher for the public `asp` agent semantic client surface.

use std::env;
use std::path::PathBuf;

use agent_semantic_client_core::{ClientMethod, ProviderRegistrySnapshot};

use crate::cli_args::{ParsedArgs, parse_client_args};
use crate::provider_method::run_provider_method;

/// Runs the agent semantic client CLI from process arguments.
pub fn run_cli_from_env() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if matches!(args.first().map(String::as_str), Some("query" | "check")) {
        return Err(
            "top-level asp query/check has been removed; use asp <rust|typescript|python|julia> <query|check> ..."
                .to_string(),
        );
    }
    let cwd = env::current_dir()
        .map_err(|error| format!("failed to resolve current directory: {error}"))?;
    run_cli_args(None, args, cwd)
}

/// Runs the agent semantic client CLI with an optional facade language.
pub fn run_cli_args(
    language_id: Option<agent_semantic_client_core::LanguageId>,
    args: Vec<String>,
    cwd: PathBuf,
) -> Result<(), String> {
    let language_id_text = language_id.as_ref().map(ToString::to_string);
    let parsed = parse_client_args(args, cwd, language_id_text.as_deref())?;
    match parsed.command.as_deref() {
        None | Some("help" | "--help" | "-h") => {
            print_guide();
            Ok(())
        }
        Some("guide") => run_guide(parsed, language_id),
        Some("tools") => crate::tools_cli::run_tools(&parsed.project_root, &parsed.forwarded_args),
        Some("providers") => run_providers(parsed),
        Some("doctor") => run_doctor(parsed),
        Some("cache") => crate::cache_cli::run_cache(
            &parsed.project_root,
            &parsed.forwarded_args,
            parsed.receipt_json,
        ),
        Some("cloud") => run_cloud(parsed),
        Some("search") => {
            if language_id.is_none()
                && parsed
                    .forwarded_args
                    .first()
                    .is_some_and(|arg| arg == "history")
            {
                return crate::search_history::run_search_history(
                    &parsed.project_root,
                    &parsed.forwarded_args,
                );
            }
            run_provider_method(
                parsed,
                ClientMethod::Search,
                language_id.ok_or_else(|| provider_language_required("search"))?,
            )
        }
        Some("query") => run_provider_method(
            parsed,
            ClientMethod::Query,
            language_id.ok_or_else(|| provider_language_required("query"))?,
        ),
        Some("check") => run_provider_method(
            parsed,
            ClientMethod::Check,
            language_id.ok_or_else(|| provider_language_required("check"))?,
        ),
        Some(command) => Err(format!("unknown client command: {command}")),
    }
}
fn run_guide(
    parsed: ParsedArgs,
    language_id: Option<agent_semantic_client_core::LanguageId>,
) -> Result<(), String> {
    let Some(language_id) = language_id else {
        print_guide();
        return Ok(());
    };
    run_provider_method(parsed, ClientMethod::Guide, language_id)
}

fn provider_language_required(command: &str) -> String {
    format!(
        "asp {command} requires a language facade; use asp <rust|typescript|python|julia> {command} ..."
    )
}

fn run_providers(parsed: ParsedArgs) -> Result<(), String> {
    match ProviderRegistrySnapshot::load(&parsed.project_root) {
        Ok(snapshot) => {
            println!(
                "[asp-providers] activation={} providers={}",
                snapshot.activation_path.display(),
                snapshot.providers.len()
            );
            for provider in snapshot.providers {
                println!(
                    "|provider language={} provider={} binary={} execution={} packageRoots={}",
                    provider.language_id,
                    provider.provider_id,
                    provider.binary,
                    provider.execution.as_str(),
                    provider.package_roots.join(",")
                );
            }
        }
        Err(error) => {
            println!("[asp-providers] activation=missing providers=0");
            println!("|reason provider-activation-unavailable");
            println!("|cmd install=asp hook install --client codex .");
            println!("|cmd guide=asp guide");
            eprintln!("[asp-providers] activation unavailable: {error}");
        }
    }
    Ok(())
}

fn run_doctor(parsed: ParsedArgs) -> Result<(), String> {
    match ProviderRegistrySnapshot::load(&parsed.project_root) {
        Ok(snapshot) => println!(
            "[asp-doctor] status=ok backend=local activation={} providers={} server=not-required",
            snapshot.activation_path.display(),
            snapshot.providers.len()
        ),
        Err(error) => {
            println!(
                "[asp-doctor] status=degraded backend=local activation=missing providers=0 server=not-required"
            );
            println!("|reason provider-activation-unavailable");
            println!("|cmd install=asp hook install --client codex .");
            println!("|cmd guide=asp guide");
            eprintln!("[asp-doctor] activation unavailable: {error}");
        }
    }
    println!(
        "|cache status=inspectable route=local-cache import=manual invalidate=manual replay=artifact-only"
    );
    println!("{}", crate::tools_cli::tools_summary_line());
    println!("|cloud status=disabled reason=local-default privateServer=optional");
    Ok(())
}

fn run_cloud(parsed: ParsedArgs) -> Result<(), String> {
    match parsed.forwarded_args.as_slice() {
        [subcommand] if subcommand == "status" => {
            println!(
                "[asp-cloud] status=disabled backend=local privateServer=optional uploadPolicy=none"
            );
            Ok(())
        }
        _ => Err("usage: asp cloud status".to_string()),
    }
}

fn print_guide() {
    println!("[asp-guide] backend=local prompt=compact json=artifact-only");
    println!("|cmd doctor=asp doctor");
    println!("|cmd providers=asp providers");
    println!("|cmd tools-doctor=asp tools doctor");
    println!("|cmd search-history=asp search history audit .");
    println!("|cmd guide=asp <rust|typescript|python> guide .");
    println!("|cmd search-guide=asp <rust|typescript|python> search guide .");
    println!("|ref query-guide=asp <rust|typescript|python> query guide .");
    println!("|ref treesitter-query-guide=asp <rust|typescript|python> query guide treesitter .");
    println!("|cmd search=asp <rust|typescript|python> search <provider-search-args>");
    println!("|cmd query=asp <rust|typescript|python> query <provider-query-args>");
    println!("|cmd check=asp <rust|typescript|python> check <provider-check-args>");
    println!("|cmd cache=asp cache status");
    println!("|cmd cache-import=asp cache import");
    println!("|cmd cache-invalidate=asp cache invalidate");
    println!("|cmd cloud=asp cloud status");
    println!(
        "|rule route=local-native cache=probe-first cloud=optional nativeProviderFacts=required"
    );
}
