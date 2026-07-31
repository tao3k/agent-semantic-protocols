use agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead;
use agent_semantic_content_identity::CanonicalItemSelector;

pub(crate) enum ResidentExactProjection {
    Hit(Vec<u8>),
    Miss(ResidentExactProjectionMiss),
}

pub(crate) struct ResidentExactProjectionMiss {
    pub(crate) owner_path: String,
    pub(crate) structural_selector: String,
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
    let requested = CanonicalItemSelector::parse_root_or_exact_descendant(structural_selector)?;
    let owner_path = structural_selector
        .split_once("://")
        .and_then(|(_, selector)| selector.split_once('#'))
        .map(|(owner_path, _)| owner_path)
        .ok_or_else(|| "exact structural selector is missing its owner path".to_owned())?;
    let (root_digest, owner) = match read {
        WorkspaceRuntimeSelectorRead::Projection { bytes, .. } => {
            return Ok(ResidentExactProjection::Hit(bytes));
        }
        WorkspaceRuntimeSelectorRead::OwnerForRepair {
            generation_digest,
            root_digest,
            owner,
        } => {
            let _ = generation_digest;
            (root_digest, Some(owner))
        }
        WorkspaceRuntimeSelectorRead::OwnerMissing {
            generation_digest,
            root_digest,
        } => {
            let _ = generation_digest;
            (root_digest, None)
        }
        WorkspaceRuntimeSelectorRead::GenerationMissing => {
            return Err("runtime workspace generation is not admitted".to_owned());
        }
    };
    let Some(owner) = owner else {
        return Ok(ResidentExactProjection::Miss(ResidentExactProjectionMiss {
            owner_path: owner_path.to_owned(),
            structural_selector: structural_selector.to_owned(),
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
    let (state, reason_kind) = if actual_kinds.is_empty() {
        ("item-missing", "item-not-in-live-owner")
    } else {
        ("kind-mismatch", "snapshot-item-kind-mismatch")
    };
    Ok(ResidentExactProjection::Miss(ResidentExactProjectionMiss {
        owner_path: owner_path.to_owned(),
        structural_selector: structural_selector.to_owned(),
        root_digest,
        item_kind: requested.kind.as_str().to_owned(),
        item_name: requested.symbol.as_str().to_owned(),
        candidates,
        actual_kinds,
        state,
        reason_kind,
    }))
}
