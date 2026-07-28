use std::{fs, path::PathBuf};

use super::{
    ProviderIncrementalScopeV1, ProviderOwnerInventoryEntryStateV1, ProviderOwnerInventoryEntryV1,
    ProviderOwnerInventoryStateV1, ProviderOwnerInventoryWriteReceiptV1,
    ProviderOwnerInventoryWriteV1, ProviderTreeSitterCaptureProjectionV1,
    ProviderTreeSitterOwnerResultStateV1, ProviderTreeSitterOwnerResultV1,
    ProviderTreeSitterQueryIdentityV1, connect_turso_client_db, scope_params,
    upsert_provider_owner_inventory_v1, write_provider_treesitter_owner_result_v1,
};

#[tokio::test(flavor = "current_thread")]
async fn exact_rust_inventory_is_isolated_from_julia_and_gerbil_scopes() {
    let fixture = ProviderTreeSitterWriteFixture::new("scope-isolation");
    let rust = fixture.scope("rust", "rust-harness", '1');
    let julia = fixture.scope("julia", "julia-harness", '2');
    let gerbil = fixture.scope("gerbil-scheme", "gerbil-harness", '3');

    fixture
        .write_inventory(&julia, ProviderOwnerInventoryStateV1::Known, &["src/a.jl"])
        .await;
    fixture
        .write_inventory(&gerbil, ProviderOwnerInventoryStateV1::Known, &["src/a.ss"])
        .await;
    let receipt = fixture
        .write_inventory(&rust, ProviderOwnerInventoryStateV1::Exact, &["src/lib.rs"])
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
    let fixture = ProviderTreeSitterWriteFixture::new("stale-owner");
    let rust = fixture.scope("rust", "rust-harness", '1');
    let julia = fixture.scope("julia", "julia-harness", '2');
    let gerbil = fixture.scope("gerbil-scheme", "gerbil-harness", '3');

    fixture
        .write_inventory(&julia, ProviderOwnerInventoryStateV1::Known, &["src/a.jl"])
        .await;
    fixture
        .write_inventory(&gerbil, ProviderOwnerInventoryStateV1::Known, &["src/a.ss"])
        .await;
    fixture
        .write_inventory(
            &rust,
            ProviderOwnerInventoryStateV1::Exact,
            &["src/lib.rs", "src/stale.rs"],
        )
        .await;
    let julia_generation = fixture.inventory_generation(&julia).await;
    let gerbil_generation = fixture.inventory_generation(&gerbil).await;

    let receipt = fixture
        .write_inventory(&rust, ProviderOwnerInventoryStateV1::Exact, &["src/lib.rs"])
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
    let fixture = ProviderTreeSitterWriteFixture::new("capture-replace");
    let scope = fixture.scope("rust", "rust-harness", '1');
    let inventory = fixture
        .write_inventory(
            &scope,
            ProviderOwnerInventoryStateV1::Exact,
            &["src/lib.rs"],
        )
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
    write_provider_treesitter_owner_result_v1(fixture.db_path.as_path(), &query, &first)
        .await
        .expect("write initial owner captures");

    let replacement = fixture.result(
        &query,
        inventory.inventory_generation.as_str(),
        content_digest.as_str(),
        vec![fixture.capture("declaration.name", "pub fn replacement()", 0, 11)],
    );
    let receipt =
        write_provider_treesitter_owner_result_v1(fixture.db_path.as_path(), &query, &replacement)
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
    let fixture = ProviderTreeSitterWriteFixture::new("cache-keys");
    let scope = fixture.scope("rust", "rust-harness", '1');
    let inventory = fixture
        .write_inventory(
            &scope,
            ProviderOwnerInventoryStateV1::Exact,
            &["src/lib.rs"],
        )
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
        write_provider_treesitter_owner_result_v1(fixture.db_path.as_path(), query, &result)
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
        let fixture = ProviderTreeSitterWriteFixture::new(label);
        let scope = fixture.scope("rust", "rust-harness", '1');
        let query = fixture.query(scope, 'a', &["declaration.name"]);
        let mut result = fixture.result(
            &query,
            digest('b').as_str(),
            digest('c').as_str(),
            vec![fixture.capture("declaration.name", "pub fn example()", 0, 8)],
        );
        match invalid {
            InvalidWrite::Completeness => {
                result.state = ProviderTreeSitterOwnerResultStateV1::Cached;
            }
            InvalidWrite::Signature => result.projections[0].signature.clear(),
            InvalidWrite::Span => result.projections[0].source_byte_end = 65,
            InvalidWrite::CacheKey => result.query_digest = digest('d'),
        }

        let error =
            write_provider_treesitter_owner_result_v1(fixture.db_path.as_path(), &query, &result)
                .await
                .expect_err("invalid writer request must fail");
        assert!(!error.is_empty());
        assert!(
            !fixture.db_path.exists(),
            "{label} validation opened or wrote the database"
        );
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
    root: PathBuf,
    db_path: PathBuf,
}

impl ProviderTreeSitterWriteFixture {
    fn new(label: &str) -> Self {
        let root = temp_root(label);
        let db_path = root.join("facts.turso");
        Self { root, db_path }
    }

