//! Document language provider facade backed by orgize.

use super::search_config::AspConfig;
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
    if is_help(args) {
        println!("{}", usage(language_id));
        return Ok(());
    }
    let Some(command) = args.first().map(String::as_str) else {
        return Err(usage(language_id));
    };
    if !matches!(command, "guide" | "search" | "query" | "elements-query") {
        return Err(format!(
            "asp {language_id}: unsupported document command `{command}`; supported commands are guide, search, query, elements-query"
        ));
    }

    agent::run_document_command_with_walk_config(
        document_language(language_id)?,
        args.to_vec(),
        DocumentWalkConfig::new(
            config.search.ignore_dirs.clone(),
            config.search.include_hidden_dirs.clone(),
        ),
    )
}

fn is_help(args: &[String]) -> bool {
    args.iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h" | "help"))
}

fn document_language(language_id: &str) -> Result<DocumentLanguage, String> {
    match language_id {
        "org" => Ok(DocumentLanguage::Org),
        "md" => Ok(DocumentLanguage::Markdown),
        _ => Err(format!("unsupported document language `{language_id}`")),
    }
}

fn usage(language_id: &str) -> String {
    format!("usage: asp {language_id} <guide|search|query|elements-query> ...")
}
