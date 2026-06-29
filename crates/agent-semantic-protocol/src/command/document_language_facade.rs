//! Document language command facade glue.

use std::path::Path;

use super::document_provider;
use super::graph::GraphTurboReceiptRequest;
use super::search_config::AspConfig;
use super::search_pipe::{FastSearchContext, run_asp_fast_search_command};

pub(super) fn is_document_language(language_id: &str) -> bool {
    document_provider::is_document_language(language_id)
}

pub(super) fn run_document_language_help(language_id: &str, args: &[String]) -> Result<(), String> {
    document_provider::run_language_command(language_id, args)
}

pub(super) fn run_document_language_command(
    language_id: &str,
    command_args: &[String],
    invocation_root: &Path,
    frontier_receipt: Option<&GraphTurboReceiptRequest>,
) -> Result<(), String> {
    let config = AspConfig::load(invocation_root, invocation_root);
    if !config.language_enabled(language_id) {
        return Err(format!("language `{language_id}` is disabled by asp.toml"));
    }
    if is_search_pipe_command(command_args) {
        let cache_home = agent_semantic_client_core::ProjectContext::resolve(invocation_root)?
            .state_layout()
            .client_cache_dir()
            .to_path_buf();
        return run_asp_fast_search_command(
            command_args,
            FastSearchContext {
                language_id,
                project_root: invocation_root,
                locator_root: invocation_root,
                cache_home: &cache_home,
                config: &config,
                provider_context: None,
                frontier_receipt,
            },
        );
    }
    document_provider::run_language_command_with_config(language_id, command_args, &config)
}

fn is_search_pipe_command(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("search"))
        && matches!(args.get(1).map(String::as_str), Some("pipe"))
}
