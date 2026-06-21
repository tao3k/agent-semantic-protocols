//! Document language provider facade backed by orgize.

use super::{org_capture, search_config::AspConfig};
use orgize::agent::{self, DocumentLanguage, DocumentWalkConfig};

const DOCUMENT_LANGUAGES: &[&str] = &["org", "md"];

pub(crate) fn is_document_language(language_id: &str) -> bool {
    DOCUMENT_LANGUAGES.contains(&language_id)
}

pub(crate) fn run_language_command(language_id: &str, args: &[String]) -> Result<(), String> {
    run_language_command_with_config(language_id, args, &AspConfig::default())
}

pub(crate) fn run_language_command_with_config(
    language_id: &str,
    args: &[String],
    config: &AspConfig,
) -> Result<(), String> {
    if is_language_help(args) {
        println!("{}", usage(language_id));
        return Ok(());
    }
    let Some(command) = args.first().map(String::as_str) else {
        return Err(usage(language_id));
    };
    if command == "contract" && language_id == "org" {
        return agent::run_org_contract_command(args[1..].to_vec());
    }
    if command == "capture" && language_id == "org" {
        let capture_args = &args[1..];
        if is_capture_state_command(capture_args) {
            return org_capture::run_org_capture_command(capture_args);
        }
        let mut orgize_args = Vec::with_capacity(args.len());
        orgize_args.push("capture-plan".to_string());
        orgize_args.extend(capture_args.iter().cloned());
        return agent::run_org_cli_command(orgize_args);
    }
    if is_document_command(command) {
        return agent::run_document_command_with_walk_config(
            document_language(language_id)?,
            args.to_vec(),
            DocumentWalkConfig::new(
                config.search.ignore_dirs.clone(),
                config.search.include_hidden_dirs.clone(),
            ),
        );
    }
    if language_id == "org" && is_embedded_org_command(command) {
        return agent::run_org_cli_command(args.to_vec());
    }
    if !is_document_command(command) {
        return Err(format!(
            "asp {language_id}: unsupported document command `{command}`; supported commands are {}",
            supported_commands(language_id)
        ));
    }

    unreachable!("document commands are returned above")
}

fn is_language_help(args: &[String]) -> bool {
    args.len() == 1 && matches!(args[0].as_str(), "--help" | "-h" | "help")
}

fn is_capture_state_command(args: &[String]) -> bool {
    args.is_empty()
        || args.iter().any(|arg| {
            matches!(
                arg.as_str(),
                "init" | "--state-root" | "--source-dir" | "--help" | "-h" | "help"
            )
        })
}

fn document_language(language_id: &str) -> Result<DocumentLanguage, String> {
    match language_id {
        "org" => Ok(DocumentLanguage::Org),
        "md" => Ok(DocumentLanguage::Markdown),
        _ => Err(format!("unsupported document language `{language_id}`")),
    }
}

fn usage(language_id: &str) -> String {
    format!(
        "usage: asp {language_id} <{}> ...",
        supported_commands(language_id)
    )
}

fn supported_commands(language_id: &str) -> &'static str {
    match language_id {
        "org" => "guide|search|query|elements-query|contract|capture|export|fmt|lint",
        _ => "guide|search|query|elements-query",
    }
}

fn is_document_command(command: &str) -> bool {
    matches!(command, "guide" | "search" | "query" | "elements-query")
}

fn is_embedded_org_command(command: &str) -> bool {
    matches!(command, "export" | "fmt" | "lint")
}
