//! Language provider command facade.

use super::provider_execution::take_frontier_receipt_request;
use super::provider_usage;

use std::env;
use std::path::Path;

use agent_semantic_client::projection_presentation::{
    ProjectionPresentation, render_exact_projection_response,
};

use super::gerbil_deps::try_run_gerbil_deps_index_command;
use super::protocol_version_line;
pub(crate) use super::provider_selector::{
    is_language_facade, unsupported_language_facade_message,
};
use provider_usage::{guide_usage, is_guide, provider_usage};

/// Observational target only; it never controls or cancels search execution.
const SEARCH_DIAGNOSTIC_SLOW_TARGET_MICROS: u64 = 500_000;

pub(crate) async fn run_language_command(
    language_id: &str,
    args: &[String],
    process_started: tokio::time::Instant,
) -> Result<(), String> {
    fn uses_client_backend(args: &[String]) -> bool {
        (args.first().is_some_and(|command| command == "search")
            && args.get(1).is_none_or(|subcommand| subcommand != "guide"))
            || matches!(args.first().map(String::as_str), Some("query" | "cache"))
    }

    async fn dispatch_asp_client_command(
        language_id: &str,
        route: &str,
        intent: serde_json::Value,
        project_root: &Path,
        presentation: ProjectionPresentation,
    ) -> Result<(), String> {
        let client = agent_semantic_client::AspClient::new(
            crate::server::runtime_server::state_home()?,
            project_root,
        );
        let response = client.dispatch(language_id, route, intent).await?;
        let rendered = if route == "query" {
            render_exact_projection_response(&response, presentation)?
        } else {
            serde_json::to_string(&response)
                .map_err(|error| format!("encode route response: {error}"))?
        };
        println!("{rendered}");
        Ok(())
    }

    if !is_language_facade(language_id) {
        return Err(unsupported_language_facade_message(
            language_id,
            args.first().map(String::as_str),
            None,
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
    if is_guide_help(&command_args) {
        println!("{}", guide_usage(language_id));
        return Ok(());
    }
    if try_run_gerbil_deps_index_command(language_id, &command_args)? {
        return Ok(());
    }
    if is_provider_owned_structural_selector_query(language_id, &command_args) {
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
        let intent = runtime_query_intent(&exact_provider_args)?;
        let presentation = runtime_query_presentation(&exact_provider_args);
        return dispatch_asp_client_command(
            language_id,
            "query",
            intent,
            &exact_project_root,
            presentation,
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
        let intent = runtime_owner_intent(&owner_args)?;
        return dispatch_asp_client_command(
            language_id,
            "search.owner",
            intent,
            &owner_project_root,
            ProjectionPresentation::MachineJson,
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
        if provider_args
            .first()
            .is_some_and(|command| command == "search")
        {
            let intent = runtime_search_intent(&provider_args)?;
            return dispatch_asp_client_command(
                language_id,
                "search",
                intent,
                &project_root,
                ProjectionPresentation::MachineJson,
            )
            .await;
        }
        return Err(format!(
            "provider command must be admitted as a Runtime Server route: languageId={language_id} command={}",
            provider_args.first().map(String::as_str).unwrap_or("<empty>")
        ));
    }
    Err(format!(
        "provider command must be admitted as a Runtime Server route: languageId={language_id} command={}",
        command_args.first().map(String::as_str).unwrap_or("<empty>")
    ))
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

fn is_search_owner_items_query(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("search"))
        && matches!(args.get(1).map(String::as_str), Some("owner"))
        && matches!(args.get(3).map(String::as_str), Some("items"))
        && !args.iter().any(|arg| arg == "--json")
}

fn option_value(args: &[String], option: &str) -> Result<Option<String>, String> {
    let Some(index) = args.iter().position(|arg| arg == option) else {
        return Ok(None);
    };
    args.get(index + 1)
        .filter(|value| !value.starts_with('-'))
        .cloned()
        .map(Some)
        .ok_or_else(|| format!("{option} requires a value"))
}

fn runtime_query_intent(args: &[String]) -> Result<serde_json::Value, String> {
    let selector = super::provider_selector::exact_query_selector_argument(args)
        .ok_or_else(|| "query requires a canonical selector".to_owned())?;
    let projection = option_value(args, "--projection")?.unwrap_or_else(|| "source".to_owned());
    Ok(serde_json::json!({
        "schemaId": "agent.semantic-protocols.asp-client-exact-query-request",
        "schemaVersion": "1",
        "selector": selector,
        "projection": projection,
    }))
}

fn runtime_query_presentation(args: &[String]) -> ProjectionPresentation {
    if args.iter().any(|arg| arg == "--json") {
        ProjectionPresentation::MachineJson
    } else {
        ProjectionPresentation::Text
    }
}

fn runtime_owner_intent(args: &[String]) -> Result<serde_json::Value, String> {
    let owner_path = args
        .get(2)
        .filter(|value| !value.starts_with('-'))
        .cloned()
        .ok_or_else(|| "search owner requires an owner path".to_owned())?;
    Ok(serde_json::json!({
        "schemaId": "agent.semantic-protocols.asp-client-owner-search-request",
        "schemaVersion": "1",
        "ownerPath": owner_path,
        "query": option_value(args, "--query")?.unwrap_or_default(),
        "view": option_value(args, "--view")?.unwrap_or_else(|| "seeds".to_owned()),
    }))
}

fn runtime_search_intent(args: &[String]) -> Result<serde_json::Value, String> {
    let operation = args
        .get(1)
        .filter(|value| !value.starts_with('-'))
        .cloned()
        .ok_or_else(|| "search requires an operation".to_owned())?;
    let mut queries = Vec::new();
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--query" => {
                let value = args
                    .get(index + 1)
                    .filter(|value| !value.starts_with('-'))
                    .ok_or_else(|| "--query requires a value".to_owned())?;
                queries.push(value.clone());
                index += 2;
            }
            "--workspace" | "--view" | "--query-set" | "--owner" | "--from-hook"
            | "--projection" | "--selector" | "--context" => index += 2,
            option if option.starts_with('-') => index += 1,
            _value if index == 1 => index += 1,
            value => {
                queries.push(value.to_owned());
                index += 1;
            }
        }
    }
    Ok(serde_json::json!({
        "schemaId": "agent.semantic-protocols.asp-client-search-request",
        "schemaVersion": "1",
        "operation": operation,
        "query": queries.join(" "),
    }))
}
use super::provider_selector::is_provider_owned_structural_selector_query;

#[cfg(test)]
#[path = "../../tests/unit/command/provider_route_intent.rs"]
mod tests;
