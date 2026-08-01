use super::{ResidentExactProjection, resolve};
use agent_semantic_client_db::runtime_server_workspace::{
    WorkspaceOwnerSnapshot, WorkspaceRuntimeSelectorRead, WorkspaceSelectorSnapshot,
};

const SELECTOR: &str = "rust://src/lib.rs#item/function/target";

fn owner_with(selector: &str) -> WorkspaceOwnerSnapshot {
    WorkspaceOwnerSnapshot {
        owner_path: "src/lib.rs".to_owned(),
        content_digest: format!("blake3-256:{}", "a".repeat(64)),
        bytes: b"fn target() {}\n".to_vec(),
        selectors: vec![WorkspaceSelectorSnapshot {
            selector: selector.to_owned(),
            byte_start: 0,
            byte_end: 14,
            derived_projections: Vec::new(),
        }],
    }
}

#[test]
fn existing_item_without_requested_projection_is_not_a_kind_mismatch() {
    let resolution = resolve(
        WorkspaceRuntimeSelectorRead::OwnerForRepair {
            generation_digest: "generation".to_owned(),
            root_digest: "root".to_owned(),
            owner: owner_with(SELECTOR),
        },
        SELECTOR,
    )
    .expect("resolve resident projection");
    let ResidentExactProjection::Miss(miss) = resolution else {
        panic!("expected projection-mode miss");
    };
    assert_eq!(miss.state, "source-unavailable");
    assert_eq!(miss.reason_kind, "projection-mode-not-in-active-generation");
    assert_eq!(miss.actual_kinds, ["function"]);
}

#[test]
fn same_symbol_with_another_item_kind_remains_a_real_kind_mismatch() {
    let resolution = resolve(
        WorkspaceRuntimeSelectorRead::OwnerForRepair {
            generation_digest: "generation".to_owned(),
            root_digest: "root".to_owned(),
            owner: owner_with("rust://src/lib.rs#item/struct/target"),
        },
        SELECTOR,
    )
    .expect("resolve resident projection");
    let ResidentExactProjection::Miss(miss) = resolution else {
        panic!("expected item-kind miss");
    };
    assert_eq!(miss.state, "kind-mismatch");
    assert_eq!(miss.reason_kind, "snapshot-item-kind-mismatch");
    assert_eq!(miss.actual_kinds, ["struct"]);
}