    fn scope(
        &self,
        language_id: &str,
        provider_id: &str,
        digest_character: char,
    ) -> ProviderIncrementalScopeV1 {
        ProviderIncrementalScopeV1 {
            project_root: "/project".to_string(),
            workspace_identity: "workspace-1".to_string(),
            provider_workspace_identity_digest: digest(digest_character),
            language_id: language_id.to_string(),
            provider_id: provider_id.to_string(),
            provider_workspace_root: "/project".to_string(),
        }
    }

    async fn write_inventory(
        &self,
        scope: &ProviderIncrementalScopeV1,
        state: ProviderOwnerInventoryStateV1,
        owner_paths: &[&str],
    ) -> ProviderOwnerInventoryWriteReceiptV1 {
        let entries = owner_paths
            .iter()
            .enumerate()
            .map(|(index, owner_path)| ProviderOwnerInventoryEntryV1 {
                owner_path: (*owner_path).to_string(),
                owner_content_digest: Some(digest(
                    char::from_digit((index + 4) as u32, 16).unwrap(),
                )),
                state: ProviderOwnerInventoryEntryStateV1::Indexed,
            })
            .collect();
        upsert_provider_owner_inventory_v1(
            self.db_path.as_path(),
            &ProviderOwnerInventoryWriteV1 {
                scope: scope.clone(),
                state,
                entries,
            },
        )
        .await
        .expect("write provider inventory")
    }

    fn query(
        &self,
        scope: ProviderIncrementalScopeV1,
        digest_character: char,
        capture_names: &[&str],
    ) -> ProviderTreeSitterQueryIdentityV1 {
        ProviderTreeSitterQueryIdentityV1 {
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
        query: &ProviderTreeSitterQueryIdentityV1,
        inventory_generation: &str,
        owner_content_digest: &str,
        projections: Vec<ProviderTreeSitterCaptureProjectionV1>,
    ) -> ProviderTreeSitterOwnerResultV1 {
        ProviderTreeSitterOwnerResultV1 {
            owner_path: "src/lib.rs".to_string(),
            owner_content_digest: owner_content_digest.to_string(),
            query_digest: query.query_digest.clone(),
            inventory_generation: inventory_generation.to_string(),
            state: ProviderTreeSitterOwnerResultStateV1::Processed,
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
    ) -> ProviderTreeSitterCaptureProjectionV1 {
        ProviderTreeSitterCaptureProjectionV1 {
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

    async fn inventory_entry_count(&self, scope: &ProviderIncrementalScopeV1) -> i64 {
        let connection = connect_turso_client_db(self.db_path.as_path())
            .await
            .expect("open fixture database");
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
        let connection = connect_turso_client_db(self.db_path.as_path())
            .await
            .expect("open fixture database");
        scalar(
            &connection,
            "SELECT COUNT(*) FROM provider_owner_inventory_entry_v1",
            (),
        )
        .await
    }

    async fn inventory_state(&self, scope: &ProviderIncrementalScopeV1) -> String {
        let connection = connect_turso_client_db(self.db_path.as_path())
            .await
            .expect("open fixture database");
        scalar_text(
            &connection,
            "SELECT inventory_state FROM provider_owner_inventory_v1
             WHERE project_root = ?1 AND workspace_identity = ?2
               AND provider_workspace_identity_digest = ?3 AND provider_id = ?4",
            scope_params(scope),
        )
        .await
    }

    async fn inventory_generation(&self, scope: &ProviderIncrementalScopeV1) -> String {
        let connection = connect_turso_client_db(self.db_path.as_path())
            .await
            .expect("open fixture database");
        scalar_text(
            &connection,
            "SELECT inventory_generation FROM provider_owner_inventory_v1
             WHERE project_root = ?1 AND workspace_identity = ?2
               AND provider_workspace_identity_digest = ?3 AND provider_id = ?4",
            scope_params(scope),
        )
        .await
    }

    async fn inventory_paths(&self, scope: &ProviderIncrementalScopeV1) -> Vec<String> {
        let connection = connect_turso_client_db(self.db_path.as_path())
            .await
            .expect("open fixture database");
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
        query: &ProviderTreeSitterQueryIdentityV1,
        content_digest: &str,
    ) -> i64 {
        let connection = connect_turso_client_db(self.db_path.as_path())
            .await
            .expect("open fixture database");
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

    async fn query_owner_row_count(&self, scope: &ProviderIncrementalScopeV1) -> i64 {
        let connection = connect_turso_client_db(self.db_path.as_path())
            .await
            .expect("open fixture database");
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
        query: &ProviderTreeSitterQueryIdentityV1,
        content_digest: &str,
    ) -> Vec<String> {
        let connection = connect_turso_client_db(self.db_path.as_path())
            .await
            .expect("open fixture database");
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

impl Drop for ProviderTreeSitterWriteFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
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
    query: &'a ProviderTreeSitterQueryIdentityV1,
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

fn temp_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "asp-provider-treesitter-write-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos()
    ));
    fs::create_dir_all(&root).expect("create provider Tree-sitter fixture root");
    root
}
