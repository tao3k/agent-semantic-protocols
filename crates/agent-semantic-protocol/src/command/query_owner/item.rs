//! Shared owner item model for bounded owner query projections.

#[derive(Debug)]
pub(super) struct OwnerItem {
    pub(super) name: String,
    pub(super) kind: &'static str,
    pub(super) syntax_node: &'static str,
    pub(super) start_line: usize,
    pub(super) end_line: usize,
}

pub(super) fn owner_item_matches_request(
    item: &OwnerItem,
    language_id: &str,
    term: &str,
    selector_kind: Option<&str>,
) -> bool {
    if item.name != term {
        return false;
    }
    selector_kind
        .is_none_or(|selector_kind| owner_item_kind_matches(language_id, item.kind, selector_kind))
}

fn owner_item_kind_matches(language_id: &str, actual: &str, selector: &str) -> bool {
    actual == selector
        || matches!(
            (language_id, actual, selector),
            ("rust", "function", "fn")
                | ("rust", "trait-function", "fn")
                | ("rust", "method", "fn")
                | ("typescript", "function", "fn")
        )
}
