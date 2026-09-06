// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Typed fixtures for active-generation projection capability tests.

use agent_semantic_client_db::active_generation_projection_capability::ActiveGenerationProjectionCapabilityManifest;
use agent_semantic_client_db::active_generation_projection_capability::ActiveGenerationProjectionCapabilityReceipt;
use agent_semantic_client_db::active_generation_projection_capability::ActiveGenerationProjectionMode;
use agent_semantic_client_db::active_generation_projection_capability::ActiveGenerationSelectorCapability;

pub(crate) const FIXTURE_PROJECT_ID: &str = "repo-0000000000000001";

pub(crate) fn content_search_generation_receipt(
    workspace_identity: &str,
    source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
) -> agent_semantic_search::ContentSearchGenerationReceipt {
    use agent_semantic_search::ContentSearchGenerationReceipt;
    use agent_semantic_search::SearchGenerationConstructionStage;
    use agent_semantic_search::SearchGenerationIdentity;
    use agent_semantic_search::SearchGenerationStageReceipt;
    use agent_semantic_search::canonical_blake3_digest;

    let identity = SearchGenerationIdentity {
        project_id: FIXTURE_PROJECT_ID.to_owned(),
        workspace_id: workspace_identity.to_owned(),
        source_root_digest: canonical_blake3_digest(&source_snapshot.root_digest)
            .expect("fixture source root digest"),
        provider_digest: canonical_blake3_digest(&source_snapshot.provider_digest)
            .expect("fixture provider digest"),
        schema_digest: canonical_blake3_digest(
            &agent_semantic_content_identity::project_resolution_schema_digest(),
        )
        .expect("fixture ProjectResolution schema digest"),
        generation_candidate_digest:
            "blake3-256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
    };
    let stage = |stage, artifact_byte: char, worker_id: &str| SearchGenerationStageReceipt {
        stage,
        identity: identity.clone(),
        artifact_digest: format!("blake3-256:{}", artifact_byte.to_string().repeat(64)),
        worker_id: worker_id.to_owned(),
        complete: true,
    };
    ContentSearchGenerationReceipt::new(stage(
        SearchGenerationConstructionStage::SourceByteAcquisition,
        '4',
        "fixture-source-byte-acquisition-v1",
    ))
    .expect("fixture content search generation receipt")
}

pub(crate) fn projection_capability_manifest_fixture()
-> ActiveGenerationProjectionCapabilityManifest {
    ActiveGenerationProjectionCapabilityManifest::single_selector(
        "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
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
#[test]
fn projection_capability_fixture_uses_a_v1_provider_catalog_digest() {
    let manifest = projection_capability_manifest_fixture();
    assert!(
        manifest.provider_catalog_digest.starts_with("sha256:")
            && manifest.provider_catalog_digest.len() == "sha256:".len() + 64,
        "actual={}",
        manifest.provider_catalog_digest
    );
}
