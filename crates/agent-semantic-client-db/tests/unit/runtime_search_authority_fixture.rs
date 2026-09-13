// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_client_db::runtime_server_workspace::read_search_generation_authority_fixture;

#[tokio::test]
async fn missing_authority_is_typed_active_generation_required() {
    let root = tempfile::tempdir().expect("fixture root");
    let pointer = root.path().join("generations").join("active.json");
    tokio::fs::create_dir_all(pointer.parent().expect("parent"))
        .await
        .expect("generation directory");
    let error =
        read_search_generation_authority_fixture(&pointer, 59, "repo-fixture", "workspace-fixture")
            .await
            .expect_err("missing authority must fail closed");
    assert!(error.contains("active-generation-required"), "{error}");
}

#[tokio::test]
async fn corrupt_authority_is_typed_authority_corrupt() {
    let root = tempfile::tempdir().expect("fixture root");
    let generations = root.path().join("generations");
    tokio::fs::create_dir_all(&generations)
        .await
        .expect("generation directory");
    let pointer = generations.join("active.json");
    tokio::fs::write(generations.join("search-authority-59.json"), b"not-json")
        .await
        .expect("corrupt authority");
    let error =
        read_search_generation_authority_fixture(&pointer, 59, "repo-fixture", "workspace-fixture")
            .await
            .expect_err("corrupt authority must fail closed");
    assert!(error.contains("authority-corrupt"), "{error}");
}
