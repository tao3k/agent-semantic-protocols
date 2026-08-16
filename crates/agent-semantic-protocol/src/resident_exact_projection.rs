use agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead;
use agent_semantic_content_identity::CanonicalItemSelector;

pub(crate) enum ResidentExactProjection {
    Hit(Vec<u8>),
    Miss(ResidentExactProjectionMiss),
}

pub(crate) struct ResidentExactProjectionMiss {
    pub(crate) owner_path: String,
    pub(crate) structural_selector: String,
    pub(crate) active_generation_digest: String,
    pub(crate) root_digest: String,
    pub(crate) item_kind: String,
    pub(crate) item_name: String,
    pub(crate) candidates: Vec<String>,
    pub(crate) actual_kinds: Vec<String>,
    pub(crate) state: &'static str,
    pub(crate) reason_kind: &'static str,
}

pub(crate) fn resolve(
    read: WorkspaceRuntimeSelectorRead,
    structural_selector: &str,
) -> Result<ResidentExactProjection, String> {
    let requested_path = agent_semantic_content_identity::exact_structural_selector::ExactStructuralSelectorPathV1::parse(
        structural_selector,
    )?;
    let requested = CanonicalItemSelector::parse_root_or_exact_descendant(structural_selector)?;
    if requested_path.root_selector != requested.structural_selector {
        return Err(
            "exact structural selector root identity does not match canonical item identity"
                .to_owned(),
        );
    }
    let requested_is_descendant = !requested_path.segments.is_empty();
    let owner_path = structural_selector
        .split_once("://")
        .and_then(|(_, selector)| selector.split_once('#'))
        .map(|(owner_path, _)| owner_path)
        .ok_or_else(|| "exact structural selector is missing its owner path".to_owned())?;
    let (active_generation_digest, root_digest, owner) = match read {
        WorkspaceRuntimeSelectorRead::ProviderProjection {
            owner_content_digest: _,
            resolved_selector: _,
            bytes,
        } => return Ok(ResidentExactProjection::Hit(bytes)),
        WorkspaceRuntimeSelectorRead::Projection {
            resolved_selector,
            bytes,
            generation_digest,
            root_digest,
        } => {
            let _ = (resolved_selector, generation_digest, root_digest);
            return Ok(ResidentExactProjection::Hit(bytes));
        }
        WorkspaceRuntimeSelectorRead::ProjectionMissing {
            generation_digest,
            root_digest,
            resolved_selector,
        } => {
            return Ok(ResidentExactProjection::Miss(ResidentExactProjectionMiss {
                owner_path: owner_path.to_owned(),
                structural_selector: structural_selector.to_owned(),
                active_generation_digest: generation_digest,
                root_digest,
                item_kind: requested.kind.as_str().to_owned(),
                item_name: requested.symbol.as_str().to_owned(),
                candidates: vec![resolved_selector],
                actual_kinds: vec![requested.kind.as_str().to_owned()],
                state: "source-unavailable",
                reason_kind: "projection-mode-not-in-active-generation",
            }));
        }
        WorkspaceRuntimeSelectorRead::ProjectionScopeOmitted {
            generation_digest,
            root_digest,
            resolved_selector,
            projection_scope: _,
            owner_content_digest: _,
        } => {
            return Ok(ResidentExactProjection::Miss(ResidentExactProjectionMiss {
                owner_path: owner_path.to_owned(),
                structural_selector: structural_selector.to_owned(),
                active_generation_digest: generation_digest,
                root_digest,
                item_kind: requested.kind.as_str().to_owned(),
                item_name: requested.symbol.as_str().to_owned(),
                candidates: vec![resolved_selector],
                actual_kinds: vec![requested.kind.as_str().to_owned()],
                state: "source-unavailable",
                reason_kind: "projection-scope-omitted",
            }));
        }
        WorkspaceRuntimeSelectorRead::OwnerForRepair {
            generation_digest,
            root_digest,
            owner,
        } => (generation_digest, root_digest, Some(owner)),
        WorkspaceRuntimeSelectorRead::OwnerMissing {
            generation_digest,
            root_digest,
        } => (generation_digest, root_digest, None),
        WorkspaceRuntimeSelectorRead::RelocationAmbiguous {
            generation_digest,
            root_digest,
            candidates,
        } => {
            let owner_candidates = candidates
                .into_iter()
                .filter(|candidate| {
                    candidate
                        .split_once("://")
                        .and_then(|(_, selector)| selector.split_once('#'))
                        .is_some_and(|(candidate_owner, _)| candidate_owner == owner_path)
                })
                .collect::<Vec<_>>();
            if owner_candidates.is_empty() {
                return Ok(ResidentExactProjection::Miss(ResidentExactProjectionMiss {
                    owner_path: owner_path.to_owned(),
                    structural_selector: structural_selector.to_owned(),
                    active_generation_digest: generation_digest,
                    root_digest,
                    item_kind: requested.kind.as_str().to_owned(),
                    item_name: requested.symbol.as_str().to_owned(),
                    candidates: Vec::new(),
                    actual_kinds: Vec::new(),
                    state: "owner-missing",
                    reason_kind: "owner-not-in-workspace",
                }));
            }
            return Ok(ResidentExactProjection::Miss(ResidentExactProjectionMiss {
                owner_path: owner_path.to_owned(),
                structural_selector: structural_selector.to_owned(),
                active_generation_digest: generation_digest,
                root_digest,
                item_kind: requested.kind.as_str().to_owned(),
                item_name: requested.symbol.as_str().to_owned(),
                candidates: owner_candidates,
                actual_kinds: vec![requested.kind.as_str().to_owned()],
                state: "ambiguous",
                reason_kind: "canonical-item-identity-ambiguous",
            }));
        }
        WorkspaceRuntimeSelectorRead::GenerationMissing => {
            return Err(
                "exact source query state=source-unavailable reasonKind=active-workspace-generation-required"
                    .to_owned(),
            );
        }
    };
    let Some(owner) = owner else {
        return Ok(ResidentExactProjection::Miss(ResidentExactProjectionMiss {
            owner_path: owner_path.to_owned(),
            structural_selector: structural_selector.to_owned(),
            active_generation_digest,
            root_digest,
            item_kind: requested.kind.as_str().to_owned(),
            item_name: requested.symbol.as_str().to_owned(),
            candidates: Vec::new(),
            actual_kinds: Vec::new(),
            state: "owner-missing",
            reason_kind: "owner-not-in-workspace",
        }));
    };
    let candidates = owner
        .selectors
        .iter()
        .map(|selector| selector.selector.clone())
        .collect::<Vec<_>>();
    let actual_kinds = candidates
        .iter()
        .filter_map(|candidate| {
            CanonicalItemSelector::parse_root_or_exact_descendant(candidate.clone()).ok()
        })
        .filter(|candidate| candidate.symbol == requested.symbol)
        .map(|candidate| candidate.kind.as_str().to_owned())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let selector_exists = candidates
        .iter()
        .any(|candidate| candidate == structural_selector);
    let root_kind_exists = actual_kinds
        .iter()
        .any(|actual_kind| actual_kind == requested.kind.as_str());
    let (state, reason_kind) = if selector_exists || (requested_is_descendant && root_kind_exists) {
        (
            "source-unavailable",
            "projection-mode-not-in-active-generation",
        )
    } else if actual_kinds.is_empty() {
        ("item-missing", "item-not-in-live-owner")
    } else {
        ("kind-mismatch", "snapshot-item-kind-mismatch")
    };
    Ok(ResidentExactProjection::Miss(ResidentExactProjectionMiss {
        owner_path: owner_path.to_owned(),
        structural_selector: structural_selector.to_owned(),
        active_generation_digest,
        root_digest,
        item_kind: requested.kind.as_str().to_owned(),
        item_name: requested.symbol.as_str().to_owned(),
        candidates,
        actual_kinds,
        state,
        reason_kind,
    }))
}

#[cfg(test)]
#[path = "../tests/unit/resident_exact_projection.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/unit/resident_exact_projection_generation_identity.rs"]
mod generation_identity_tests;
