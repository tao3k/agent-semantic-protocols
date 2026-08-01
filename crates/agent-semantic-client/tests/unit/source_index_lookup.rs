use std::path::PathBuf;

use crate::source_index::search_pipe_source_index_lookup_from_client_result;
use agent_semantic_client_core::{LanguageId, ProviderId};
use agent_semantic_client_db::{
    ClientDbSourceIndexCandidate, ClientDbSourceIndexLookupResult, ClientDbSourceIndexLookupState,
    ClientDbSourceIndexSourceKind,
};
use agent_semantic_content_identity::{
    CanonicalItemSelector,
    exact_selector_merkle::{ExactProjectionModeV1, blake3_content_digest_v1},
    exact_selector_projection_packet::{
        ExactSelectorProjectionPacketV1Input, ProjectionPacketExecutionCommandDigestV1,
        ProjectionPacketLanguageIdV1, ProjectionPacketOwnerPathV1, ProjectionPacketProviderIdV1,
        ProjectionPacketSemanticRegistryDigestV1, ProjectionPacketStructuralSelectorV1,
        build_exact_selector_projection_packet_v1, derive_parser_identity_digest_v1,
        derive_query_pack_identity_digest_v1,
    },
    workspace_merkle_v1::WorkspacePathMerkleTreeV1,
};

#[test]
fn search_pipe_source_index_lookup_projection_preserves_payload_proof() {
    const OWNER_PATH: &str = "src/lib.rs";
    const SELECTOR: &str = "rust://src/lib.rs#item/function/owner";
    const SOURCE: &[u8] = b"fn owner()\n";
    let language_id = ProjectionPacketLanguageIdV1::from("rust");
    let provider_id = ProjectionPacketProviderIdV1::from("rs-harness");
    let owner_path = ProjectionPacketOwnerPathV1::from(OWNER_PATH);
    let structural_selector = ProjectionPacketStructuralSelectorV1::from(SELECTOR);
    let parser_identity_digest = derive_parser_identity_digest_v1(
        &provider_id,
        &ProjectionPacketExecutionCommandDigestV1::from("test-execution-command"),
        &ProjectionPacketSemanticRegistryDigestV1::from("test-semantic-registry"),
    );
    let query_pack_digest = derive_query_pack_identity_digest_v1(b"rust-test-query-pack");
    let workspace_tree = WorkspacePathMerkleTreeV1::from_file_digests([(
        OWNER_PATH.to_owned(),
        blake3_content_digest_v1(SOURCE),
    )])
    .expect("workspace Merkle tree");
    let selector_projection =
        build_exact_selector_projection_packet_v1(ExactSelectorProjectionPacketV1Input {
            language_id: &language_id,
            provider_id: &provider_id,
            canonical_item_selector: CanonicalItemSelector::parse(SELECTOR)
                .expect("canonical item selector"),
            parser_identity_digest: &parser_identity_digest,
            query_pack_digest: &query_pack_digest,
            owner_path: &owner_path,
            structural_selector: &structural_selector,
            projection_mode: ExactProjectionModeV1::Code,
            source_byte_start: 0,
            source_byte_end: 10,
            source: SOURCE,
            normalized_parser_facts: b"kind=function;name=owner",
            projection: &SOURCE[..10],
        })
        .enrich_projection_record(&workspace_tree)
        .expect("canonical exact-selector projection record");
    let snapshot = agent_semantic_content_identity::SourceSnapshotEvidence::new(
        "sha256:test-source-snapshot",
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        1,
        "sha256:test-provider",
    );
    let index_artifact_digest =
        agent_semantic_client_db::client_db_source_index_artifact_digest(&snapshot);
    let lookup =
        search_pipe_source_index_lookup_from_client_result(ClientDbSourceIndexLookupResult {
            db_path: PathBuf::from("live/client/client.turso"),
            state: ClientDbSourceIndexLookupState::Hit,
            candidates: vec![ClientDbSourceIndexCandidate {
                path: "src/lib.rs".to_string().into(),
                language_id: Some(LanguageId::from("rust")),
                provider_id: Some(ProviderId::from("rs-harness")),
                source_kind: ClientDbSourceIndexSourceKind::File,
                line_count: Some(12),
                query_keys: vec!["owner".to_string().into()],
                selector_kind: None,
                selector_symbol: None,
                selector_projection: Some(selector_projection),
            }],
            source_snapshot: Some(snapshot.clone()),
            index_artifact_digest: Some(index_artifact_digest.clone()),
        });

    let proof = lookup.candidates[0]
        .selector_projection
        .as_ref()
        .expect("selector projection");
    assert_eq!(
        proof.proof.structural_selector(),
        "rust://src/lib.rs#item/function/owner"
    );
    assert_eq!(
        proof.proof.projection_mode(),
        &agent_semantic_content_identity::exact_selector_merkle::ExactProjectionModeV1::Code
    );
    assert_eq!(proof.source_byte_range, 0..10);
    assert_eq!(proof.projection_payload, b"fn owner()");
    assert_eq!(lookup.source_snapshot, Some(snapshot));
    assert_eq!(
        lookup
            .index_artifact_digest
            .as_ref()
            .map(|value| value.as_str()),
        Some(index_artifact_digest.as_str())
    );
}
