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
        if projection.projection_kind
            != crate::runtime_server_workspace::ExactProjectionKind::CallableSkeleton
        {
            return Err(format!(
                "workspace derived selector projection kind is unsupported: projectionKind={}",
                projection.projection_kind
            ));
        }
        if projection.bytes.is_empty() {
            return Err("workspace derived selector projection bytes are empty".to_owned());
        }
        let envelope: agent_semantic_content_identity::semantic_projection::SemanticProjection<agent_semantic_content_identity::callable_skeleton_projection::CallableSkeletonPayload> =
            serde_json::from_slice(&projection.bytes).map_err(|error| {
                format!(
                    "workspace callable-skeleton projection is not shared-schema JSON: selector={} error={error}",
                    selector.selector
                )
            })?;
        envelope.validate().map_err(|error| {
            format!(
                "workspace callable-skeleton projection failed shared validation: selector={} error={error}",
                selector.selector
            )
        })?;
        envelope.payload.validate().map_err(|error| {
            format!("workspace callable-skeleton payload failed shared validation: selector={} error={error}", selector.selector)
        })?;
        envelope.payload.validate_scope(&envelope.root_selector).map_err(|error| {
            format!("workspace callable-skeleton scope failed shared validation: selector={} error={error}", selector.selector)
        })?;
        let context = projection.evidence_context.as_ref().ok_or_else(|| {
            "callable-skeleton projection is missing its evidence context".to_owned()
        })?;
        context.validate().map_err(|error| {
            format!("workspace projection evidence context failed validation: {error}")
        })?;
        if context.evidence_context_ref != envelope.evidence_context_ref
            || context.language_id != envelope.language_id
            || context.provider_id != envelope.provider_id
        {
            return Err("workspace callable-skeleton evidence context identity drift".to_owned());
        }
        if envelope.root_selector != selector.selector {
            return Err(format!(
                "workspace callable-skeleton root selector drift: expected={} actual={}",
                selector.selector, envelope.root_selector
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
