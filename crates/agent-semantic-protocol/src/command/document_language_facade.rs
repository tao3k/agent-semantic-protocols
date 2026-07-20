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
