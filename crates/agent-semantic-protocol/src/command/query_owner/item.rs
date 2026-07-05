//! Shared owner item model for bounded owner query projections.

#[derive(Debug)]
pub(in crate::command) struct OwnerItem {
    pub(super) name: String,
    pub(super) kind: &'static str,
    pub(super) syntax_node: &'static str,
    pub(super) start_line: usize,
    pub(super) end_line: usize,
}

impl OwnerItem {
    pub(in crate::command) fn name(&self) -> &str {
        &self.name
    }

    pub(in crate::command) fn kind(&self) -> &str {
        self.kind
    }

    pub(in crate::command) fn start_line(&self) -> usize {
        self.start_line
    }

    pub(in crate::command) fn end_line(&self) -> usize {
        self.end_line
    }
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
