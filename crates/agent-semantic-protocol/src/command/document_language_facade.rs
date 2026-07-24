//! Document language command facade glue.

use super::document_provider;

pub(super) fn is_document_language(language_id: &str) -> bool {
    document_provider::is_document_language(language_id)
}

pub(super) fn run_document_language_help(language_id: &str, args: &[String]) -> Result<(), String> {
    document_provider::run_language_command(language_id, args)
}
