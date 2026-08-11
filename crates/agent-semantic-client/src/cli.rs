//! CLI dispatcher for the public `asp` agent semantic client surface.

use std::env;
use std::path::PathBuf;

use agent_semantic_client_core::{ClientMethod, ProviderRegistrySnapshot};

use crate::cli_args::{ParsedArgs, parse_client_args};
use crate::provider_method::run_provider_method;

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
        Some("wrap") => crate::tools_cli::run_wrap(&parsed.forwarded_args),
        Some("providers") => run_providers(parsed),
        Some("doctor") => run_doctor(parsed),
        Some("cache") => {
            crate::cache_cli::run_cache(
                &parsed.project_root,
                language_id.as_ref(),
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
            run_provider_method(
                parsed,
                ClientMethod::Search,
                language_id.ok_or_else(|| provider_language_required("search"))?,
            )
            .await
        }
        Some("query") => {
            run_provider_method(
                parsed,
                ClientMethod::Query,
                language_id.ok_or_else(|| provider_language_required("query"))?,
            )
            .await
        }
        Some("check") => {
            run_provider_method(
                parsed,
                ClientMethod::Check,
                language_id.ok_or_else(|| provider_language_required("check"))?,
            )
            .await
        }
        Some(command) => Err(format!("unknown client command: {command}")),
    }
}
fn provider_language_required(command: &str) -> String {
    format!(
        "asp {command} requires a language facade; use asp <language> {command} ...; run asp providers for active facades"
    )
}

fn provider_contract_status(
    manifest: &agent_semantic_hook::ProviderManifest,
) -> (&'static str, Vec<String>) {
    let errors = agent_semantic_hook::validate_provider_manifest_contract(manifest);
    let status = if errors.is_empty() {
        "valid"
    } else {
        "invalid"
    };
    (status, errors)
}

fn run_providers(parsed: ParsedArgs) -> Result<(), String> {
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
    let manifests = agent_semantic_hook::builtin_provider_manifests();
    let activation = ProviderRegistrySnapshot::load(&parsed.activation_root);
    let activation_evidence =
        |language_id: &agent_semantic_client_core::LanguageId,
         provider_id: &agent_semantic_client_core::ProviderId| match &activation {
            Ok(snapshot) => {
                let provider = snapshot.providers.iter().find(|provider| {
                    &provider.language_id == language_id && &provider.provider_id == provider_id
                });
                serde_json::json!({
                    "status": if provider.is_some() { "activated" } else { "not-activated" },
                    "activationPath": snapshot.activation_path,
                    "provider": provider.map(|provider| serde_json::json!({
                        "manifestId": provider.manifest_id,
                        "manifestDigest": provider.manifest_digest,
                        "binary": provider.binary,
                        "execution": provider.execution.as_str(),
                    })),
                })
            }
            Err(error) => serde_json::json!({
                "status": "unavailable",
                "reasonKind": "provider-registry-unavailable",
                "message": error,
            }),
        };

    if let Some(language_id) = requested_language {
        let manifest = manifests
            .iter()
            .find(|manifest| manifest.language_id().as_str() == language_id)
            .ok_or_else(|| format!("no builtin provider manifest for language `{language_id}`"))?;
        let query_pack_descriptor = manifest.query_pack_descriptor();
        let (contract_status, contract_errors) = provider_contract_status(manifest);
        let manifest_digest = agent_semantic_hook::provider_manifest_digest(manifest)
            .map_err(|error| format!("digest builtin provider manifest: {error}"))?;
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "manifest": manifest,
                "manifestDigest": manifest_digest,
                "queryPackDescriptor": query_pack_descriptor,
                    "searchCapabilities": manifest.search_capabilities(),
                    "semanticFactsDescriptor": manifest.semantic_facts_descriptor(),
                "contractStatus": contract_status,
                "contractErrors": contract_errors,
                "activation": activation_evidence(manifest.language_id(), manifest.provider_id()),
            }))
            .map_err(|error| format!("serialize provider manifest evidence: {error}"))?
        );
    } else {
        let providers = manifests
            .iter()
            .map(|manifest| {
                let query_pack_descriptor = manifest.query_pack_descriptor();
                let (contract_status, contract_errors) = provider_contract_status(manifest);
                let manifest_digest = agent_semantic_hook::provider_manifest_digest(manifest)
                    .map_err(|error| format!("digest builtin provider manifest: {error}"))?;
                Ok(serde_json::json!({
                    "manifest": manifest,
                    "manifestDigest": manifest_digest,
                    "queryPackDescriptor": query_pack_descriptor,
                    "searchCapabilities": manifest.search_capabilities(),
                    "semanticFactsDescriptor": manifest.semantic_facts_descriptor(),
                    "contractStatus": contract_status,
                    "contractErrors": contract_errors,
                    "activation": activation_evidence(
                        manifest.language_id(),
                        manifest.provider_id(),
                    ),
                }))
            })
            .collect::<Result<Vec<_>, String>>()?;
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({ "providers": providers }))
                .map_err(|error| format!("serialize provider registry evidence: {error}"))?
        );
    }
    Ok(())
}

fn run_doctor(parsed: ParsedArgs) -> Result<(), String> {
    validate_doctor_args(&parsed)?;
    match ProviderRegistrySnapshot::load(&parsed.activation_root) {
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
            println!("|cmd install=asp install plugin --codex .");
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
