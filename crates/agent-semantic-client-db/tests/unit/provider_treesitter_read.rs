use agent_semantic_client_db::ProviderIncrementalScoped;
use agent_semantic_client_db::ProviderOwnerInventoryEntry;
use agent_semantic_client_db::ProviderOwnerInventoryEntryState;
use agent_semantic_client_db::ProviderOwnerInventoryState;
use agent_semantic_client_db::ProviderOwnerInventoryWrite;
use agent_semantic_client_db::ProviderSearchWorkspaceSession;
use agent_semantic_client_db::ProviderTreeSitterCaptureProjection;
use agent_semantic_client_db::ProviderTreeSitterOwnerResult;
use agent_semantic_client_db::ProviderTreeSitterOwnerResultState;
use agent_semantic_client_db::ProviderTreeSitterQueryIdentity;
use agent_semantic_client_db::ProviderTreeSitterQueryReadState;
use agent_semantic_client_db::WorkspaceDbRegistry;

use crate::test_support::StateHomeGuard;
use crate::test_support::TestDir;
use crate::test_support::environment_lock;
use crate::test_support::workspace;

fn digest(seed: u8) -> String {
    format!("{seed:064x}")
}

struct TestContext {
    _state_home: StateHomeGuard,
    _temp: TestDir,
    _environment: std::sync::MutexGuard<'static, ()>,
    registry: WorkspaceDbRegistry,
    session: ProviderSearchWorkspaceSession,
    scope: ProviderIncrementalScoped,
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
            _state_home: state_home,
            _temp: temp,
            _environment: environment,
            registry,
            session,
            scope,
        }
    }
}

