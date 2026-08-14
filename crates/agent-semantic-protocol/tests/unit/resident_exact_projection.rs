use super::{ResidentExactProjection, resolve};
use agent_semantic_client_db::runtime_server_workspace::{
    WorkspaceOwnerSnapshot, WorkspaceRuntimeSelectorRead, WorkspaceSelectorSnapshot,
};

const SELECTOR: &str = "rust://src/lib.rs#item/function/target";

#[test]
fn missing_generation_is_a_typed_read_only_failure() {
    let Err(error) = resolve(WorkspaceRuntimeSelectorRead::GenerationMissing, SELECTOR) else {
        panic!("a query must not admit a missing generation");
    };
    assert_eq!(
        error,
        "exact source query state=source-unavailable reasonKind=active-workspace-generation-required"
    );
}

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
fn descendant_projection_missing_from_generation_is_not_a_root_kind_mismatch() {
    let descendant_selector = format!("{SELECTOR}/segment/binding/ordinal-1");
    let resolution = resolve(
        WorkspaceRuntimeSelectorRead::OwnerForRepair {
            generation_digest: "generation".to_owned(),
            root_digest: "root".to_owned(),
            owner: owner_with(SELECTOR),
        },
        &descendant_selector,
    )
    .expect("resolve resident descendant projection");
    let ResidentExactProjection::Miss(miss) = resolution else {
        panic!("expected projection-mode miss");
    };
    assert_eq!(miss.structural_selector, descendant_selector);
    assert_eq!(miss.state, "source-unavailable");
    assert_eq!(miss.reason_kind, "projection-mode-not-in-active-generation");
    assert_eq!(miss.actual_kinds, ["function"]);
}

#[test]
fn uniquely_relocated_projection_is_an_exact_selector_hit() {
    let requested = "rust://src/lib.rs#item/function/target";
    let resolved = "rust://src/moved.rs#item/function/target";
    let resolution = resolve(
        WorkspaceRuntimeSelectorRead::Projection {
            generation_digest: "generation".to_owned(),
            root_digest: "root".to_owned(),
            resolved_selector: resolved.to_owned(),
            bytes: b"fn target() {}\n".to_vec(),
        },
        requested,
    )
    .expect("resolve relocated resident projection");
    let ResidentExactProjection::Hit(bytes) = resolution else {
        panic!("unique relocated selector must remain an exact hit");
    };
    assert_eq!(bytes, b"fn target() {}\n");
}

#[test]
fn relocated_item_with_missing_projection_is_not_selector_stale() {
    let requested = "rust://src/lib.rs#item/function/target";
    let resolved = "rust://src/moved.rs#item/function/target";
    let resolution = resolve(
        WorkspaceRuntimeSelectorRead::ProjectionMissing {
            generation_digest: "generation".to_owned(),
            root_digest: "root".to_owned(),
            resolved_selector: resolved.to_owned(),
        },
        requested,
    )
    .expect("resolve missing relocated projection");
    let ResidentExactProjection::Miss(miss) = resolution else {
        panic!("missing projection must remain a typed miss");
    };
    assert_eq!(miss.state, "source-unavailable");
    assert_eq!(miss.reason_kind, "projection-mode-not-in-active-generation");
    assert_eq!(miss.candidates, [resolved]);
}

#[test]
fn cross_owner_symbol_candidates_do_not_make_an_exact_selector_ambiguous() {
    let requested = "rust://src/requested.rs#item/function/run";
    let resolution = resolve(
        WorkspaceRuntimeSelectorRead::RelocationAmbiguous {
            generation_digest: "generation".to_owned(),
            root_digest: "root".to_owned(),
            candidates: vec![
                "rust://src/other.rs#item/function/run".to_owned(),
                "rust://src/another.rs#item/function/run".to_owned(),
            ],
        },
        requested,
    )
    .expect("resolve owner-scoped selector miss");
    let ResidentExactProjection::Miss(miss) = resolution else {
        panic!("cross-owner symbols must not resolve the requested owner");
    };
    assert_eq!(miss.state, "owner-missing");
    assert_eq!(miss.reason_kind, "owner-not-in-workspace");
    assert!(miss.candidates.is_empty());
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

#[test]
fn provider_projection_does_not_require_a_workspace_generation() {
    let projection = resolve(
        WorkspaceRuntimeSelectorRead::ProviderProjection {
            owner_content_digest: format!("blake3-256:{}", "a".repeat(64)),
            resolved_selector: SELECTOR.to_owned(),
            bytes: b"fn target() {}".to_vec(),
        },
        SELECTOR,
    )
    .expect("provider projection should resolve independently of generation identity");
    let ResidentExactProjection::Hit(bytes) = projection else {
        panic!("provider projection must not degrade into a generation miss");
    };
    assert_eq!(bytes, b"fn target() {}");
}
