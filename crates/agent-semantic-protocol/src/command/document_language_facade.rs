//! Document language command facade glue.

use std::path::{Path, PathBuf};

use super::document_provider;
use super::graph::GraphTurboReceiptRequest;
use super::search_config::AspConfig;
use super::search_pipe::{
    FastSearchContext, is_asp_fast_search, run_asp_fast_search_command, search_workspace_root,
};

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
    if is_asp_fast_search(command_args) {
        let explicit_workspace = explicit_workspace_arg(command_args)?;
        let project_root = search_workspace_root(
            invocation_root,
            invocation_root,
            explicit_workspace.as_deref(),
        );
        let cache_home = agent_semantic_client_core::ProjectContext::resolve(invocation_root)?
            .state_layout()
            .client_cache_dir()
            .to_path_buf();
        return run_asp_fast_search_command(
            command_args,
            FastSearchContext {
                language_id,
                project_root: &project_root,
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

fn explicit_workspace_arg(args: &[String]) -> Result<Option<PathBuf>, String> {
    let mut workspace = None;
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--workspace" {
            let value = args
                .get(index + 1)
                .ok_or_else(|| "--workspace requires a value".to_string())?;
            if workspace.is_some() {
                return Err("expected at most one --workspace argument".to_string());
            }
            workspace = Some(PathBuf::from(value));
            index += 2;
        } else {
            index += 1;
        }
    }
    Ok(workspace)
}
