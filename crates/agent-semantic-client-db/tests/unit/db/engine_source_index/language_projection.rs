use std::fs;

use agent_semantic_client_core::LanguageId;
use agent_semantic_client_db::{
    ClientDbEngine, ClientDbLanguageProjection, ClientDbLanguageProjectionImportRequest,
    source_index_import_from_language_projection,
};
use agent_semantic_content_identity::{
    DerivedArtifactAuthorityState, SourceSnapshotEvidence, SourceSnapshotKind,
};

use super::temp_root;

#[tokio::test(flavor = "current_thread")]
async fn harness_projection_imports_without_source_text_projection() {
    let client_dir = temp_root("db-language-projection-client");
    let project_root = temp_root("db-language-projection-project");
    let source_path = project_root.join("src/projection.ss");
    fs::create_dir_all(source_path.parent().expect("source parent")).expect("create source dir");
    fs::write(&source_path, "(def (run) 1)\n").expect("write source fixture");
    let projection = ClientDbLanguageProjection::from_json(
        r#"{
          "schemaId":"agent.semantic-protocols.semantic-language-projection",
          "schemaVersion":"1",
          "protocolId":"agent.semantic-protocols.language-projection",
          "protocolVersion":"1",
          "languageId":"gerbil-scheme",
          "harness":{"harnessId":"gerbil-scheme-language-project-harness","parserAbi":"gerbil-parser-v1","selectorDialect":"gerbil-scheme"},
          "sources":[{"sourceId":"source:src/projection.ss","path":"src/projection.ss","sourceKind":"source"}],
          "owners":[{"ownerId":"owner:src/projection.ss","sourceId":"source:src/projection.ss","kind":"module","name":"projection"}],
          "items":[{"itemId":"item:run","ownerId":"owner:src/projection.ss","kind":"function","name":"run","selector":"gerbil-scheme://src/projection.ss#item/function/run"}],
          "relations":[
            {"from":{"kind":"source","id":"source:src/projection.ss"},"kind":"contains","to":{"kind":"owner","id":"owner:src/projection.ss"}},
            {"from":{"kind":"owner","id":"owner:src/projection.ss"},"kind":"contains","to":{"kind":"item","id":"item:run"}}
          ]
        }"#,
    )
    .expect("decode language projection");
    let import =
        source_index_import_from_language_projection(ClientDbLanguageProjectionImportRequest {
            project_root: project_root.clone(),
            previous_file_hashes: None,
            registry_fingerprint: "language-projection-registry".to_string(),
            projection: projection.clone(),
        })
        .expect("assemble language projection import");
    let source_snapshot = import.source_snapshot.clone();
    assert_eq!(
        import.source_index.generation_id,
        agent_semantic_client_db::client_db_source_index_generation_id_for_snapshot(
            &source_snapshot,
        ),
    );
    let import_counts = (
        import.source_index.file_hashes.len(),
        import.source_index.owners.len(),
        import.source_index.selectors.len(),
    );
    assert_eq!(
        import.source_index.file_hashes.len(),
        source_snapshot.leaf_count,
        "language projection source-index counts={import_counts:?}"
    );
    assert_eq!(
        import.source_index.owners.len(),
        1,
        "language projection source-index counts={import_counts:?}"
    );
    assert_eq!(
        import.source_index.selectors.len(),
        1,
        "language projection source-index counts={import_counts:?}"
    );
    assert_eq!(import.source_index.selectors[0].start_line, 0);
    assert_eq!(import.source_index.selectors[0].end_line, 0);
    assert_eq!(
        import.source_index.owners[0]
            .provider_id
            .as_ref()
            .map(|id| id.as_str()),
        Some("gerbil-scheme-language-project-harness"),
    );

    let report = ClientDbEngine::persist_language_projection_read_model_from_client_dir(
        &client_dir,
        &import.source_index,
        &projection,
        &source_snapshot,
    )
    .expect("persist language projection import");
    assert_eq!(report.graph_entity_count, 3);
    assert_eq!(report.graph_edge_count, 2);
    assert!(!report.graph_artifact_digest.is_empty());
    let language_id = LanguageId::from("gerbil-scheme");
    let graph_owner = ClientDbEngine::lookup_graph_owner_read_model_from_client_dir(
        &client_dir,
        &source_snapshot,
        "src/projection.ss",
        Some(&language_id),
        8,
    )
    .await
    .expect("lookup imported graph owner read model");
    assert_eq!(
        graph_owner.artifact_evidence.authority_state,
        DerivedArtifactAuthorityState::Current
    );
    assert!(graph_owner.owner_present);
    assert_eq!(graph_owner.selector_nodes.len(), 1);
    assert_eq!(graph_owner.selector_nodes[0].label, "run");
    assert_eq!(
        graph_owner.selector_nodes[0].semantic_kind.as_deref(),
        Some("function")
    );
    assert_eq!(
        graph_owner.selector_nodes[0].selector.as_deref(),
        Some("gerbil-scheme://src/projection.ss#item/function/run")
    );
    assert_eq!(
        graph_owner
            .artifact_evidence
            .resolved_artifact_digest
            .as_deref(),
        Some(report.graph_artifact_digest.as_str())
    );
    assert_eq!(
        graph_owner.artifact_evidence.source_snapshot,
        source_snapshot
    );
    let stale_snapshot = SourceSnapshotEvidence::new(
        "c".repeat(64),
        SourceSnapshotKind::Filesystem,
        1,
        source_snapshot.provider_digest.clone(),
    );
    let stale_owner = ClientDbEngine::lookup_graph_owner_read_model_from_client_dir(
        &client_dir,
        &stale_snapshot,
        "src/projection.ss",
        Some(&language_id),
        8,
    )
    .await
    .expect("stale graph artifact must degrade without returning selectors");
    assert_eq!(
        stale_owner.artifact_evidence.authority_state,
        DerivedArtifactAuthorityState::Stale
    );
    assert!(!stale_owner.owner_present);
    assert!(stale_owner.selector_nodes.is_empty());
    assert_eq!(
        stale_owner.artifact_evidence.source_snapshot,
        stale_snapshot
    );
    assert!(
        stale_owner
            .artifact_evidence
            .resolved_artifact_digest
            .is_none()
    );

    let missing_client_dir = temp_root("db-language-projection-missing-client");
    let missing_owner = ClientDbEngine::lookup_graph_owner_read_model_from_client_dir(
        &missing_client_dir,
        &source_snapshot,
        "src/projection.ss",
        Some(&language_id),
        8,
    )
    .await
    .expect("missing graph cache must remain a non-authoritative cache miss");
    assert_eq!(
        missing_owner.artifact_evidence.authority_state,
        DerivedArtifactAuthorityState::Missing
    );
    assert!(!missing_owner.owner_present);
    assert!(missing_owner.selector_nodes.is_empty());
    let lookup = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
        &client_dir,
        &source_snapshot,
        "run",
        Some(&language_id),
        8,
    )
    .await
    .expect("lookup imported projection");
    assert_eq!(lookup.source_snapshot.as_ref(), Some(&source_snapshot));
    assert!(lookup.index_artifact_digest.is_some());
    let proof = lookup
        .candidates
        .iter()
        .find(|candidate| candidate.path == "src/projection.ss")
        .and_then(|candidate| candidate.selector_proof.as_ref())
        .expect("structural selector proof");
    assert_eq!(
        proof.structural_selector,
        "gerbil-scheme://src/projection.ss#item/function/run"
    );
    assert!(proof.bounded);
    let candidate = lookup
        .candidates
        .iter()
        .find(|candidate| candidate.path == "src/projection.ss")
        .expect("projection candidate");
    assert_eq!(candidate.selector_symbol.as_deref(), Some("run"));
    assert_eq!(candidate.selector_kind.as_deref(), Some("function"));

    let _ = fs::remove_dir_all(client_dir);
    let _ = fs::remove_dir_all(missing_client_dir);
    let _ = fs::remove_dir_all(project_root);
}
