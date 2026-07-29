use std::path::PathBuf;

use crate::source_index::search_pipe_source_index_lookup_from_client_result;
use agent_semantic_client_core::{LanguageId, ProviderId};
use agent_semantic_client_db::{
    ClientDbSourceIndexCandidate, ClientDbSourceIndexLookupResult, ClientDbSourceIndexLookupState,
    ClientDbSourceIndexSourceKind,
};

#[test]
fn search_pipe_source_index_lookup_projection_preserves_payload_proof() {
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
                selector_proof: Some(
                    agent_semantic_content_identity::ExactSelectorMaterializationProofV1 {
                        language_id: "rust".to_string(),
                        provider_id: "rs-harness".to_string(),
                        canonical_item_selector:
                            agent_semantic_content_identity::CanonicalItemSelector::parse(
                                "rust://src/lib.rs#item/function/owner",
                            )
                            .expect("canonical item selector"),
                        parser_identity_digest: [1; 32],
                        query_pack_digest: [2; 32],
                        workspace_root_digest: [3; 32],
                        owner_path: "src/lib.rs".to_string(),
                        owner_subtree_digest: [4; 32],
                        owner_inclusion_proof: Vec::new(),
                        source_blob_digest: [5; 32],
                        normalized_parser_facts_digest: [6; 32],
                        structural_selector: "rust://src/lib.rs#item/function/owner".to_string(),
                        projection_mode:
                            agent_semantic_content_identity::ExactSelectorProjectionModeV1::Source,
                        source_byte_start: 0,
                        source_byte_end: 10,
                        projection_digest: [7; 32],
                        projection: b"fn owner()".to_vec(),
                    },
                ),
            }],
            source_snapshot: Some(snapshot.clone()),
            index_artifact_digest: Some(index_artifact_digest.clone()),
        });

    let proof = lookup.candidates[0].selector_proof.as_ref().unwrap();
    assert_eq!(
        proof.structural_selector,
        "rust://src/lib.rs#item/function/owner"
    );
    assert_eq!(
        proof.projection_mode,
        agent_semantic_content_identity::ExactSelectorProjectionModeV1::Source
    );
    assert_eq!(proof.source_byte_start, 0);
    assert_eq!(proof.source_byte_end, 10);
    assert_eq!(proof.projection, b"fn owner()");
    assert_eq!(lookup.source_snapshot, Some(snapshot));
    assert_eq!(
        lookup
            .index_artifact_digest
            .as_ref()
            .map(|value| value.as_str()),
        Some(index_artifact_digest.as_str())
    );
}
