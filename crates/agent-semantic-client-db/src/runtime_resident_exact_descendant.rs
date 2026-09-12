// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Resolve exact descendants from admitted V1 parser materializations, never files.

use super::{ExactProjectionKind, RuntimeResidentReadClient, WorkspaceRuntimeSelectorRead};
use agent_semantic_content_identity::{
    CanonicalItemSelector, callable_skeleton_projection::CallableSkeletonPayload,
    semantic_projection::SemanticProjection,
};

impl RuntimeResidentReadClient {
    pub(super) fn read_exact_descendant(
        &self,
        selector: &str,
    ) -> Result<WorkspaceRuntimeSelectorRead, String> {
        let root = CanonicalItemSelector::parse_root_or_exact_descendant(selector)?;
        let root_selector = root.structural_selector();
        let skeleton =
            self.read_runtime_selector(ExactProjectionKind::CallableSkeleton, root_selector)?;
        let WorkspaceRuntimeSelectorRead::Projection {
            generation_digest,
            root_digest,
            resolved_selector,
            bytes,
        } = skeleton
        else {
            return Ok(WorkspaceRuntimeSelectorRead::ProjectionMissing {
                generation_digest: self.generation_digest(),
                root_digest: self.owner_merkle_root_digest(),
                resolved_selector: selector.to_owned(),
            });
        };
        if generation_digest != self.generation_digest()
            || root_digest != self.owner_merkle_root_digest()
            || resolved_selector != root_selector
        {
            return Err("resident exact descendant generation or root binding drift".to_owned());
        }
        let range = descendant_range(&bytes, root_selector, selector)?;
        let bytes = self
            .search_projection
            .read_admitted_selector_slice(root_selector, range)?;
        Ok(WorkspaceRuntimeSelectorRead::Projection {
            generation_digest,
            root_digest,
            resolved_selector: selector.to_owned(),
            bytes,
        })
    }
}

fn descendant_range(
    bytes: &[u8],
    root: &str,
    selector: &str,
) -> Result<std::ops::Range<usize>, String> {
    let envelope: SemanticProjection<CallableSkeletonPayload> = serde_json::from_slice(bytes)
        .map_err(|error| format!("decode resident exact descendant envelope: {error}"))?;
    envelope.validate().map_err(|error| error.to_string())?;
    envelope
        .payload
        .validate()
        .map_err(|error| error.to_string())?;
    envelope
        .payload
        .validate_scope(root)
        .map_err(|error| error.to_string())?;
    if envelope.root_selector.as_str() != root {
        return Err("resident exact descendant envelope root drift".to_owned());
    }
    let mut nodes = envelope
        .payload
        .nodes
        .iter()
        .filter(|node| node.queryable && node.selector.as_deref() == Some(selector));
    let node = nodes
        .next()
        .ok_or_else(|| "resident exact descendant is not materialized".to_owned())?;
    if nodes.next().is_some() {
        return Err("resident exact descendant identity is ambiguous".to_owned());
    }
    let range = node
        .source_locator_hint
        .as_ref()
        .and_then(|hint| hint.source_byte_start.zip(hint.source_byte_end))
        .ok_or_else(|| "resident exact descendant has no materialized byte range".to_owned())?;
    let start =
        usize::try_from(range.0).map_err(|_| "descendant byte start overflow".to_owned())?;
    let end = usize::try_from(range.1).map_err(|_| "descendant byte end overflow".to_owned())?;
    Ok(start..end)
}

#[cfg(test)]
#[path = "../tests/unit/runtime_resident_exact_descendant.rs"]
mod tests;
