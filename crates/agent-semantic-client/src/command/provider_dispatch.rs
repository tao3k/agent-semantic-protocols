//! Language provider command facade.

use super::provider_usage;

use agent_semantic_client::{
    LanguageCommandApplication, LanguageCommandOperation, LanguageCommandRequest,
};
use agent_semantic_client_protocol::{AspClientExactQueryRequest, AspClientSearchRequest};
use std::env;

use super::protocol_version_line;
pub(crate) use super::provider_selector::{
    is_language_facade, unsupported_language_facade_message,
};
use provider_usage::{guide_usage, is_guide, provider_usage};

/// Observational target only; it never controls or cancels search execution.
const SEARCH_DIAGNOSTIC_SLOW_TARGET_MICROS: u64 = 500_000;

async fn forward_language_command(
    application: &impl LanguageCommandApplication,
    language_id: &str,
    operation: LanguageCommandOperation,
    project_root: std::path::PathBuf,
    machine_readable: bool,
) -> Result<(), String> {
    application
        .execute(LanguageCommandRequest {
            language_id: agent_semantic_client::LanguageId::new(language_id),
            operation,
            project_root,
            machine_readable,
        })
        .await
}

async fn forward_runtime_language_command(
    language_id: &str,
    operation: LanguageCommandOperation,
    project_root: std::path::PathBuf,
    machine_readable: bool,
) -> Result<(), String> {
    let state_home = agent_semantic_runtime::state_core::resolve_state_home()?;
    if agent_semantic_client_db::read_runtime_server_endpoint(&state_home)
        .await?
        .is_none()
    {
        crate::server::runtime_server::ensure_healthy_runtime_server_for_bounded_operation()
            .await?;
    }
    forward_language_command(
        &agent_semantic_client::RuntimeLanguageCommandApplication,
        language_id,
        operation,
        project_root,
        machine_readable,
    )
    .await
}

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
    if is_runtime_exact_query(&command_args) {
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
        return forward_runtime_language_command(
            language_id,
            LanguageCommandOperation::ExactQuery(intent),
            exact_project_root,
            presentation,
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
            return forward_runtime_language_command(
                language_id,
                LanguageCommandOperation::Search(intent),
                project_root,
                true,
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

fn runtime_query_intent(args: &[String]) -> Result<AspClientExactQueryRequest, String> {
    if let Some(removed) = args
        .iter()
        .find(|argument| matches!(argument.as_str(), "--code" | "--names-only"))
    {
        return Err(format!("unexpected argument `{removed}` for exact query"));
    }
    let selector = super::provider_selector::exact_query_selector_argument(args)
        .ok_or_else(|| "query requires a canonical selector".to_owned())?;
    let projection = option_value(args, "--projection")?.unwrap_or_else(|| "source".to_owned());
    Ok(AspClientExactQueryRequest {
        schema_id: "agent.semantic-protocols.asp-client-exact-query-request".to_owned(),
        schema_version: "1".to_owned(),
        selector: selector.to_owned(),
        projection,
    })
}

fn runtime_query_presentation(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--json")
}

fn runtime_search_intent(args: &[String]) -> Result<AspClientSearchRequest, String> {
    let playbook = agent_semantic_search::parse_search_playbook_args(args)?;
    Ok(AspClientSearchRequest {
        schema_id: "agent.semantic-protocols.asp-client-search-request".to_owned(),
        schema_version: "1".to_owned(),
        intent: playbook.intent,
        query: playbook.query,
        scope: playbook.scope,
        coverage: playbook.coverage,
        max_owners: playbook.max_owners,
        deadline_ms: playbook.deadline_ms,
        explain: playbook.explain,
    })
}

use super::provider_selector::is_runtime_exact_query;

#[cfg(test)]
#[path = "../../tests/unit/command/provider_route_intent.rs"]
mod tests;
