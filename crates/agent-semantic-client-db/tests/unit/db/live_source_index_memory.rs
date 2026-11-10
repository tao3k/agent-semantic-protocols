// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_client_core::LanguageId;
use agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_SCHEMA_ID;
use agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION;
use agent_semantic_client_db::ClientDbEngine;
use agent_semantic_client_db::ClientDbLiveSourceIndexFacts;
use agent_semantic_client_db::ClientDbSourceIndexClientDirLookupRequest;
use agent_semantic_client_db::ClientDbSourceIndexImport;
use agent_semantic_client_db::ClientDbSourceIndexLookupState;
use agent_semantic_client_db::ClientDbSourceIndexOwner;
use agent_semantic_client_db::client_db_source_index_generation_id_for_snapshot;
use std::fs;
use std::path::PathBuf;

fn temp_root() -> PathBuf {
    let nonce = format!(
        "asp-live-source-index-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    );
    let root = std::env::temp_dir().join(nonce);
    fs::create_dir_all(&root).expect("create test root");
    root
}

#[test]
fn live_source_index_hit_does_not_create_or_open_client_db() {
    let root = temp_root();
    let client_dir = root.join("missing-client");
    let project_root = root.join("project");
    fs::create_dir_all(&project_root).expect("create project root");
    let source_snapshot = agent_semantic_content_identity::SourceSnapshotEvidence {
        schema_id: "asp.source-snapshot.v1".to_string(),
        algorithm: "blake3-merkle-v1".to_string(),
        root_digest: "live-root".to_string(),
        source_kind: agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        leaf_count: 1,
        base_root_digest: None,
        provider_digest: "provider-digest".to_string(),
        dirty_paths_digest: None,
    };
    let source_index_import = ClientDbSourceIndexImport {
        source_blobs: Default::default(),
        relations: Vec::new(),
        generation_id: client_db_source_index_generation_id_for_snapshot(&source_snapshot),
        project_root: project_root.clone(),
        schema_id: CLIENT_DB_SOURCE_INDEX_SCHEMA_ID.into(),
        schema_version: CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION.into(),
        file_hashes: Vec::new(),
        owners: vec![ClientDbSourceIndexOwner {
            owner_path: "src/lib.rs".into(),
            language_id: Some("rust".into()),
            provider_id: Some("asp-rust".into()),
            source_kind: "file".into(),
            line_count: Some(1),
            query_keys: vec!["needle".into()],
        }],
        selectors: Vec::new(),
    };
    let expected_artifact_digest =
        agent_semantic_search_projection::source_index_artifact_digest(&source_snapshot);
    let rust_language_id = LanguageId::from("rust");

    let mut samples = Vec::with_capacity(128);
    for _ in 0..128 {
        let started = std::time::Instant::now();
        let lookup = ClientDbEngine::lookup_source_index_from_client_dir(
            ClientDbSourceIndexClientDirLookupRequest {
                client_dir: &client_dir,
                indexed_project_root: &project_root,
                language_id: Some(&rust_language_id),
                query_keys: vec!["needle".into()],
                limit: 8,
                expected_snapshot_root: source_snapshot.root_digest.as_str(),
                expected_index_artifact_digest: expected_artifact_digest.as_str(),
                live_facts: Some(ClientDbLiveSourceIndexFacts {
                    source_snapshot: &source_snapshot,
                    import: &source_index_import,
                }),
            },
        )
        .expect("live-memory lookup");
        samples.push(started.elapsed());
        assert_eq!(lookup.state, ClientDbSourceIndexLookupState::Hit);
        assert_eq!(lookup.candidates.len(), 1);
    }
    samples.sort_unstable();
    let p95 = samples[(samples.len() * 95 / 100).min(samples.len() - 1)];
    assert!(
        p95 <= std::time::Duration::from_millis(10),
        "live-memory source-index lookup p95 exceeded 10ms: {p95:?}"
    );
    assert!(
        !client_dir.exists(),
        "live-memory lookup must not create the client DB directory"
    );
    let _ = fs::remove_dir_all(root);
}
