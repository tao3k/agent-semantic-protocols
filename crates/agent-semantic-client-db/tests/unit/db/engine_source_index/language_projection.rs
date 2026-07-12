use std::fs;

use agent_semantic_client_core::{CacheGenerationId, LanguageId};
use agent_semantic_client_db::{
    ClientDbEngine, ClientDbLanguageProjection, ClientDbLanguageProjectionImportRequest,
    source_index_import_from_language_projection,
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
            generation_id: CacheGenerationId::from("language-projection-turso"),
            project_root: project_root.clone(),
            previous_file_hashes: None,
            registry_fingerprint: "language-projection-registry".to_string(),
            projection: projection.clone(),
        })
        .expect("assemble language projection import");
    assert_eq!(import.owners.len(), 1);
    assert_eq!(import.selectors.len(), 1);
    assert_eq!(import.selectors[0].start_line, 0);
    assert_eq!(import.selectors[0].end_line, 0);

    let report = ClientDbEngine::persist_language_projection_read_model_from_client_dir(
        &client_dir,
        &import,
        &projection,
    )
    .expect("persist language projection import");
    assert_eq!(report.graph_entity_count, 3);
    assert_eq!(report.graph_edge_count, 2);
    let language_id = LanguageId::from("gerbil-scheme");
    let graph_selectors = ClientDbEngine::lookup_graph_owner_selectors_from_client_dir(
        &client_dir,
        "src/projection.ss",
        Some(&language_id),
        8,
    )
    .await
    .expect("lookup imported graph selector");
    assert_eq!(graph_selectors.len(), 1);
    assert_eq!(graph_selectors[0].label, "run");
    assert_eq!(
        graph_selectors[0].semantic_kind.as_deref(),
        Some("function")
    );
    assert_eq!(
        graph_selectors[0].selector.as_deref(),
        Some("gerbil-scheme://src/projection.ss#item/function/run")
    );
    let graph_owner = ClientDbEngine::lookup_graph_owner_read_model_from_client_dir(
        &client_dir,
        "src/projection.ss",
        Some(&language_id),
        8,
    )
    .await
    .expect("lookup imported graph owner read model");
    assert!(graph_owner.projection_ready);
    assert_eq!(graph_owner.selector_nodes, graph_selectors);
    let lookup = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
        &client_dir,
        "run",
        Some(&language_id),
        8,
    )
    .await
    .expect("lookup imported projection");
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
    let _ = fs::remove_dir_all(project_root);
}
