use agent_semantic_client_db::{
    ProviderIncrementalScopeV1, ProviderOwnerInventoryEntryStateV1, ProviderOwnerInventoryEntryV1,
    ProviderOwnerInventoryStateV1, ProviderOwnerInventoryWriteV1,
    ProviderTreeSitterCaptureProjectionV1, ProviderTreeSitterOwnerResultStateV1,
    ProviderTreeSitterOwnerResultV1, ProviderTreeSitterQueryIdentityV1,
    ProviderTreeSitterQueryReadStateV1, ProviderSearchWorkspaceSessionV1, WorkspaceDbRegistry,
};

use super::workspace_db_registry::{StateHomeGuard, TestDir, environment_lock, workspace};

fn digest(seed: u8) -> String {
    format!("{seed:064x}")
}

struct TestContext {
    _environment: std::sync::MutexGuard<'static, ()>,
    _temp: TestDir,
    _state_home: StateHomeGuard,
    registry: WorkspaceDbRegistry,
    session: ProviderSearchWorkspaceSessionV1,
    scope: ProviderIncrementalScopeV1,
}

impl TestContext {
    async fn new(label: &str, provider_id: &str, language_id: &str) -> Self {
        let environment = environment_lock();
        let temp = TestDir::new(label);
        let state_home = StateHomeGuard::install(&temp.path().join("state"));
        let (project_root, _resolved, mut scope) = workspace(temp.path(), label);
        scope.provider_workspace_identity_digest =
            digest(provider_id.bytes().fold(1_u8, u8::wrapping_add));
        scope.language_id = language_id.into();
        scope.provider_id = provider_id.into();
        scope.provider_workspace_root = project_root.to_string_lossy().into_owned();
        let registry = WorkspaceDbRegistry::default();
        let session = registry
            .acquire(&project_root, &scope)
            .await
            .expect("acquire Tree-sitter workspace session");
        Self {
            _environment: environment,
            _temp: temp,
            _state_home: state_home,
            registry,
            session,
            scope,
        }
    }
}

fn scope(
    base: &ProviderIncrementalScopeV1,
    provider_id: &str,
    language_id: &str,
) -> ProviderIncrementalScopeV1 {
    ProviderIncrementalScopeV1 {
        project_root: base.project_root.clone(),
        workspace_identity: base.workspace_identity.clone(),
        provider_workspace_identity_digest: digest(
            provider_id.bytes().fold(1_u8, u8::wrapping_add),
        ),
        language_id: language_id.into(),
        provider_id: provider_id.into(),
        provider_workspace_root: base.provider_workspace_root.clone(),
    }
}

fn entry(
    owner_path: &str,
    content_seed: u8,
    state: ProviderOwnerInventoryEntryStateV1,
) -> ProviderOwnerInventoryEntryV1 {
    ProviderOwnerInventoryEntryV1 {
        owner_path: owner_path.into(),
        owner_content_digest: Some(digest(content_seed)),
        state,
    }
}

fn query(scope: &ProviderIncrementalScopeV1, query_seed: u8) -> ProviderTreeSitterQueryIdentityV1 {
    ProviderTreeSitterQueryIdentityV1 {
        scope: scope.clone(),
        query_digest: digest(query_seed),
        capture_names: vec!["reference.name".into()],
    }
}

async fn publish_inventory(
    session: &ProviderSearchWorkspaceSessionV1,
    scope: &ProviderIncrementalScopeV1,
    state: ProviderOwnerInventoryStateV1,
    entries: Vec<ProviderOwnerInventoryEntryV1>,
) -> String {
    session
        .upsert_provider_owner_inventory(&ProviderOwnerInventoryWriteV1 {
            scope: scope.clone(),
            state,
            entries,
        })
        .await
        .expect("publish inventory")
        .inventory_generation
}

async fn publish_cached_owner(
    session: &ProviderSearchWorkspaceSessionV1,
    query: &ProviderTreeSitterQueryIdentityV1,
    inventory_generation: &str,
    owner_path: &str,
    content_seed: u8,
    with_capture: bool,
) {
    let projections = if with_capture {
        vec![ProviderTreeSitterCaptureProjectionV1 {
            structural_selector: format!("rust://{owner_path}#item/struct/Hit"),
            signature: "struct Hit".into(),
            item_kind: "struct".into(),
            item_name: "Hit".into(),
            capture_name: "reference.name".into(),
            item_source_byte_start: 0,
            item_source_byte_end: 20,
            source_byte_start: 7,
            source_byte_end: 10,
        }]
    } else {
        Vec::new()
    };
    session
        .write_provider_treesitter_owner_result(
            query,
            &ProviderTreeSitterOwnerResultV1 {
            owner_path: owner_path.into(),
            owner_content_digest: digest(content_seed),
            query_digest: query.query_digest.clone(),
            inventory_generation: inventory_generation.into(),
            state: ProviderTreeSitterOwnerResultStateV1::Processed,
            complete_owner_refresh_count: 1,
            projections,
            },
        )
        .await
        .expect("publish cached owner");
}

