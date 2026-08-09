use super::{ResidentExactProjection, resolve};
use agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead;

const SELECTOR: &str = "rust://src/lib.rs#item/function/run";

#[test]
fn absent_owner_without_requested_generation_is_not_selector_stale() {
    let resolved = resolve(
        WorkspaceRuntimeSelectorRead::OwnerMissing {
            generation_digest: "blake3-256:active".to_owned(),
            root_digest: "source-root".to_owned(),
        },
        SELECTOR,
    )
    .expect("resolve absent owner");

    let ResidentExactProjection::Miss(miss) = resolved else {
        panic!("absent owner must remain a typed miss");
    };
    assert_eq!(miss.state, "owner-missing");
    assert_eq!(miss.reason_kind, "owner-not-in-workspace");
}
