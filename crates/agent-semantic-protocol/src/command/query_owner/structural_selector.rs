//! Structural owner selector parsing for ASP-owned query projections.

use std::path::PathBuf;

pub(super) struct StructuralOwnerQuery {
    pub(super) owner_path: PathBuf,
    pub(super) kind: Option<String>,
    pub(super) term: String,
    pub(super) projection: &'static str,
}

pub(super) fn parse_structural_owner_query(
    language_id: &str,
    from_hook: &str,
    selector: &str,
) -> Option<StructuralOwnerQuery> {
    let projection = match from_hook {
        "item-skeleton" => "skeleton",
        "syntax-outline" => "outline",
        "query-code" => "code",
        _ => return None,
    };
    let rest = structural_selector_rest(language_id, selector)?;
    let (owner_path, item_fragment) = rest.split_once("#item/")?;
    if owner_path.is_empty() {
        return None;
    }
    let (kind, name) = item_fragment
        .split_once('/')
        .map_or((None, item_fragment), |(kind, name)| (Some(kind), name));
    let name = name.strip_prefix("query-axis:").unwrap_or(name);
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    Some(StructuralOwnerQuery {
        owner_path: PathBuf::from(owner_path),
        kind: kind
            .map(str::trim)
            .filter(|kind| !kind.is_empty())
            .map(ToString::to_string),
        term: name.to_string(),
        projection,
    })
}

fn structural_selector_rest<'a>(language_id: &str, selector: &'a str) -> Option<&'a str> {
    for prefix in structural_selector_prefixes(language_id) {
        if let Some(rest) = selector.strip_prefix(prefix) {
            return Some(rest);
        }
    }
    None
}

fn structural_selector_prefixes(language_id: &str) -> &'static [&'static str] {
    match language_id {
        "typescript" => &["typescript://", "ts://"],
        "gerbil-scheme" => &["gerbil-scheme://", "gerbil://"],
        "rust" => &["rust://"],
        "python" => &["python://"],
        "julia" => &["julia://"],
        _ => &[],
    }
}
