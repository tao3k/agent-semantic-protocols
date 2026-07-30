use std::fs;

use agent_semantic_client_core::LanguageId;
use agent_semantic_client_db::{
    ClientDbLanguageProjection, ClientDbLanguageProjectionImportRequest,
    source_index_import_from_language_projection,
};

use super::temp_root;

#[tokio::test(flavor = "current_thread")]
async fn harness_projection_imports_without_source_text_projection() {
    let project_root = temp_root("db-language-projection-project");
    let source_path = project_root.join("src/projection.ss");
    fs::create_dir_all(source_path.parent().expect("source parent")).expect("create source dir");
    fs::write(&source_path, "(def (run) 1)\n").expect("write source fixture");
    let projection_json = r#"{
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
        }"#;
    let mut projection_value: serde_json::Value =
        serde_json::from_str(projection_json).expect("decode projection fixture JSON");
    let source = b"(def (run) 1)\n";
    projection_value["items"][0]["materializationProof"] =
        serde_json::to_value(crate::materialization_fixture::materialization_proof(
            crate::materialization_fixture::MaterializationFixtureInput {
                language_id: "gerbil-scheme",
                provider_id: "gerbil-scheme-language-project-harness",
                owner_path: "src/projection.ss",
                structural_selector: "gerbil-scheme://src/projection.ss#item/function/run",
                item_kind: "function",
                item_name: "run",
                source,
                source_byte_start: 0,
                source_byte_end: source.len() as u64,
            },
        ))
        .expect("encode projection materialization proof");
    let projection_text =
        serde_json::to_string(&projection_value).expect("encode projection fixture");
    let projection = ClientDbLanguageProjection::from_json(&projection_text)
        .expect("decode language projection");
    let source_blobs =
        agent_semantic_client_db::ClientDbSourceIndexSourceBlobs::from_normalized([(
            agent_semantic_client_db::ClientDbSourceIndexPath::new("src/projection.ss"),
            b"(def (run) 1)\n".to_vec(),
        )]);
    let import =
        source_index_import_from_language_projection(ClientDbLanguageProjectionImportRequest {
            project_root: project_root.clone(),
            registry_fingerprint: "language-projection-registry".to_string(),
            projection: projection.clone(),
            source_blobs: source_blobs.clone(),
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
    assert_eq!(
        import.source_index.selectors[0]
            .materialization_proof
            .source_byte_start,
        0
    );
    assert_eq!(
        import.source_index.selectors[0]
            .materialization_proof
            .source_byte_end,
        b"(def (run) 1)\n".len() as u64
    );
    assert_eq!(
        import.source_index.owners[0]
            .provider_id
            .as_ref()
            .map(|id| id.as_str()),
        Some("gerbil-scheme-language-project-harness"),
    );

    let fixture = agent_semantic_client_db::fixture::SourceIndexFixture::default();
    fixture
        .commit_source_index_generation(
            agent_semantic_client_db::ClientDbSourceIndexRefreshRequest {
                file_count: import.source_index.file_hashes.len().min(u32::MAX as usize) as u32,
                import: import.source_index.clone(),
                source_snapshot: source_snapshot.clone(),
            },
            &source_blobs,
        )
        .expect("persist language projection import");
    let language_id = LanguageId::from("gerbil-scheme");
    let lookup = fixture
        .read_source_index(
            project_root.clone(),
            project_root.clone(),
            source_snapshot.clone(),
            "run".to_owned(),
            Some(language_id),
            8,
        )
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
    assert_eq!(proof.source_byte_start, 0);
    assert_eq!(proof.source_byte_end, b"(def (run) 1)\n".len() as u64);
    assert_eq!(proof.projection, b"(def (run) 1)\n");
    let candidate = lookup
        .candidates
        .iter()
        .find(|candidate| candidate.path == "src/projection.ss")
        .expect("projection candidate");
    assert_eq!(candidate.selector_symbol.as_deref(), Some("run"));
    assert_eq!(
        candidate.selector_kind.as_ref().map(|kind| kind.as_str()),
        Some("function")
    );

    let _ = fs::remove_dir_all(project_root);
}