#[tokio::test(flavor = "current_thread")]
async fn missing_inventory_is_typed_without_reopening_turso_state() {
    let context = TestContext::new("treesitter-missing", "rust-provider", "rust").await;
    let query = query(&context.scope, 40);

    let read = context
        .session
        .read_provider_treesitter_query(&query, 2, None)
        .await
        .expect("typed missing inventory");

    assert_eq!(
        read.state,
        ProviderTreeSitterQueryReadStateV1::MissingInventory
    );
    assert!(!read.absence_authoritative);
    assert!(read.inventory.is_none());
    assert!(read.receipt.is_none());
    assert_eq!(context.registry.counters().database_open_count, 1);
}

#[tokio::test(flavor = "current_thread")]
async fn exact_inventory_returns_cache_and_deterministic_budget_queue() {
    let context = TestContext::new("treesitter-budget", "rust-provider", "rust").await;
    let rust = &context.scope;
    let generation = publish_inventory(
        &context.session,
        rust,
        ProviderOwnerInventoryStateV1::Exact,
        vec![
            entry("a.rs", 1, ProviderOwnerInventoryEntryStateV1::Indexed),
            entry("b.rs", 2, ProviderOwnerInventoryEntryStateV1::Indexed),
            entry("c.rs", 3, ProviderOwnerInventoryEntryStateV1::Indexed),
        ],
    )
    .await;
    let query = query(rust, 40);
    publish_cached_owner(&context.session, &query, &generation, "a.rs", 1, true).await;

    let read = context
        .session
        .read_provider_treesitter_query(&query, 1, None)
        .await
        .expect("read");

    assert_eq!(read.state, ProviderTreeSitterQueryReadStateV1::Partial);
    assert_eq!(read.cached_results[0].owner_path, "a.rs");
    assert_eq!(read.scheduled_entries[0].owner_path, "b.rs");
    let receipt = read.receipt.expect("receipt");
    assert_eq!(receipt.remaining_owner_count, 1);
    assert_eq!(
        receipt
            .continuation
            .expect("continuation")
            .next_owner_cursor,
        "c.rs"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn continuation_advances_and_exact_inventory_completes_after_writes() {
    let context = TestContext::new("treesitter-continuation", "rust-provider", "rust").await;
    let rust = &context.scope;
    let generation = publish_inventory(
        &context.session,
        rust,
        ProviderOwnerInventoryStateV1::Exact,
        vec![
            entry("a.rs", 1, ProviderOwnerInventoryEntryStateV1::Indexed),
            entry("b.rs", 2, ProviderOwnerInventoryEntryStateV1::Indexed),
        ],
    )
    .await;
    let query = query(rust, 40);
    let first = context
        .session
        .read_provider_treesitter_query(&query, 1, None)
        .await
        .expect("first");
    assert_eq!(first.scheduled_entries[0].owner_path, "a.rs");
    publish_cached_owner(&context.session, &query, &generation, "a.rs", 1, false).await;
    let token = first
        .receipt
        .expect("receipt")
        .continuation
        .expect("continuation");
    let second = context
        .session
        .read_provider_treesitter_query(&query, 1, Some(&token))
        .await
        .expect("resume");
    assert_eq!(second.scheduled_entries[0].owner_path, "b.rs");
    publish_cached_owner(&context.session, &query, &generation, "b.rs", 2, false).await;

    let complete = context
        .session
        .read_provider_treesitter_query(&query, 1, None)
        .await
        .expect("complete");
    assert_eq!(complete.state, ProviderTreeSitterQueryReadStateV1::Complete);
    assert!(complete.absence_authoritative);
    assert!(complete.scheduled_entries.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn continuation_rejects_query_identity_drift() {
    let context = TestContext::new("treesitter-query-drift", "rust-provider", "rust").await;
    let rust = &context.scope;
    publish_inventory(
        &context.session,
        rust,
        ProviderOwnerInventoryStateV1::Exact,
        vec![entry(
            "a.rs",
            1,
            ProviderOwnerInventoryEntryStateV1::Indexed,
        )],
    )
    .await;
    let original = query(rust, 40);
    let first = context
        .session
        .read_provider_treesitter_query(&original, 0, None)
        .await
        .expect("first");
    let token = first.receipt.unwrap().continuation.unwrap();

    let error = context
        .session
        .read_provider_treesitter_query(&query(rust, 41), 1, Some(&token))
        .await
        .expect_err("query drift must fail");
    assert!(error.contains("identity"));
}

#[tokio::test(flavor = "current_thread")]
async fn continuation_rejects_inventory_generation_drift() {
    let context = TestContext::new("treesitter-inventory-drift", "rust-provider", "rust").await;
    let rust = &context.scope;
    publish_inventory(
        &context.session,
        rust,
        ProviderOwnerInventoryStateV1::Exact,
        vec![entry(
            "a.rs",
            1,
            ProviderOwnerInventoryEntryStateV1::Indexed,
        )],
    )
    .await;
    let query = query(rust, 40);
    let first = context
        .session
        .read_provider_treesitter_query(&query, 0, None)
        .await
        .expect("first");
    let token = first.receipt.unwrap().continuation.unwrap();
    publish_inventory(
        &context.session,
        rust,
        ProviderOwnerInventoryStateV1::Exact,
        vec![entry("a.rs", 2, ProviderOwnerInventoryEntryStateV1::Dirty)],
    )
    .await;

    let error = context
        .session
        .read_provider_treesitter_query(&query, 1, Some(&token))
        .await
        .expect_err("inventory drift must fail");
    assert!(error.contains("identity"));
}

#[tokio::test(flavor = "current_thread")]
async fn changed_owner_content_digest_invalidates_old_query_cache() {
    let context = TestContext::new("treesitter-content-drift", "rust-provider", "rust").await;
    let rust = &context.scope;
    let generation = publish_inventory(
        &context.session,
        rust,
        ProviderOwnerInventoryStateV1::Exact,
        vec![entry(
            "a.rs",
            1,
            ProviderOwnerInventoryEntryStateV1::Indexed,
        )],
    )
    .await;
    let query = query(rust, 40);
    publish_cached_owner(&context.session, &query, &generation, "a.rs", 1, true).await;
    publish_inventory(
        &context.session,
        rust,
        ProviderOwnerInventoryStateV1::Exact,
        vec![entry("a.rs", 2, ProviderOwnerInventoryEntryStateV1::Dirty)],
    )
    .await;

    let read = context
        .session
        .read_provider_treesitter_query(&query, 1, None)
        .await
        .expect("read changed owner");
    assert!(read.cached_results.is_empty());
    assert_eq!(read.scheduled_entries[0].owner_path, "a.rs");
}

#[tokio::test(flavor = "current_thread")]
async fn new_query_digest_schedules_owner_even_when_old_query_is_cached() {
    let context = TestContext::new("treesitter-new-query", "rust-provider", "rust").await;
    let rust = &context.scope;
    let generation = publish_inventory(
        &context.session,
        rust,
        ProviderOwnerInventoryStateV1::Exact,
        vec![entry(
            "a.rs",
            1,
            ProviderOwnerInventoryEntryStateV1::Indexed,
        )],
    )
    .await;
    publish_cached_owner(
        &context.session,
        &query(rust, 40),
        &generation,
        "a.rs",
        1,
        true,
    )
    .await;

    let read = context
        .session
        .read_provider_treesitter_query(&query(rust, 41), 1, None)
        .await
        .expect("new query");
    assert!(read.cached_results.is_empty());
    assert_eq!(read.scheduled_entries[0].owner_path, "a.rs");
}

#[tokio::test(flavor = "current_thread")]
async fn known_inventory_never_completes_and_ignores_unrelated_provider_state() {
    let context = TestContext::new("treesitter-known", "rust-provider", "rust").await;
    let rust = &context.scope;
    let julia = scope(&context.scope, "julia-provider", "julia");
    let rust_generation = publish_inventory(
        &context.session,
        rust,
        ProviderOwnerInventoryStateV1::Known,
        vec![entry(
            "a.rs",
            1,
            ProviderOwnerInventoryEntryStateV1::Indexed,
        )],
    )
    .await;
    publish_inventory(
        &context.session,
        &julia,
        ProviderOwnerInventoryStateV1::Exact,
        vec![entry(
            "unrelated.jl",
            9,
            ProviderOwnerInventoryEntryStateV1::Indexed,
        )],
    )
    .await;
    let query = query(rust, 40);
    publish_cached_owner(
        &context.session,
        &query,
        &rust_generation,
        "a.rs",
        1,
        false,
    )
    .await;

    let read = context
        .session
        .read_provider_treesitter_query(&query, 2, None)
        .await
        .expect("known inventory");
    assert_eq!(read.state, ProviderTreeSitterQueryReadStateV1::Partial);
    assert!(!read.absence_authoritative);
    assert_eq!(read.cached_results.len(), 1);
    assert!(
        read.inventory
            .unwrap()
            .entries
            .iter()
            .all(|entry| entry.owner_path != "unrelated.jl")
    );
}
