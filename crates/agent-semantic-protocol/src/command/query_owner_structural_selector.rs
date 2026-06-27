//! Structural owner selector parsing for ASP-owned query projections.

use std::path::PathBuf;

pub(super) struct StructuralOwnerQuery {
    pub(super) owner_path: PathBuf,
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
        _ => return None,
    };
    let prefix = format!("{language_id}://");
    let rest = selector.strip_prefix(&prefix)?;
    let (owner_path, item_fragment) = rest.split_once("#item/")?;
    if owner_path.is_empty() {
        return None;
    }
    let name = item_fragment.rsplit('/').next()?.trim();
    let name = name.strip_prefix("query-axis:").unwrap_or(name);
    if name.is_empty() {
        return None;
    }
    Some(StructuralOwnerQuery {
        owner_path: PathBuf::from(owner_path),
        term: name.to_string(),
        projection,
    })
}
