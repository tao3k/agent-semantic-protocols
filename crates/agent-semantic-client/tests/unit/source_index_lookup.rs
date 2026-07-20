use std::path::PathBuf;

use crate::source_index::search_pipe_source_index_lookup_from_client_result;
use agent_semantic_client_core::{LanguageId, ProviderId};
use agent_semantic_client_db::{
    ClientDbSourceIndexCandidate, ClientDbSourceIndexLookupResult, ClientDbSourceIndexLookupState,
    ClientDbSourceIndexSelectorPayloadProof, ClientDbSourceIndexSourceKind,
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
                path: "src/lib.rs".to_string(),
                language_id: Some(LanguageId::from("rust")),
                provider_id: Some(ProviderId::from("rs-harness")),
                source_kind: ClientDbSourceIndexSourceKind::File,
                line_count: Some(12),
                query_keys: vec!["owner".to_string()],
                selector_kind: None,
                selector_symbol: None,
                selector_proof: Some(ClientDbSourceIndexSelectorPayloadProof {
                    structural_selector: "rust://src/lib.rs#item/function/owner".to_string(),
                    payload_kind: "code".to_string(),
                    bounded: true,
                }),
            }],
            source_snapshot: Some(snapshot.clone()),
            index_artifact_digest: Some(index_artifact_digest.clone()),
        });

    let proof = lookup.candidates[0].selector_proof.as_ref().unwrap();
    assert_eq!(
        proof.structural_selector,
        "rust://src/lib.rs#item/function/owner"
    );
    assert_eq!(proof.payload_kind, "code");
    assert!(proof.bounded);
    assert_eq!(lookup.source_snapshot, Some(snapshot));
    assert_eq!(lookup.index_artifact_digest, Some(index_artifact_digest));
}
