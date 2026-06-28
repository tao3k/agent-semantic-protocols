//! Document language provider facade backed by orgize.

use super::{org_archive, org_capture, org_recall, search_config::AspConfig};
use orgize::agent::{self, DocumentLanguage, DocumentWalkConfig};
use std::ffi::OsString;

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
    if command == "archive" && language_id == "org" {
        return org_archive::run_org_archive_command(&args[1..]);
    }
    if command == "recall" && language_id == "org" {
        return org_recall::run_org_recall_command(&args[1..]);
    }
    if command == "capture" && language_id == "org" {
        let capture_args = &args[1..];
        if is_capture_state_command(capture_args) || capture_has_contract(capture_args) {
            return org_capture::run_org_capture_command(capture_args);
        }
        return Err(
            "asp org capture expects `--contract CONTRACT_ID`; use `asp org capture --contract agent.task.v1 --title TITLE --target-file ORG_FILE` for a contract-checked non-mutating Org entry. ASP Org state is initialized during install/sync."
                .to_string(),
        );
    }
    if is_document_command(command) {
        let _generic_session_env = GenericSessionEnvGuard::remove_for(language_id);
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

struct GenericSessionEnvGuard {
    values: Vec<(&'static str, Option<OsString>)>,
}

impl GenericSessionEnvGuard {
    fn remove_for(language_id: &str) -> Self {
        let names = if language_id == "org" {
            &["AGENT_SESSION_ID", "SESSION_ID"][..]
        } else {
            &[][..]
        };
        let values = names
            .iter()
            .map(|name| {
                let value = std::env::var_os(name);
                unsafe {
                    std::env::remove_var(name);
                }
                (*name, value)
            })
            .collect();
        Self { values }
    }
}

impl Drop for GenericSessionEnvGuard {
    fn drop(&mut self) {
        for (name, value) in self.values.drain(..) {
            if let Some(value) = value {
                unsafe {
                    std::env::set_var(name, value);
                }
            } else {
                unsafe {
                    std::env::remove_var(name);
                }
            }
        }
    }
}

fn is_language_help(args: &[String]) -> bool {
    args.len() == 1 && matches!(args[0].as_str(), "--help" | "-h" | "help")
}

fn is_capture_state_command(args: &[String]) -> bool {
    matches!(
        args.first().map(String::as_str),
        Some("init" | "--state-root" | "--source-dir" | "--help" | "-h" | "help")
    )
}

fn capture_has_contract(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--contract")
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
        "org" => {
            "guide|search|query|elements-query|contract|capture|recall|archive|export|fmt|lint"
        }
        _ => "guide|search|query|elements-query",
    }
}

fn is_document_command(command: &str) -> bool {
    matches!(command, "guide" | "search" | "query" | "elements-query")
}

fn is_embedded_org_command(command: &str) -> bool {
    matches!(command, "export" | "fmt" | "lint")
}