fn scope(
    base: &ProviderIncrementalScoped,
    provider_id: &str,
    language_id: &str,
) -> ProviderIncrementalScoped {
    ProviderIncrementalScoped {
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
    state: ProviderOwnerInventoryEntryState,
) -> ProviderOwnerInventoryEntry {
    ProviderOwnerInventoryEntry {
        owner_path: owner_path.into(),
        owner_content_digest: Some(digest(content_seed)),
        state,
    }
}

fn query(scope: &ProviderIncrementalScoped, query_seed: u8) -> ProviderTreeSitterQueryIdentity {
    ProviderTreeSitterQueryIdentity {
        scope: scope.clone(),
        query_digest: digest(query_seed),
        capture_names: vec!["reference.name".into()],
    }
}

async fn publish_inventory(
    session: &ProviderSearchWorkspaceSession,
    scope: &ProviderIncrementalScoped,
    state: ProviderOwnerInventoryState,
    entries: Vec<ProviderOwnerInventoryEntry>,
) -> String {
    session
        .upsert_provider_owner_inventory(&ProviderOwnerInventoryWrite {
            scope: scope.clone(),
            state,
            entries,
        })
        .await
        .expect("publish inventory")
        .inventory_generation
}

async fn publish_cached_owner(
    session: &ProviderSearchWorkspaceSession,
    query: &ProviderTreeSitterQueryIdentity,
    inventory_generation: &str,
    owner_path: &str,
    content_seed: u8,
    with_capture: bool,
) {
    let projections = if with_capture {
        vec![ProviderTreeSitterCaptureProjection {
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
            &ProviderTreeSitterOwnerResult {
                owner_path: owner_path.into(),
                owner_content_digest: digest(content_seed),
                query_digest: query.query_digest.clone(),
                inventory_generation: inventory_generation.into(),
                state: ProviderTreeSitterOwnerResultState::Processed,
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
        ProviderTreeSitterQueryReadState::MissingInventory
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
        ProviderOwnerInventoryState::Exact,
        vec![
            entry("a.rs", 1, ProviderOwnerInventoryEntryState::Indexed),
            entry("b.rs", 2, ProviderOwnerInventoryEntryState::Indexed),
            entry("c.rs", 3, ProviderOwnerInventoryEntryState::Indexed),
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

    assert_eq!(read.state, ProviderTreeSitterQueryReadState::Partial);
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
        ProviderOwnerInventoryState::Exact,
        vec![
            entry("a.rs", 1, ProviderOwnerInventoryEntryState::Indexed),
            entry("b.rs", 2, ProviderOwnerInventoryEntryState::Indexed),
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
    assert_eq!(complete.state, ProviderTreeSitterQueryReadState::Complete);
    assert!(complete.absence_authoritative);
    assert!(complete.scheduled_entries.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn exact_two_owner_rounds_are_complete_only_after_both_query_results_exist() {
    for (label, inserted, expected) in [
        (
            "treesitter-order-lib-other",
            [("other.rs", 2_u8), ("lib.rs", 1_u8)],
            [("lib.rs", 1_u8), ("other.rs", 2_u8)],
        ),
        (
            "treesitter-order-a-z",
            [("z.rs", 4_u8), ("a.rs", 3_u8)],
            [("a.rs", 3_u8), ("z.rs", 4_u8)],
        ),
    ] {
        let context = TestContext::new(label, "rust-provider", "rust").await;
        let generation = publish_inventory(
            &context.session,
            &context.scope,
            ProviderOwnerInventoryState::Exact,
            inserted
                .into_iter()
                .map(|(owner, seed)| entry(owner, seed, ProviderOwnerInventoryEntryState::Indexed))
                .collect(),
        )
        .await;
        let query = query(&context.scope, 40);

        let first = context
            .session
            .read_provider_treesitter_query(&query, 1, None)
            .await
            .expect("schedule first owner");
        assert_eq!(first.state, ProviderTreeSitterQueryReadState::Partial);
        assert_eq!(first.scheduled_entries.len(), 1);
        assert_eq!(first.scheduled_entries[0].owner_path, expected[0].0);
        let first_receipt = first.receipt.expect("first receipt");
        assert_eq!(first_receipt.remaining_owner_count, 1);
        assert_eq!(
            first_receipt
                .continuation
                .expect("first continuation")
                .next_owner_cursor,
            expected[1].0
        );
        publish_cached_owner(
            &context.session,
            &query,
            &generation,
            expected[0].0,
            expected[0].1,
            false,
        )
        .await;

        let audit = context
            .session
            .read_provider_treesitter_query(&query, 0, None)
            .await
            .expect("audit pending owner");
        assert_eq!(audit.state, ProviderTreeSitterQueryReadState::Partial);
        assert!(audit.scheduled_entries.is_empty());
        let audit_receipt = audit.receipt.expect("audit receipt");
        assert_eq!(audit_receipt.remaining_owner_count, 1);
        assert_eq!(
            audit_receipt
                .continuation
                .expect("audit continuation")
                .next_owner_cursor,
            expected[1].0
        );

        let second = context
            .session
            .read_provider_treesitter_query(&query, 1, None)
            .await
            .expect("schedule second owner");
        assert_eq!(second.state, ProviderTreeSitterQueryReadState::Partial);
        assert_eq!(second.scheduled_entries.len(), 1);
        assert_eq!(second.scheduled_entries[0].owner_path, expected[1].0);
        assert_eq!(
            second
                .receipt
                .expect("second receipt")
                .remaining_owner_count,
            0
        );
        publish_cached_owner(
            &context.session,
            &query,
            &generation,
            expected[1].0,
            expected[1].1,
            false,
        )
        .await;

        let complete = context
            .session
            .read_provider_treesitter_query(&query, 0, None)
            .await
            .expect("complete audit");
        assert_eq!(complete.state, ProviderTreeSitterQueryReadState::Complete);
        assert!(complete.scheduled_entries.is_empty());
        let complete_receipt = complete.receipt.expect("complete receipt");
        assert_eq!(complete_receipt.remaining_owner_count, 0);
        assert!(complete_receipt.continuation.is_none());
    }
}

#[tokio::test(flavor = "current_thread")]
async fn continuation_rejects_query_identity_drift() {
    let context = TestContext::new("treesitter-query-drift", "rust-provider", "rust").await;
    let rust = &context.scope;
    publish_inventory(
        &context.session,
        rust,
        ProviderOwnerInventoryState::Exact,
        vec![entry("a.rs", 1, ProviderOwnerInventoryEntryState::Indexed)],
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
        ProviderOwnerInventoryState::Exact,
        vec![entry("a.rs", 1, ProviderOwnerInventoryEntryState::Indexed)],
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
        ProviderOwnerInventoryState::Exact,
        vec![entry("a.rs", 2, ProviderOwnerInventoryEntryState::Dirty)],
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
        ProviderOwnerInventoryState::Exact,
        vec![entry("a.rs", 1, ProviderOwnerInventoryEntryState::Indexed)],
    )
    .await;
    let query = query(rust, 40);
    publish_cached_owner(&context.session, &query, &generation, "a.rs", 1, true).await;
    publish_inventory(
        &context.session,
        rust,
        ProviderOwnerInventoryState::Exact,
        vec![entry("a.rs", 2, ProviderOwnerInventoryEntryState::Dirty)],
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
        ProviderOwnerInventoryState::Exact,
        vec![entry("a.rs", 1, ProviderOwnerInventoryEntryState::Indexed)],
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
        ProviderOwnerInventoryState::Known,
        vec![entry("a.rs", 1, ProviderOwnerInventoryEntryState::Indexed)],
    )
    .await;
    publish_inventory(
        &context.session,
        &julia,
        ProviderOwnerInventoryState::Exact,
        vec![entry(
            "unrelated.jl",
            9,
            ProviderOwnerInventoryEntryState::Indexed,
        )],
    )
    .await;
    let query = query(rust, 40);
    publish_cached_owner(&context.session, &query, &rust_generation, "a.rs", 1, false).await;

    let read = context
        .session
        .read_provider_treesitter_query(&query, 2, None)
        .await
        .expect("known inventory");
    assert_eq!(read.state, ProviderTreeSitterQueryReadState::Partial);
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
