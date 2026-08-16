//! Typed fixtures for active-generation projection capability tests.

use agent_semantic_client_db::active_generation_projection_capability::{
    ActiveGenerationProjectionCapabilityManifest, ActiveGenerationProjectionCapabilityReceipt,
    ActiveGenerationProjectionMode, ActiveGenerationSelectorCapability,
};

pub(crate) fn projection_capability_manifest_fixture()
-> ActiveGenerationProjectionCapabilityManifest {
    ActiveGenerationProjectionCapabilityManifest::single_selector(
        "blake3-256:0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
        "rust://fixture/src/lib.rs#item/function/fixture".to_owned(),
        "src/lib.rs".to_owned(),
        std::collections::BTreeSet::from([ActiveGenerationProjectionMode::Source]),
    )
    .expect("test projection capability manifest")
}

pub(crate) fn overlay_projection_capability_manifest_fixture()
-> ActiveGenerationProjectionCapabilityManifest {
    let modes = std::collections::BTreeSet::from([
        ActiveGenerationProjectionMode::Source,
        ActiveGenerationProjectionMode::CallableSkeleton,
    ]);
    let manifest = ActiveGenerationProjectionCapabilityManifest {
        provider_catalog_digest:
            "blake3-256:0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
        selectors: ["first", "second"]
            .into_iter()
            .map(|name| ActiveGenerationSelectorCapability {
                selector: format!("rust://src/lib.rs#item/function/{name}"),
                owner_path: "src/lib.rs".to_owned(),
                projection_modes: modes.clone(),
            })
            .collect(),
    };
    manifest
        .validate()
        .expect("overlay projection capability manifest");
    manifest
}

pub(crate) fn ready_projection_capability_fixture(
    workspace_identity: impl Into<String>,
    generation_digest: impl Into<String>,
    root_digest: impl Into<String>,
    publication_epoch: u64,
) -> ActiveGenerationProjectionCapabilityReceipt {
    projection_capability_manifest_fixture()
        .into_ready_receipt(
            workspace_identity.into(),
            generation_digest.into(),
            root_digest.into(),
            publication_epoch,
        )
        .expect("test projection capability receipt")
}
