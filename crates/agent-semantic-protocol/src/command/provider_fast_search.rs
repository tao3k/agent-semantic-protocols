//! Determines when parser-owned fast search needs activated provider facts.

use agent_semantic_hook::ActivatedProvider;


pub(super) fn fast_search_needs_provider_context(
    args: &[String],
    _provider: &ActivatedProvider,
) -> Result<bool, String> {
    Ok(matches!(args.first().map(String::as_str), Some("search")))
}
