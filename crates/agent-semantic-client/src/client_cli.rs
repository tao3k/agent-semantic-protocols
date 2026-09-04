//! CLI dispatcher for the public `asp` agent semantic client surface.

use std::env;
use std::path::PathBuf;

use crate::cli_args::ParsedArgs;
use crate::cli_args::parse_client_args;

/// Runs the agent semantic client CLI from process arguments.
pub async fn run_cli_from_env() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if matches!(args.first().map(String::as_str), Some("query" | "check")) {
        return Err(
            "top-level asp query/check has been removed; use asp <rust|typescript|python|julia> <query|check> ..."
                .to_string(),
        );
    }
    let cwd = env::current_dir()
        .map_err(|error| format!("failed to resolve current directory: {error}"))?;
    run_cli_args(None, args, cwd).await
}

/// Runs the agent semantic client CLI with an optional facade language.
pub async fn run_cli_args(
    language_id: Option<agent_semantic_client_core::LanguageId>,
    args: Vec<String>,
    cwd: PathBuf,
) -> Result<(), String> {
    let language_id_text = language_id.as_ref().map(ToString::to_string);
    let parsed = parse_client_args(args, cwd, language_id_text.as_deref())?;
    match parsed.command.as_deref() {
        None | Some("help" | "--help" | "-h") => {
            print_help();
            Ok(())
        }
        Some("tools") => crate::tools_cli::run_tools(&parsed.project_root, &parsed.forwarded_args),
        Some("providers") => run_providers(parsed).await,
        Some("doctor") => run_doctor(parsed).await,
        Some("cache") => {
            crate::cache_cli::run_cache(
                &parsed.project_root,
                language_id.as_ref(),
                &parsed.forwarded_args,
                parsed.receipt_json,
            )
            .await
        }
        Some("clean") => {
            crate::cache_cli::run_project_registry_clean(
                &parsed.project_root,
                &parsed.forwarded_args,
                parsed.receipt_json,
            )
            .await
        }
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
                )
                .await;
            }
            Err(
                "provider search is Runtime Server-owned; use the ASP language facade ProviderSearch operation"
                    .to_owned(),
            )
        }
        Some("query" | "check") => Err(
            "provider query/check is Runtime Server route-owned; direct client provider execution has been removed"
                .to_owned(),
        ),
        Some(command) => Err(format!("unknown client command: {command}")),
    }
}
async fn run_providers(parsed: ParsedArgs) -> Result<(), String> {
    let requested_language = match parsed.forwarded_args.as_slice() {
        [command] if command == "list" => None,
        [command, language_id] if command == "get" => Some(language_id.as_str()),
        _ => {
            return Err(
                "usage: asp providers <list|get <language-id>>; provider evidence requires an explicit operation"
                    .to_string(),
            );
        }
    };
    let state_home = agent_semantic_runtime::state_core::resolve_state_home()?;
    let endpoint = agent_semantic_client_db::read_runtime_server_endpoint(&state_home)
        .await?
        .ok_or_else(|| "runtime-server-endpoint-unavailable".to_owned())?;
    let request = agent_semantic_provider_protocol::ProviderRegisterRequest {
        schema_id: agent_semantic_provider_protocol::PROVIDER_REGISTER_REQUEST_SCHEMA_ID.to_owned(),
        schema_version: agent_semantic_provider_protocol::PROVIDER_REGISTER_SCHEMA_VERSION
            .to_owned(),
        expected_generation: None,
        request: agent_semantic_provider_protocol::ProviderRegisterOperation::List,
    };
    let response =
        agent_semantic_provider_transport::grpc_session::call_runtime_provider_register_tcp(
            endpoint.provider_endpoint.socket_addr(),
            &request,
        )
        .await?;
    let agent_semantic_provider_protocol::ProviderRegisterResult::Snapshot { snapshot } =
        response.result
    else {
        return Err("Runtime Server rejected read-only provider register snapshot".to_owned());
    };
    let generation = snapshot.generation;
    let digest = snapshot.digest;
    let providers = match requested_language {
        Some(language_id) => snapshot
            .providers
            .into_iter()
            .filter(|provider| provider.language_id == language_id)
            .collect::<Vec<_>>(),
        None => snapshot.providers,
    };
    if let Some(language_id) = requested_language
        && providers.is_empty()
    {
        return Err(format!(
            "no Runtime Server provider registration for language `{language_id}`"
        ));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "generation": generation,
            "digest": digest,
            "providers": providers,
        }))
        .map_err(|error| format!("serialize provider register snapshot: {error}"))?
    );
    Ok(())
}

async fn run_doctor(parsed: ParsedArgs) -> Result<(), String> {
    validate_doctor_args(&parsed)?;
    let state_home = agent_semantic_runtime::state_core::resolve_state_home()?;
    match agent_semantic_client_db::runtime_server_health::
        cached_runtime_server_health_for_state_home(&state_home)
        .await
    {
        Ok(health) => println!(
            "[asp-doctor] status={} backend=runtime-server state={:?} elapsedMicros={}",
            if health.is_healthy() {
                "ok"
            } else {
                "degraded"
            },
            health.resident.state,
            health.elapsed_micros,
        ),
        Err(error) => {
            println!("[asp-doctor] status=degraded backend=runtime-server");
            println!("|reason runtime-server-cached-health-unavailable");
            eprintln!("[asp-doctor] Runtime Server cached health unavailable: {error}");
        }
    }
    println!("|cache status=inspectable authority=runtime-server mutations=explicit-only");
    println!("{}", crate::tools_cli::tools_summary_line());
    println!("|cloud status=disabled reason=local-default privateServer=optional");
    Ok(())
}

const DOCTOR_USAGE: &str = "usage: asp doctor\nrooted health: asp tools doctor [PROJECT_ROOT]";

fn validate_doctor_args(parsed: &ParsedArgs) -> Result<(), String> {
    if parsed.forwarded_args.is_empty()
        && !parsed.receipt_json
        && parsed.frontier_receipt_out.is_none()
    {
        Ok(())
    } else {
        Err(DOCTOR_USAGE.to_string())
    }
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

fn print_help() {
    println!("usage: asp <command> [options]");
}
