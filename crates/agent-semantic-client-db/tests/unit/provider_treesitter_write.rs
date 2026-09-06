// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use std::sync::MutexGuard;

use super::{
    ProviderIncrementalScoped, ProviderOwnerInventoryEntry, ProviderOwnerInventoryEntryState,
    ProviderOwnerInventoryState, ProviderOwnerInventoryWrite, ProviderOwnerInventoryWriteReceipt,
    ProviderTreeSitterCaptureProjection, ProviderTreeSitterOwnerResult,
    ProviderTreeSitterOwnerResultState, ProviderTreeSitterQueryIdentity, scope_params,
};
use crate::engine::{ProviderSearchWorkspaceSession, WorkspaceDbRegistry};
use crate::test_support::{StateHomeGuard, TestDir, environment_lock, workspace};

#[tokio::test(flavor = "current_thread")]
async fn exact_rust_inventory_is_isolated_from_julia_and_gerbil_scopes() {
    let fixture = ProviderTreeSitterWriteFixture::new("scope-isolation").await;
    let rust = fixture.scope("rust", "asp-rust", '1');
    let julia = fixture.scope("julia", "asp-julia", '2');
    let gerbil = fixture.scope("gerbil-scheme", "asp-gerbil-scheme", '3');

    fixture
        .write_inventory(&julia, ProviderOwnerInventoryState::Known, &["src/a.jl"])
        .await;
    fixture
        .write_inventory(&gerbil, ProviderOwnerInventoryState::Known, &["src/a.ss"])
        .await;
    let receipt = fixture
        .write_inventory(&rust, ProviderOwnerInventoryState::Exact, &["src/lib.rs"])
        .await;

    assert_eq!(receipt.upserted_entry_count, 1);
    assert_eq!(receipt.deleted_entry_count, 0);
    assert_eq!(fixture.inventory_state(&rust).await, "exact");
    assert_eq!(fixture.inventory_entry_count(&rust).await, 1);
    assert_eq!(fixture.inventory_entry_count(&julia).await, 1);
    assert_eq!(fixture.inventory_entry_count(&gerbil).await, 1);
    assert_eq!(fixture.total_inventory_entry_count().await, 3);
}

#[tokio::test(flavor = "current_thread")]
async fn second_inventory_write_deletes_only_stale_rust_owner() {
    let fixture = ProviderTreeSitterWriteFixture::new("stale-owner").await;
    let rust = fixture.scope("rust", "asp-rust", '1');
    let julia = fixture.scope("julia", "asp-julia", '2');
    let gerbil = fixture.scope("gerbil-scheme", "asp-gerbil-scheme", '3');

    fixture
        .write_inventory(&julia, ProviderOwnerInventoryState::Known, &["src/a.jl"])
        .await;
    fixture
        .write_inventory(&gerbil, ProviderOwnerInventoryState::Known, &["src/a.ss"])
        .await;
    fixture
        .write_inventory(
            &rust,
            ProviderOwnerInventoryState::Exact,
            &["src/lib.rs", "src/stale.rs"],
        )
        .await;
    let julia_generation = fixture.inventory_generation(&julia).await;
    let gerbil_generation = fixture.inventory_generation(&gerbil).await;

    let receipt = fixture
        .write_inventory(&rust, ProviderOwnerInventoryState::Exact, &["src/lib.rs"])
        .await;

    assert_eq!(receipt.upserted_entry_count, 1);
    assert_eq!(receipt.deleted_entry_count, 1);
    assert_eq!(fixture.inventory_paths(&rust).await, vec!["src/lib.rs"]);
    assert_eq!(fixture.inventory_generation(&julia).await, julia_generation);
    assert_eq!(
        fixture.inventory_generation(&gerbil).await,
        gerbil_generation
    );
    assert_eq!(fixture.inventory_entry_count(&julia).await, 1);
    assert_eq!(fixture.inventory_entry_count(&gerbil).await, 1);
}

