//! In-process query-owner entry points for scenario gates.

use std::path::Path;

/// Run the fast owner query path and return the rendered output.
pub fn run_fast_owner_query_to_string(
    language_id: &str,
    args: &[String],
    project_root: &Path,
    locator_root: &Path,
) -> Result<Option<String>, String> {
    crate::command::query_owner::runner::run_asp_fast_owner_query_to_string(
        language_id,
        args,
        project_root,
        locator_root,
    )
}
