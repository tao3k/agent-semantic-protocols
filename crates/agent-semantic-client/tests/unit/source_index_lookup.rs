use std::path::{Path, PathBuf};

use agent_semantic_client_core::{LanguageId, ProviderId};
use agent_semantic_client_db::{
    ClientDbSourceIndexCandidate, ClientDbSourceIndexLookupResult, ClientDbSourceIndexLookupState,
    ClientDbSourceIndexSelectorPayloadProof, ClientDbSourceIndexSourceKind,
};
use agent_semantic_search::QueryWrapperSearchSurface;

use crate::source_index::{
    lookup_query_wrapper_source_index, query_wrapper_source_index_lookup_from_client_result,
    search_pipe_source_index_lookup_from_client_result,
};

#[test]
fn query_wrapper_source_index_lookup_skips_empty_terms() {
    let lookup =
        lookup_query_wrapper_source_index(QueryWrapperSearchSurface::Fd, Path::new("."), &[])
            .expect("empty query-wrapper source-index lookup");

    assert!(lookup.is_none());
}

#[test]
fn query_wrapper_source_index_lookup_projection_preserves_client_fields() {
    let lookup =
        query_wrapper_source_index_lookup_from_client_result(ClientDbSourceIndexLookupResult {
            db_path: PathBuf::from("live/client/client.turso"),
            state: ClientDbSourceIndexLookupState::Hit,
            candidates: vec![ClientDbSourceIndexCandidate {
                path: "src/lib.rs".to_string(),
                language_id: Some(LanguageId::from("rust")),
                provider_id: Some(ProviderId::from("rs-harness")),
                source_kind: ClientDbSourceIndexSourceKind::File,
                line_count: Some(12),
                query_keys: vec!["query_wrapper_source_index".to_string()],
                selector_proof: None,
            }],
        });

    assert_eq!(lookup.db_path, PathBuf::from("live/client/client.turso"));
    assert_eq!(lookup.state, "hit");
    assert_eq!(lookup.candidates[0].path, "src/lib.rs");
    assert_eq!(lookup.candidates[0].language_id.as_deref(), Some("rust"));
    assert_eq!(
        lookup.candidates[0].provider_id.as_deref(),
        Some("rs-harness")
    );
    assert_eq!(lookup.candidates[0].source_kind, "file");
    assert_eq!(lookup.candidates[0].line_count, Some(12));
    assert_eq!(
        lookup.candidates[0].query_keys,
        vec!["query_wrapper_source_index".to_string()]
    );
}

#[test]
fn search_pipe_source_index_lookup_projection_preserves_payload_proof() {
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
                selector_proof: Some(ClientDbSourceIndexSelectorPayloadProof {
                    structural_selector: "rust://src/lib.rs#item/function/owner".to_string(),
                    payload_kind: "code".to_string(),
                    bounded: true,
                }),
            }],
        });

    let proof = lookup.candidates[0].selector_proof.as_ref().unwrap();
    assert_eq!(
        proof.structural_selector,
        "rust://src/lib.rs#item/function/owner"
    );
    assert_eq!(proof.payload_kind, "code");
    assert!(proof.bounded);
}
