use super::{WorkspaceOwnerSnapshot, WorkspaceSelectorSnapshot};

pub(super) fn validate_selector(
    owner: &WorkspaceOwnerSnapshot,
    selector: &WorkspaceSelectorSnapshot,
) -> Result<(), String> {
    if selector.selector.trim().is_empty() {
        return Err("workspace selector must be non-empty text".to_owned());
    }
    let (_, selector_target) = selector
        .selector
        .split_once("://")
        .ok_or_else(|| "workspace selector must include a language scheme".to_owned())?;
    let (selector_owner, _) = selector_target
        .split_once('#')
        .ok_or_else(|| "workspace selector must include an owner fragment".to_owned())?;
    if selector_owner != owner.owner_path {
        return Err(format!(
            "workspace selector owner drift: selector={} ownerPath={}",
            selector.selector, owner.owner_path
        ));
    }
    if selector.byte_start > selector.byte_end || selector.byte_end > owner.bytes.len() {
        return Err(format!(
            "workspace selector byte range is invalid: selector={}",
            selector.selector
        ));
    }
    let mut projection_kinds = std::collections::HashSet::new();
    for projection in &selector.derived_projections {
        if projection.projection_kind != "callable-skeleton" {
            return Err(format!(
                "workspace derived selector projection kind is unsupported: projectionKind={}",
                projection.projection_kind
            ));
        }
        if projection.bytes.is_empty() {
            return Err("workspace derived selector projection bytes are empty".to_owned());
        }
        let callable_skeleton: agent_semantic_content_identity::callable_skeleton_projection::CallableSkeletonProjectionV1 =
            serde_json::from_slice(&projection.bytes).map_err(|error| {
                format!(
                    "workspace callable-skeleton projection is not shared-schema JSON: selector={} error={error}",
                    selector.selector
                )
            })?;
        callable_skeleton.validate().map_err(|error| {
            format!(
                "workspace callable-skeleton projection failed shared validation: selector={} error={error}",
                selector.selector
            )
        })?;
        if callable_skeleton.root_selector.selector != selector.selector {
            return Err(format!(
                "workspace callable-skeleton root selector drift: expected={} actual={}",
                selector.selector, callable_skeleton.root_selector.selector
            ));
        }
        if !projection_kinds.insert(projection.projection_kind.as_str()) {
            return Err(format!(
                "duplicate workspace derived selector projection: selector={} projectionKind={}",
                selector.selector, projection.projection_kind
            ));
        }
    }
    Ok(())
}
