use std::sync::Arc;

use agent_semantic_content_identity::exact_selector_generation_fixture::ExactSelectorGenerationIdentityV1;
use agent_semantic_content_identity::exact_selector_generation_fixture::ExactSelectorGenerationRecordV1;
use agent_semantic_content_identity::exact_selector_generation_fixture::ExactSelectorProjectionModeV1;
use agent_semantic_content_identity::exact_selector_generation_fixture::build_exact_selector_generation_fixture_v1;
use agent_semantic_content_identity::exact_selector_generation_fixture::fixture_digest_v1;
use agent_semantic_content_identity::workspace_search_identity::WorkspaceSearchIdentityInputV1;
use agent_semantic_content_identity::workspace_search_identity::WorkspaceSearchIdentityV1;
use agent_semantic_content_identity::workspace_search_identity::WorkspaceSearchScopeKindV1;
use agent_semantic_search::exact_selector_generation_fixture::ExactSelectorFixturePublicationV1;
use agent_semantic_search::exact_selector_generation_fixture::ExactSelectorGenerationMemorySearchV1;
use agent_semantic_search::exact_selector_generation_fixture::publish_immutable_exact_selector_generation_fixture_v1;

fn digest(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn workspace_identity() -> WorkspaceSearchIdentityV1 {
    workspace_identity_with_snapshot(digest(1))
}

fn workspace_identity_with_snapshot(
    source_snapshot_root_digest: [u8; 32],
) -> WorkspaceSearchIdentityV1 {
    WorkspaceSearchIdentityV1::admit(WorkspaceSearchIdentityInputV1 {
        language_id: "rust".to_string(),
        provider_id: "asp-rust".to_string(),
        scope_kind: WorkspaceSearchScopeKindV1::Package,
        requested_discovery_root: "/repo/crates/search".into(),
        cargo_workspace_root: "/repo".into(),
        selected_package_root: Some("/repo/crates/search".into()),
        provider_tool_root: "/providers/asp-rust".into(),
        envelope_root: "/repo/crates/search".into(),
        root_count: 1,
        owner_count: 1,
        leaf_count: 1,
        selector_count: 1,
        source_snapshot_root_digest,
    })
    .expect("workspace identity")
}

#[test]
fn exact_memory_search_has_zero_io_receipt() {
    let generation_digest = digest(4);
    let bytes = build_exact_selector_generation_fixture_v1(
        &ExactSelectorGenerationIdentityV1 {
            workspace_identity_digest: *workspace_identity().identity_digest(),
            language_id: "rust".to_string(),
            provider_id: "asp-rust".to_string(),
            workspace_root_digest: digest(1),
            parser_identity_digest: digest(2),
            query_pack_digest: digest(3),
            generation_digest,
            selector_count: 1,
            owner_count: 1,
            leaf_count: 1,
        },
        vec![ExactSelectorGenerationRecordV1 {
            structural_selector: "rust://src/lib.rs#item/function/run".to_string(),
            owner_path: "src/lib.rs".to_string(),
            owner_subtree_digest: digest(5),
            source_blob_digest: digest(6),
            normalized_parser_facts_digest: digest(7),
            projection_mode: ExactSelectorProjectionModeV1::Source,
            source_byte_range: 0..11,
            projection: b"fn run() {}".to_vec(),
        }],
    )
    .expect("fixture");
    let fixture_digest = *fixture_digest_v1(&bytes).expect("fixture digest");
    let search = ExactSelectorGenerationMemorySearchV1::attach(
        Arc::from(bytes),
        &workspace_identity(),
        generation_digest,
        fixture_digest,
    )
    .expect("attach");
    let (hit, receipt) = search
        .resolve_with_receipt("rust://src/lib.rs#item/function/run")
        .expect("resolve");
    assert_eq!(hit.projection, b"fn run() {}");
    assert!(receipt.hit);
    assert_eq!(receipt.subprocesses, 0);
    assert_eq!(receipt.db_opens, 0);
    assert_eq!(receipt.source_files_read, 0);
    assert_eq!(receipt.source_bytes_read, 0);
    assert_eq!(receipt.manifest_writes, 0);
}

#[test]
fn exact_memory_search_publishes_one_content_addressed_artifact_for_parallel_writers() {
    let _publication_resource = super::performance_gate::lock();
    let generation_digest = digest(4);
    let bytes = build_exact_selector_generation_fixture_v1(
        &ExactSelectorGenerationIdentityV1 {
            workspace_identity_digest: *workspace_identity().identity_digest(),
            language_id: "rust".to_string(),
            provider_id: "asp-rust".to_string(),
            workspace_root_digest: digest(1),
            parser_identity_digest: digest(2),
            query_pack_digest: digest(3),
            generation_digest,
            selector_count: 1,
            owner_count: 1,
            leaf_count: 1,
        },
        vec![ExactSelectorGenerationRecordV1 {
            structural_selector: "rust://src/lib.rs#item/function/run".to_string(),
            owner_path: "src/lib.rs".to_string(),
            owner_subtree_digest: digest(5),
            source_blob_digest: digest(6),
            normalized_parser_facts_digest: digest(7),
            projection_mode: ExactSelectorProjectionModeV1::Source,
            source_byte_range: 0..11,
            projection: b"fn run() {}".to_vec(),
        }],
    )
    .expect("fixture");
    let fixture_digest = *fixture_digest_v1(&bytes).expect("fixture digest");
    let generation_directory = std::env::temp_dir().join(format!(
        "asp-exact-selector-generation-fixture-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let bytes = Arc::<[u8]>::from(bytes);
    let writers = (0..8)
        .map(|_| {
            let bytes = Arc::clone(&bytes);
            let generation_directory = generation_directory.clone();
            std::thread::spawn(move || {
                publish_immutable_exact_selector_generation_fixture_v1(
                    ExactSelectorFixturePublicationV1 {
                        generation_directory: &generation_directory,
                        fixture: bytes.as_ref(),
                        workspace_identity: &workspace_identity(),
                        generation_digest,
                        fixture_digest: *fixture_digest_v1(bytes.as_ref()).expect("fixture digest"),
                    },
                )
                .expect("publish")
            })
        })
        .collect::<Vec<_>>();
    let paths = writers
        .into_iter()
        .map(|writer| writer.join().expect("writer"))
        .collect::<Vec<_>>();
    assert!(paths.windows(2).all(|window| window[0] == window[1]));
    let search = ExactSelectorGenerationMemorySearchV1::load_immutable_artifact(
        &paths[0],
        &workspace_identity(),
        generation_digest,
        fixture_digest,
    )
    .expect("load immutable artifact");
    assert!(
        search
            .resolve("rust://src/lib.rs#item/function/run")
            .is_ok()
    );
    std::fs::remove_dir_all(generation_directory).expect("cleanup generation directory");
}

#[test]
fn exact_memory_search_rejects_a_fixture_from_another_workspace_identity() {
    let generation_digest = digest(4);
    let bytes = build_exact_selector_generation_fixture_v1(
        &ExactSelectorGenerationIdentityV1 {
            workspace_identity_digest: *workspace_identity().identity_digest(),
            language_id: "rust".to_string(),
            provider_id: "asp-rust".to_string(),
            workspace_root_digest: digest(1),
            parser_identity_digest: digest(2),
            query_pack_digest: digest(3),
            generation_digest,
            selector_count: 1,
            owner_count: 1,
            leaf_count: 1,
        },
        vec![ExactSelectorGenerationRecordV1 {
            structural_selector: "rust://src/lib.rs#item/function/run".to_string(),
            owner_path: "src/lib.rs".to_string(),
            owner_subtree_digest: digest(5),
            source_blob_digest: digest(6),
            normalized_parser_facts_digest: digest(7),
            projection_mode: ExactSelectorProjectionModeV1::Source,
            source_byte_range: 0..11,
            projection: b"fn run() {}".to_vec(),
        }],
    )
    .expect("fixture");
    let fixture_digest = *fixture_digest_v1(&bytes).expect("fixture digest");
    let other_workspace = workspace_identity_with_snapshot(digest(9));
    let error = ExactSelectorGenerationMemorySearchV1::attach(
        Arc::from(bytes),
        &other_workspace,
        generation_digest,
        fixture_digest,
    )
    .expect_err("workspace mismatch");
    assert_eq!(
        error,
        agent_semantic_search::exact_selector_generation_fixture::ExactSelectorGenerationSearchErrorV1::WorkspaceIdentityMismatch(
            "sourceSnapshotRootDigest"
        )
    );
}
