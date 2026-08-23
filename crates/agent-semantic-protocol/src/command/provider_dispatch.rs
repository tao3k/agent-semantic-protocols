//! Language provider command facade.

use super::provider_execution::take_frontier_receipt_request;
use super::provider_usage;

use std::env;
use std::path::Path;

use super::gerbil_deps::try_run_gerbil_deps_index_command;
use super::protocol_version_line;
pub(crate) use super::provider_selector::{
    is_language_facade, unsupported_language_facade_message,
};
use super::search_owner_items::{
    SearchOwnerItemsContext, is_search_owner_items_query, run_search_owner_items_query_command,
};
use provider_usage::{guide_usage, is_guide, provider_usage, validate_provider_command};

/// Observational target only; it never controls or cancels search execution.
const SEARCH_DIAGNOSTIC_SLOW_TARGET_MICROS: u64 = 500_000;

pub(crate) async fn run_language_command(
    language_id: &str,
    args: &[String],
    process_started: tokio::time::Instant,
) -> Result<(), String> {
    let exact_query_started = process_started;
    fn uses_client_backend(args: &[String]) -> bool {
        (args.first().is_some_and(|command| command == "search")
            && args.get(1).is_none_or(|subcommand| subcommand != "guide"))
            || matches!(args.first().map(String::as_str), Some("cache"))
    }

    async fn run_runtime_provider_search_command(
        language_id: &str,
        args: Vec<String>,
        project_root: &Path,
    ) -> Result<(), String> {
        let language_id = agent_semantic_client_core::LanguageId::try_from(language_id)
            .map_err(|error| format!("decode provider search language id: {error}"))?;
        let session =
            crate::server::runtime_server::runtime_server_workspace_session_async(project_root)
                .await?;
        let operation_id = format!(
            "provider-search-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| format!("read provider search operation clock: {error}"))?
                .as_nanos()
        );
        let receipt = session
            .provider_search(operation_id, language_id, args)
            .await?;
        std::io::Write::write_all(&mut std::io::stderr(), &receipt.stderr)
            .map_err(|error| format!("write Runtime provider search stderr: {error}"))?;
        std::io::Write::write_all(&mut std::io::stdout(), &receipt.stdout)
            .map_err(|error| format!("write Runtime provider search stdout: {error}"))?;
        if receipt.status_code != 0 {
            return Err(format!(
                "Runtime provider search failed: statusCode={} operationId={}",
                receipt.status_code, receipt.operation_id
            ));
        }
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
    validate_provider_command(&command_args)?;
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
        if provider_args
            .first()
            .is_some_and(|command| command == "search")
        {
            return run_runtime_provider_search_command(
                language_id,
                provider_args,
                &project_root,
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
use super::provider_selector::is_provider_owned_structural_selector_query;