#[tokio::test(flavor = "current_thread")]
async fn one_owner_capture_set_is_atomically_replaced() {
    let fixture = ProviderTreeSitterWriteFixture::new("capture-replace").await;
    let scope = fixture.scope("rust", "asp-rust", '1');
    let inventory = fixture
        .write_inventory(&scope, ProviderOwnerInventoryState::Exact, &["src/lib.rs"])
        .await;
    let query = fixture.query(scope, 'a', &["declaration.name", "reference.name"]);
    let content_digest = digest('b');
    let first = fixture.result(
        &query,
        inventory.inventory_generation.as_str(),
        content_digest.as_str(),
        vec![
            fixture.capture("declaration.name", "pub fn example()", 0, 7),
            fixture.capture("reference.name", "example()", 8, 15),
        ],
    );
    fixture
        .session
        .write_provider_treesitter_owner_result(&query, &first)
        .await
        .expect("write initial owner captures");

    let replacement = fixture.result(
        &query,
        inventory.inventory_generation.as_str(),
        content_digest.as_str(),
        vec![fixture.capture("declaration.name", "pub fn replacement()", 0, 11)],
    );
    let receipt = fixture
        .session
        .write_provider_treesitter_owner_result(&query, &replacement)
        .await
        .expect("replace owner captures");

    assert_eq!(receipt.capture_projection_writes, 1);
    assert_eq!(
        fixture
            .capture_signatures(&query, content_digest.as_str())
            .await,
        vec!["pub fn replacement()"]
    );
    assert_eq!(
        fixture
            .owner_capture_count(&query, content_digest.as_str())
            .await,
        1
    );
}

