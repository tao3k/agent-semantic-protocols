// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

#[tokio::test]
async fn db_engine_rejects_uncommitted_state_core_path_without_creating_project_shell() {
    let state_home = temp_root("db-engine-uncommitted-state-home");
    let projects_by_id = state_home.join("projects/by-id");
    let client_dir = projects_by_id
        .join("repo-uncommitted")
        .join("workspaces/workspace-uncommitted/live/client");

    let Err(error) = ClientDbEngine::open_write_session_client_dir(&client_dir) else {
        panic!("naked DB path must not materialize State Core ancestors");
    };

    assert!(error.contains("state-core-materialization-required"));
    assert!(
        !projects_by_id.exists(),
        "rejected DB write must create no project registry shell"
    );
}

#[tokio::test]
async fn db_engine_write_session_imports_manifest_without_exposing_removed_db_handle() {
    let project_root = temp_root("db-engine-write-session-project");
    let state_home = temp_root("db-engine-write-session-state-home");
    init_git_repository(&project_root);
    let state = ResolvedState::resolve_with_state_home(&project_root, &state_home)
        .expect("resolve state with explicit state home");
    let workspace_paths = state
        .ensure_workspace_state_layout()
        .expect("commit State Core identity before DB write");
    fs::create_dir_all(project_root.join("src")).expect("create src dir");
    fs::write(
        project_root.join("src/lib.rs"),
        "pub fn cached_fixture() {}\n",
    )
    .expect("write source fixture");
    let manifest: ClientCacheManifest = serde_json::from_value(json!({
        "schemaId": "agent.semantic-protocols.client-cache-manifest",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.client",
        "protocolVersion": "1",
        "cacheRoot": workspace_paths.root.display().to_string(),
        "generations": [
            {
                "generationId": "rust-main-1",
                "languageId": "rust",
                "providerId": "asp-rust",
                "providerVersion": "0.1.0",
                "exportMethod": "search/playbook",
                "projectRoot": project_root.display().to_string(),
                "packageRoot": ".",
                "schemaIds": ["agent.semantic-protocols.workspace-search-playbook-result"],
                "cacheStatus": "hit",
                "rawSourceStored": false,
                "requestFingerprint": "fnv64:write-session",
                "fileHashes": [{
                    "path": "src/lib.rs",
                    "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
                    "byteLen": 1,
                    "mtimeMs": 1
                }],
                "artifactIds": ["search/rust-main-1.json"]
            }
        ]
    }))
    .expect("manifest fixture");

    let mut write_session = ClientDbEngine::open_write_session_client_dir(&workspace_paths.root)
        .expect("open DB Engine write session");
    write_session
        .import_manifest(&manifest)
        .expect("import manifest through DB Engine write session");
    let write_report = write_session.inspect().expect("inspect write session");
    assert_eq!(
        write_report.status,
        agent_semantic_client_db::ClientDbStatus::Present
    );
    assert!(ClientDbEngine::turso_path_for_client_dir(&workspace_paths.root).exists());

    let read_session = ClientDbEngine::open_read_session_client_dir(&workspace_paths.root)
        .expect("open DB Engine read session")
        .expect("read session exists");
    let hit = read_session
        .lookup_generation_request(
            &LanguageId::from("rust"),
            &ProviderId::from("asp-rust"),
            &project_root,
            &CacheExportMethod::from("search/playbook"),
            Some("fnv64:write-session".to_string()),
        )
        .expect("lookup generation through DB Engine read session")
        .expect("generation hit");
    assert_eq!(hit.artifact_ids.len(), 1);
    assert_eq!(hit.artifact_ids[0].as_str(), "search/rust-main-1.json");

    let engine = ClientDbEngine::from_resolved_state(&state);
    assert!(
        engine.db_path().exists(),
        "write-session import must materialize active Turso cache-generation read model"
    );
    let mut write_session = ClientDbEngine::open_write_session_client_dir(&workspace_paths.root)
        .expect("open DB Engine write session for Turso invalidate");
    let invalidated = write_session
        .invalidate_generations_for_project(&project_root)
        .expect("invalidate DB Engine cache-generation rows");
    assert_eq!(invalidated, 1);
    let read_session = ClientDbEngine::open_read_session_client_dir(&workspace_paths.root)
        .expect("open DB Engine read session after Turso invalidation")
        .expect("read session exists after Turso invalidation");
    let miss = read_session
        .lookup_generation_request(
            &LanguageId::from("rust"),
            &ProviderId::from("asp-rust"),
            &project_root,
            &CacheExportMethod::from("search/playbook"),
            Some("fnv64:write-session".to_string()),
        )
        .expect("lookup generation after DB Engine invalidation");
    assert_eq!(miss, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn db_engine_cache_status_survives_concurrent_read_write_smoke() {
    let project_root = temp_root("db-engine-cache-status-concurrent-project");
    let state_home = temp_root("db-engine-cache-status-concurrent-state-home");
    init_git_repository(&project_root);
    let state = ResolvedState::resolve_with_state_home(&project_root, &state_home)
        .expect("resolve state with explicit state home");
    let workspace_paths = state
        .ensure_workspace_state_layout()
        .expect("commit State Core identity before DB write");
    ClientDbEngine::from_resolved_state(&state)
        .bootstrap_active_turso()
        .await
        .expect("bootstrap the Turso 0.7 format receipt before concurrent admission");
    fs::create_dir_all(project_root.join("src")).expect("create src dir");
    fs::write(
        project_root.join("src/lib.rs"),
        "pub fn concurrent_cache_status_fixture() {}\n",
    )
    .expect("write source fixture");

    let client_dir = Arc::new(workspace_paths.root);
    let project_root = Arc::new(project_root);
    let reader_count = 6usize;
    let barrier = Arc::new(Barrier::new(reader_count + 1));
    let mut readers = Vec::new();

    for reader_id in 0..reader_count {
        let barrier = Arc::clone(&barrier);
        let client_dir = Arc::clone(&client_dir);
        let project_root = Arc::clone(&project_root);
        readers.push(thread::spawn(move || {
            barrier.wait();
            for _ in 0..12 {
                let report = ClientDbEngine::inspect_client_dir(client_dir.as_path());
                assert_eq!(
                    report.db_path.file_name().and_then(|name| name.to_str()),
                    Some("facts.turso")
                );
                let read_session =
                    ClientDbEngine::open_read_session_client_dir(client_dir.as_path())
                        .expect("open read session during concurrent cache status smoke");
                if let Some(read_session) = read_session {
                    let _ = read_session
                        .lookup_generation_request(
                            &LanguageId::from("rust"),
                            &ProviderId::from("asp-rust"),
                            project_root.as_path(),
                            &CacheExportMethod::from("search/playbook"),
                            None,
                        )
                        .expect("lookup generation during concurrent cache status smoke");
                }
            }
            reader_id
        }));
    }

    barrier.wait();
    for generation_index in 0..8 {
        let manifest = concurrent_cache_status_manifest(
            client_dir.as_ref(),
            project_root.as_ref(),
            generation_index,
        );
        let mut write_session = ClientDbEngine::open_write_session_client_dir(client_dir.as_path())
            .expect("open write session during concurrent cache status smoke");
        write_session
            .import_manifest(&manifest)
            .expect("import manifest during concurrent cache status smoke");
        let write_report = write_session
            .inspect()
            .expect("inspect write session during concurrent cache status smoke");
        assert_eq!(
            write_report.status,
            agent_semantic_client_db::ClientDbStatus::Present
        );
    }

    for reader in readers {
        reader.join().expect("join cache status reader");
    }

    let read_session = ClientDbEngine::open_read_session_client_dir(client_dir.as_path())
        .expect("open final read session")
        .expect("final read session exists");
    let hit = read_session
        .lookup_generation_request(
            &LanguageId::from("rust"),
            &ProviderId::from("asp-rust"),
            project_root.as_path(),
            &CacheExportMethod::from("search/playbook"),
            Some("fnv64:cache-status-7".to_string()),
        )
        .expect("lookup final concurrent cache status generation")
        .expect("final concurrent cache status generation exists");
    assert_eq!(hit.artifact_ids.len(), 1);
    assert_eq!(hit.artifact_ids[0].as_str(), "search/cache-status-7.json");
}

fn concurrent_cache_status_manifest(
    client_dir: &Path,
    project_root: &Path,
    generation_index: usize,
) -> ClientCacheManifest {
    serde_json::from_value(json!({
        "schemaId": "agent.semantic-protocols.client-cache-manifest",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.client",
        "protocolVersion": "1",
        "cacheRoot": client_dir.display().to_string(),
        "generations": [
            {
                "generationId": format!("rust-cache-status-{generation_index}"),
                "languageId": "rust",
                "providerId": "asp-rust",
                "providerVersion": "0.1.0",
                "exportMethod": "search/playbook",
                "projectRoot": project_root.display().to_string(),
                "packageRoot": ".",
                "schemaIds": ["agent.semantic-protocols.workspace-search-playbook-result"],
                "cacheStatus": "hit",
                "rawSourceStored": false,
                "requestFingerprint": format!("fnv64:cache-status-{generation_index}"),
                "fileHashes": [{
                    "path": "src/lib.rs",
                    "sha256": "1111111111111111111111111111111111111111111111111111111111111111",
                    "byteLen": 1,
                    "mtimeMs": generation_index
                }],
                "artifactIds": [format!("search/cache-status-{generation_index}.json")]
            }
        ]
    }))
    .expect("concurrent cache status manifest fixture")
}

use super::fixture::init_git_repository;
use super::fixture::temp_root;
use agent_semantic_client_core::CacheExportMethod;
use agent_semantic_client_core::ClientCacheManifest;
use agent_semantic_client_core::LanguageId;
use agent_semantic_client_core::ProviderId;
use agent_semantic_client_core::state_core::ResolvedState;
use agent_semantic_client_db::ClientDbEngine;
use serde_json::json;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::Barrier;
use std::thread;
