// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use crate::source_index_fixture::build_fixture_source_index_import;

use agent_semantic_client_core::CacheGenerationId;
use agent_semantic_client_core::ClientCacheFileHash;
use agent_semantic_client_core::LanguageId;
use agent_semantic_client_core::ProviderId;
use agent_semantic_client_core::SemanticSchemaId;
use agent_semantic_client_core::SemanticSchemaVersion;
use agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_PROVIDER_ID;
use agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_SCHEMA_ID;
use agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION;
use agent_semantic_client_db::ClientDbSourceIndexImportAssemblyRequest;
use agent_semantic_client_db::ClientDbSourceIndexImportFile;
use agent_semantic_client_db::ClientDbSourceIndexImportRequest;
use agent_semantic_client_db::ClientDbSourceIndexRefreshRequest;
use agent_semantic_client_db::ClientDbSourceIndexScopeFile;
use agent_semantic_client_db::ClientDbSourceIndexSource;
use agent_semantic_client_db::client_db_source_index_file_count;
use agent_semantic_client_db::source_index_relative_path;
use agent_semantic_client_db::source_index_scope_dirs;

#[tokio::test]
async fn schema_version_stays_on_first_turso_release_contract() {
    assert_eq!(
        agent_semantic_client_db::AGENT_SEMANTIC_CLIENT_DB_SCHEMA_VERSION,
        1
    );
}

#[path = "db_cases/session_and_source_index.rs"]
mod session_and_source_index;

fn temp_root(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let repository = gix::discover(env!("CARGO_MANIFEST_DIR"))
        .expect("discover owner-backed database fixture repository with Gix");
    repository
        .worktree()
        .expect("database fixtures require a non-bare owner checkout")
        .base()
        .join("target/asp-live-project-fixtures")
        .join(format!("asp-client-db-{name}-{nanos}"))
}