#[tokio::test(flavor = "current_thread")]
async fn query_and_content_digest_keys_do_not_cross_results() {
    let fixture = ProviderTreeSitterWriteFixture::new("cache-keys").await;
    let scope = fixture.scope("rust", "asp-rust", '1');
    let inventory = fixture
        .write_inventory(&scope, ProviderOwnerInventoryState::Exact, &["src/lib.rs"])
        .await;
    let query_a = fixture.query(scope.clone(), 'a', &["declaration.name"]);
    let query_b = fixture.query(scope, 'b', &["declaration.name"]);
    let content_a = digest('c');
    let content_b = digest('d');

    for (query, content, signature) in [
        (&query_a, content_a.as_str(), "query-a-content-a"),
        (&query_a, content_b.as_str(), "query-a-content-b"),
        (&query_b, content_b.as_str(), "query-b-content-b"),
    ] {
        let result = fixture.result(
            query,
            inventory.inventory_generation.as_str(),
            content,
            vec![fixture.capture("declaration.name", signature, 0, 8)],
        );
        fixture
            .session
            .write_provider_treesitter_owner_result(query, &result)
            .await
            .expect("write cache-key fixture result");
    }

    assert_eq!(fixture.query_owner_row_count(&query_a.scope).await, 3);
    assert_eq!(
        fixture
            .capture_signatures(&query_a, content_a.as_str())
            .await,
        vec!["query-a-content-a"]
    );
    assert_eq!(
        fixture
            .capture_signatures(&query_a, content_b.as_str())
            .await,
        vec!["query-a-content-b"]
    );
    assert_eq!(
        fixture
            .capture_signatures(&query_b, content_b.as_str())
            .await,
        vec!["query-b-content-b"]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn invalid_completeness_signature_span_and_cache_key_fail_before_db_open() {
    for (label, invalid) in [
        ("completeness", InvalidWrite::Completeness),
        ("signature", InvalidWrite::Signature),
        ("span", InvalidWrite::Span),
        ("cache-key", InvalidWrite::CacheKey),
    ] {
        let fixture = ProviderTreeSitterWriteFixture::new(label).await;
        let scope = fixture.scope("rust", "asp-rust", '1');
        let query = fixture.query(scope, 'a', &["declaration.name"]);
        let mut result = fixture.result(
            &query,
            digest('b').as_str(),
            digest('c').as_str(),
            vec![fixture.capture("declaration.name", "pub fn example()", 0, 8)],
        );
        match invalid {
            InvalidWrite::Completeness => {
                result.state = ProviderTreeSitterOwnerResultState::Cached;
            }
            InvalidWrite::Signature => result.projections[0].signature.clear(),
            InvalidWrite::Span => result.projections[0].source_byte_end = 65,
            InvalidWrite::CacheKey => result.query_digest = digest('d'),
        }

        let transactions_before = fixture.registry.counters().writer_transaction_count;
        let error = fixture
            .session
            .write_provider_treesitter_owner_result(&query, &result)
            .await
            .expect_err("invalid writer request must fail");
        assert!(!error.is_empty());
        assert_eq!(
            fixture.registry.counters().writer_transaction_count,
            transactions_before,
            "{label} validation entered a writer transaction"
        );
        assert_eq!(fixture.query_owner_row_count(&query.scope).await, 0);
    }
}

#[derive(Clone, Copy)]
enum InvalidWrite {
    Completeness,
    Signature,
    Span,
    CacheKey,
}

struct ProviderTreeSitterWriteFixture {
    _state_home: StateHomeGuard,
    _temp: TestDir,
    _environment: MutexGuard<'static, ()>,
    base_scope: ProviderIncrementalScoped,
    registry: WorkspaceDbRegistry,
    session: ProviderSearchWorkspaceSession,
}

impl ProviderTreeSitterWriteFixture {
    async fn new(label: &str) -> Self {
        let environment = environment_lock();
        let temp = TestDir::new(label);
        let state_home = StateHomeGuard::install(&temp.path().join("state"));
        let (project_root, _resolved, base_scope) = workspace(temp.path(), label);
        let registry = WorkspaceDbRegistry::default();
        let session = registry
            .acquire(&project_root, &base_scope)
            .await
            .expect("acquire provider Tree-sitter workspace session");
        Self {
            _state_home: state_home,
            _temp: temp,
            _environment: environment,
            base_scope,
            registry,
            session,
        }
    }

    fn scope(
        &self,
        language_id: &str,
        provider_id: &str,
        digest_character: char,
    ) -> ProviderIncrementalScoped {
        ProviderIncrementalScoped {
            project_root: self.base_scope.project_root.clone(),
            workspace_identity: self.base_scope.workspace_identity.clone(),
            provider_workspace_identity_digest: digest(digest_character),
            language_id: language_id.to_string(),
            provider_id: provider_id.to_string(),
            provider_workspace_root: self.base_scope.provider_workspace_root.clone(),
        }
    }

    async fn write_inventory(
        &self,
        scope: &ProviderIncrementalScoped,
        state: ProviderOwnerInventoryState,
        owner_paths: &[&str],
    ) -> ProviderOwnerInventoryWriteReceipt {
        let entries = owner_paths
            .iter()
            .enumerate()
            .map(|(index, owner_path)| ProviderOwnerInventoryEntry {
                owner_path: (*owner_path).to_string(),
                owner_content_digest: Some(digest(
                    char::from_digit((index + 4) as u32, 16).unwrap(),
                )),
                state: ProviderOwnerInventoryEntryState::Indexed,
            })
            .collect();
        self.session
            .upsert_provider_owner_inventory(&ProviderOwnerInventoryWrite {
                scope: scope.clone(),
                state,
                entries,
            })
            .await
            .expect("write provider inventory")
    }

    fn query(
        &self,
        scope: ProviderIncrementalScoped,
        digest_character: char,
        capture_names: &[&str],
    ) -> ProviderTreeSitterQueryIdentity {
        ProviderTreeSitterQueryIdentity {
            scope,
            query_digest: digest(digest_character),
            capture_names: capture_names
                .iter()
                .map(|capture| (*capture).to_string())
                .collect(),
        }
    }

    fn result(
        &self,
        query: &ProviderTreeSitterQueryIdentity,
        inventory_generation: &str,
        owner_content_digest: &str,
        projections: Vec<ProviderTreeSitterCaptureProjection>,
    ) -> ProviderTreeSitterOwnerResult {
        ProviderTreeSitterOwnerResult {
            owner_path: "src/lib.rs".to_string(),
            owner_content_digest: owner_content_digest.to_string(),
            query_digest: query.query_digest.clone(),
            inventory_generation: inventory_generation.to_string(),
            state: ProviderTreeSitterOwnerResultState::Processed,
            complete_owner_refresh_count: 1,
            projections,
        }
    }

    fn capture(
        &self,
        capture_name: &str,
        signature: &str,
        start: u64,
        end: u64,
    ) -> ProviderTreeSitterCaptureProjection {
        ProviderTreeSitterCaptureProjection {
            structural_selector: format!(
                "rust://src/lib.rs#item/function/example-{capture_name}-{start}"
            ),
            signature: signature.to_string(),
            item_kind: "function".to_string(),
            item_name: "example".to_string(),
            capture_name: capture_name.to_string(),
            item_source_byte_start: 0,
            item_source_byte_end: 64,
            source_byte_start: start,
            source_byte_end: end,
        }
    }

    async fn inventory_entry_count(&self, scope: &ProviderIncrementalScoped) -> i64 {
        let connection = self.session.read_connection();
        scalar(
            &connection,
            "SELECT COUNT(*) FROM provider_owner_inventory_entry_v1
             WHERE project_root = ?1 AND workspace_identity = ?2
               AND provider_workspace_identity_digest = ?3 AND provider_id = ?4",
            scope_params(scope),
        )
        .await
    }

    async fn total_inventory_entry_count(&self) -> i64 {
        let connection = self.session.read_connection();
        scalar(
            &connection,
            "SELECT COUNT(*) FROM provider_owner_inventory_entry_v1",
            (),
        )
        .await
    }

    async fn inventory_state(&self, scope: &ProviderIncrementalScoped) -> String {
        let connection = self.session.read_connection();
        scalar_text(
            &connection,
            "SELECT inventory_state FROM provider_owner_inventory_v1
             WHERE project_root = ?1 AND workspace_identity = ?2
               AND provider_workspace_identity_digest = ?3 AND provider_id = ?4",
            scope_params(scope),
        )
        .await
    }

    async fn inventory_generation(&self, scope: &ProviderIncrementalScoped) -> String {
        let connection = self.session.read_connection();
        scalar_text(
            &connection,
            "SELECT inventory_generation FROM provider_owner_inventory_v1
             WHERE project_root = ?1 AND workspace_identity = ?2
               AND provider_workspace_identity_digest = ?3 AND provider_id = ?4",
            scope_params(scope),
        )
        .await
    }

    async fn inventory_paths(&self, scope: &ProviderIncrementalScoped) -> Vec<String> {
        let connection = self.session.read_connection();
        let mut rows = connection
            .query(
                "SELECT owner_path FROM provider_owner_inventory_entry_v1
                 WHERE project_root = ?1 AND workspace_identity = ?2
                   AND provider_workspace_identity_digest = ?3 AND provider_id = ?4
                 ORDER BY owner_path",
                scope_params(scope),
            )
            .await
            .expect("query inventory paths");
        let mut paths = Vec::new();
        while let Some(row) = rows.next().await.expect("read inventory path row") {
            paths.push(row.get::<String>(0).expect("decode inventory path"));
        }
        paths
    }

    async fn owner_capture_count(
        &self,
        query: &ProviderTreeSitterQueryIdentity,
        content_digest: &str,
    ) -> i64 {
        let connection = self.session.read_connection();
        scalar(
            &connection,
            "SELECT capture_count FROM provider_treesitter_query_owner_v1
             WHERE project_root = ?1 AND workspace_identity = ?2
               AND provider_workspace_identity_digest = ?3 AND provider_id = ?4
               AND query_digest = ?5 AND owner_path = ?6 AND owner_content_digest = ?7",
            query_owner_params(query, content_digest),
        )
        .await
    }

    async fn query_owner_row_count(&self, scope: &ProviderIncrementalScoped) -> i64 {
        let connection = self.session.read_connection();
        scalar(
            &connection,
            "SELECT COUNT(*) FROM provider_treesitter_query_owner_v1
             WHERE project_root = ?1 AND workspace_identity = ?2
               AND provider_workspace_identity_digest = ?3 AND provider_id = ?4",
            scope_params(scope),
        )
        .await
    }

    async fn capture_signatures(
        &self,
        query: &ProviderTreeSitterQueryIdentity,
        content_digest: &str,
    ) -> Vec<String> {
        let connection = self.session.read_connection();
        let mut rows = connection
            .query(
                "SELECT signature FROM provider_treesitter_capture_projection_v1
                 WHERE project_root = ?1 AND workspace_identity = ?2
                   AND provider_workspace_identity_digest = ?3 AND provider_id = ?4
                   AND query_digest = ?5 AND owner_path = ?6 AND owner_content_digest = ?7
                 ORDER BY source_byte_start",
                query_owner_params(query, content_digest),
            )
            .await
            .expect("query capture signatures");
        let mut signatures = Vec::new();
        while let Some(row) = rows.next().await.expect("read capture signature row") {
            signatures.push(row.get::<String>(0).expect("decode capture signature"));
        }
        signatures
    }
}

async fn scalar<P: turso::IntoParams>(connection: &turso::Connection, sql: &str, params: P) -> i64 {
    let mut rows = connection.query(sql, params).await.expect("query scalar");
    rows.next()
        .await
        .expect("read scalar row")
        .expect("scalar row exists")
        .get::<i64>(0)
        .expect("decode scalar")
}

async fn scalar_text<P: turso::IntoParams>(
    connection: &turso::Connection,
    sql: &str,
    params: P,
) -> String {
    let mut rows = connection.query(sql, params).await.expect("query text");
    rows.next()
        .await
        .expect("read text row")
        .expect("text row exists")
        .get::<String>(0)
        .expect("decode text")
}

fn query_owner_params<'a>(
    query: &'a ProviderTreeSitterQueryIdentity,
    content_digest: &'a str,
) -> (
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'static str,
    &'a str,
) {
    (
        query.scope.project_root.as_str(),
        query.scope.workspace_identity.as_str(),
        query.scope.provider_workspace_identity_digest.as_str(),
        query.scope.provider_id.as_str(),
        query.query_digest.as_str(),
        "src/lib.rs",
        content_digest,
    )
}

fn digest(character: char) -> String {
    std::iter::repeat_n(character, 64).collect()
}
