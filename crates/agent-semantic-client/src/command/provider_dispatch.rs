//! Language provider command facade.

use super::provider_usage;

use std::env;

use super::protocol_version_line;
pub(crate) use super::provider_selector::is_language_facade;
pub(crate) use super::provider_selector::unsupported_language_facade_message;
use provider_usage::guide_usage;
use provider_usage::is_guide;
use provider_usage::provider_usage;

/// Observational target only; it never controls or cancels search execution.
const SEARCH_DIAGNOSTIC_SLOW_TARGET_MICROS: u64 = 500_000;

pub(crate) async fn run_language_command(
    language_id: &str,
    args: &[String],
    process_started: tokio::time::Instant,
) -> Result<(), String> {
    fn uses_client_backend(args: &[String]) -> bool {
        matches!(args.first().map(String::as_str), Some("cache"))
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
    match command_args.first().map(String::as_str) {
        Some("search") => {
            return Err(
                "language-first Search was removed; use `asp search playbook --languages <language|...> ...`"
                    .to_owned(),
            );
        }
        Some("query") => {
            return Err(
                "language-first Query was removed; use `asp query --selector <exact-selector>` or `asp query --languages <language|...> --syntax ...`"
                    .to_owned(),
            );
        }
        _ => {}
    }
    if uses_client_backend(&command_args) {
        let (_project_root, provider_args) =
            super::provider_roots::explicit_workspace_project_root(
                language_id,
                &command_args,
                &invocation_root,
            )?
            .unwrap_or_else(|| (invocation_root.clone(), command_args.clone()));
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

#[cfg(test)]
#[path = "../../tests/unit/command/provider_route_intent.rs"]
mod tests;
