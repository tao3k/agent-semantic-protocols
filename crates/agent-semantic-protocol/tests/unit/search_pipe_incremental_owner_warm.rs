use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_db::{
    ProviderIncrementalOwnerWrite, ProviderIncrementalScoped, ProviderOwnerFingerprint,
    ProviderSelectorProjection, WorkspaceDbRegistry,
};

use super::{
    CommitOwnerRequest, OwnerItemsLookup, commit_complete_owner_response, lookup_owner_state,
    provider_owner_metadata,
};
use crate::command::provider_owner_native::{
    ExpectedOwnerResponse, ProviderNativeOwnerSearchResponse, validate_provider_owner_response,
};

#[test]
fn complete_owner_seed_serves_alpha_then_beta_without_provider_or_source_io() {
    let fixture = WarmOwnerFixture::new();
    let owner_key = "src/lib.rs";
    let metadata = provider_owner_metadata(&fixture.owner_path).expect("owner metadata");
    let scope = fixture.scope();
    let projections = vec![projection("alpha", 0, 17), projection("Beta", 18, 34)];
    fixture
        .runtime
        .block_on(fixture.session.write_provider_incremental_owner(
            &ProviderIncrementalOwnerWrite {
                scope: scope.clone(),
                owner_path: owner_key.to_string(),
                fingerprint: ProviderOwnerFingerprint {
                    metadata: metadata.clone(),
                    content_digest: "a".repeat(64),
                },
                projection_completeness: "complete-owner".to_string(),
                projections,
            },
        ))
        .expect("seed complete owner transaction");

    let alpha = lookup_owner_state(
        &fixture.runtime,
        &fixture.session,
        &scope,
        owner_key,
        &metadata,
        "alpha",
    )
    .expect("alpha warm lookup");
    let beta = lookup_owner_state(
        &fixture.runtime,
        &fixture.session,
        &scope,
        owner_key,
        &metadata,
        "Beta",
    )
    .expect("beta warm lookup");
    let OwnerItemsLookup::Warm(alpha) = alpha else {
        panic!("alpha must be a DB hit");
    };
    let OwnerItemsLookup::Warm(beta) = beta else {
        panic!("beta must be a DB hit");
    };

    assert_eq!(item_names(&alpha.projections), vec!["alpha"]);
    assert_eq!(item_names(&beta.projections), vec!["Beta"]);
    assert_zero_side_effects(&alpha.receipt);
    assert_zero_side_effects(&beta.receipt);
}

#[test]
fn validated_new_then_changed_commit_becomes_zero_provider_warm_hit() {
    let fixture = WarmOwnerFixture::new();
    let owner_key = "src/lib.rs";
    let scope = fixture.scope();
    let metadata_new = provider_owner_metadata(&fixture.owner_path).expect("new metadata");
    let new_probe = expect_refresh(
        lookup_owner_state(
            &fixture.runtime,
            &fixture.session,
            &scope,
            owner_key,
            &metadata_new,
            "alpha",
        )
        .expect("new lookup"),
    );
    assert_eq!(
        new_probe.decision,
        agent_semantic_client_db::ProviderOwnerDecision::New
    );
    let new_hit = commit_complete_owner_response(CommitOwnerRequest {
        runtime: &fixture.runtime,
        session: &fixture.session,
        scope: &scope,
        owner_path: owner_key,
        fingerprint: ProviderOwnerFingerprint {
            metadata: metadata_new,
            content_digest: digest(&fixture.owner_path),
        },
        query: "alpha",
        decision: new_probe.decision,
        projections: vec![projection("alpha", 0, 17), projection("Beta", 18, 34)],
    })
    .expect("commit new owner");
    assert_refresh_side_effects(&new_hit.receipt);

    fs::write(
        &fixture.owner_path,
        b"pub fn alpha() {}\npub struct GammaWithLongerName;\n",
    )
    .expect("change owner fixture");
    let metadata_changed = provider_owner_metadata(&fixture.owner_path).expect("changed metadata");
    let changed_probe = expect_refresh(
        lookup_owner_state(
            &fixture.runtime,
            &fixture.session,
            &scope,
            owner_key,
            &metadata_changed,
            "Gamma",
        )
        .expect("changed lookup"),
    );
    assert_eq!(
        changed_probe.decision,
        agent_semantic_client_db::ProviderOwnerDecision::Changed
    );
    let changed_hit = commit_complete_owner_response(CommitOwnerRequest {
        runtime: &fixture.runtime,
        session: &fixture.session,
        scope: &scope,
        owner_path: owner_key,
        fingerprint: ProviderOwnerFingerprint {
            metadata: metadata_changed.clone(),
            content_digest: digest(&fixture.owner_path),
        },
        query: "Gamma",
        decision: changed_probe.decision,
        projections: vec![
            projection("alpha", 0, 17),
            projection("GammaWithLongerName", 18, 49),
        ],
    })
    .expect("commit changed owner");
    assert_refresh_side_effects(&changed_hit.receipt);

    let warm = lookup_owner_state(
        &fixture.runtime,
        &fixture.session,
        &scope,
        owner_key,
        &metadata_changed,
        "Gamma",
    )
    .expect("post-change warm lookup");
    let OwnerItemsLookup::Warm(warm) = warm else {
        panic!("changed owner must become warm");
    };
    assert_eq!(item_names(&warm.projections), vec!["GammaWithLongerName"]);
    assert_zero_side_effects(&warm.receipt);
}

#[test]
fn provider_response_rejects_partial_completeness_and_invalid_span_before_write() {
    let content_digest = "c".repeat(64);
    let expected = ExpectedOwnerResponse {
        language_id: "rust",
        provider_id: "rs-harness",
        owner_path: "src/lib.rs",
        projection_mode: "complete-owner",
        content_digest: content_digest.as_str(),
        source_size: 20,
    };
    let partial: ProviderNativeOwnerSearchResponse =
        serde_json::from_value(provider_response("query-filtered", 0, 17))
            .expect("partial response shape");
    let partial_error =
        validate_provider_owner_response(partial, expected).expect_err("reject partial response");
    assert!(partial_error.contains("completeness drift"));

    let invalid_span: ProviderNativeOwnerSearchResponse =
        serde_json::from_value(provider_response("complete-owner", 0, 21))
            .expect("invalid span response shape");
    let invalid_span_error = validate_provider_owner_response(
        invalid_span,
        ExpectedOwnerResponse {
            language_id: "rust",
            provider_id: "rs-harness",
            owner_path: "src/lib.rs",
            projection_mode: "complete-owner",
            content_digest: content_digest.as_str(),
            source_size: 20,
        },
    )
    .expect_err("reject invalid span");
    assert!(invalid_span_error.contains("projection is invalid"));
}

fn provider_response(
    projection_completeness: &str,
    source_byte_start: u64,
    source_byte_end: u64,
) -> serde_json::Value {
    serde_json::json!({
        "schemaId": "agent.semantic-protocols.provider-native-owner-search-response",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "requestedOwnerPath": "src/lib.rs",
        "requestedProjectionMode": "complete-owner",
        "sourceContentDigest": "c".repeat(64),
        "parsedOwnerCount": 1,
        "projectionCompleteness": projection_completeness,
        "projections": [{
            "structuralSelector": "rust://src/lib.rs#item/function/alpha",
            "signature": "pub fn alpha()",
            "itemKind": "function",
            "itemName": "alpha",
            "captureName": "declaration.name",
            "sourceByteStart": source_byte_start,
            "sourceByteEnd": source_byte_end,
        }],
    })
}

fn projection(
    name: &str,
    source_byte_start: u64,
    source_byte_end: u64,
) -> ProviderSelectorProjection {
    ProviderSelectorProjection {
        structural_selector: format!("rust://src/lib.rs#item/function/{name}"),
        capture_name: "declaration.name".to_string(),
        signature: format!("pub fn {name}()"),
        item_kind: "function".to_string(),
        item_name: name.to_string(),
        source_byte_start,
        source_byte_end,
    }
}

fn item_names(projections: &[ProviderSelectorProjection]) -> Vec<&str> {
    projections
        .iter()
        .map(|projection| projection.item_name.as_str())
        .collect()
}

fn assert_zero_side_effects(receipt: &super::OwnerItemsExecutionReceipt) {
    assert_eq!(receipt.metadata_reads, 1);
    assert_eq!(receipt.source_byte_reads, 0);
    assert_eq!(receipt.provider_invocations, 0);
    assert_eq!(receipt.provider_parses, 0);
    assert_eq!(receipt.owner_index_writes, 0);
    assert_eq!(receipt.cas_writes, 0);
    assert_eq!(receipt.merkle_leaf_writes, 0);
    assert_eq!(receipt.merkle_path_node_writes, 0);
}

fn assert_refresh_side_effects(receipt: &super::OwnerItemsExecutionReceipt) {
    assert_eq!(receipt.metadata_reads, 1);
    assert_eq!(receipt.source_byte_reads, 1);
    assert_eq!(receipt.provider_invocations, 1);
    assert_eq!(receipt.provider_parses, 1);
    assert_eq!(receipt.owner_index_writes, 1);
    assert_eq!(receipt.cas_writes, 0);
    assert_eq!(receipt.merkle_leaf_writes, 1);
    assert!(receipt.merkle_path_node_writes >= 1);
}

fn expect_refresh(lookup: OwnerItemsLookup) -> agent_semantic_client_db::ProviderOwnerProbe {
    let OwnerItemsLookup::Refresh(probe) = lookup else {
        panic!("owner must require refresh");
    };
    probe
}

fn digest(path: &std::path::Path) -> String {
    let bytes = fs::read(path).expect("read digest fixture");
    agent_semantic_content_identity::ArtifactHash::blake3(bytes).value
}

struct WarmOwnerFixture {
    _state_home: StateHomeGuard,
    _environment: MutexGuard<'static, ()>,
    root: PathBuf,
    owner_path: PathBuf,
    runtime: tokio::runtime::Runtime,
    session: agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession,
    server_shutdown: Option<tokio::sync::watch::Sender<bool>>,
    server_task: Option<tokio::task::JoinHandle<Result<(), String>>>,
    scope: ProviderIncrementalScoped,
}

impl WarmOwnerFixture {
    fn new() -> Self {
        let environment = environment_lock();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("asp-owner-warm-{}-{unique}", std::process::id()));
        let state_home = StateHomeGuard::install(&root.join("state"));
        let owner_path = root.join("src/lib.rs");
        fs::create_dir_all(owner_path.parent().expect("owner parent"))
            .expect("create owner parent");
        fs::write(&owner_path, b"pub fn alpha() {}\npub struct Beta;\n")
            .expect("write owner fixture");
        let resolved = agent_semantic_client_core::state_core::ResolvedState::resolve(&root)
            .expect("resolve warm owner workspace");
        let scope = ProviderIncrementalScoped {
            project_root: resolved.workspace.root.to_string_lossy().into_owned(),
            workspace_identity: resolved.workspace.workspace_id.as_str().to_owned(),
            provider_workspace_identity_digest: "b".repeat(64),
            language_id: "rust".to_string(),
            provider_id: "rs-harness".to_string(),
            provider_workspace_root: resolved.workspace.root.to_string_lossy().into_owned(),
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build warm owner runtime");
        let registry = std::sync::Arc::new(WorkspaceDbRegistry::default());
        runtime
            .block_on(registry.bootstrap_workspace(&root))
            .expect("bootstrap warm owner workspace");
        let endpoint =
            agent_semantic_client_db::workspace_db_ipc::prepare_workspace_db_owner_endpoint(
                &root.join("runtime"),
                &scope.workspace_identity,
                1,
                "warm-owner-fixture",
            )
            .expect("prepare warm owner endpoint");
        let listener =
            agent_semantic_client_db::workspace_db_ipc::bind_workspace_db_owner(&endpoint)
                .expect("bind warm owner endpoint");
        let session = agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession::new(
            endpoint.clone(),
        );
        let last_activity = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let (server_shutdown, shutdown_rx) = tokio::sync::watch::channel(false);
        let server_task = runtime.spawn(async move {
            agent_semantic_client_db::workspace_db_ipc::serve_workspace_db_session_until_shutdown(
                &listener,
                &endpoint,
                registry,
                last_activity,
                shutdown_rx,
            )
            .await
        });
        Self {
            _state_home: state_home,
            _environment: environment,
            root,
            owner_path,
            runtime,
            session,
            server_shutdown: Some(server_shutdown),
            server_task: Some(server_task),
            scope,
        }
    }

    fn scope(&self) -> ProviderIncrementalScoped {
        self.scope.clone()
    }
}

fn environment_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("lock owner warm test environment")
}

struct StateHomeGuard {
    previous: Option<OsString>,
}

impl StateHomeGuard {
    fn install(state_home: &std::path::Path) -> Self {
        let previous = std::env::var_os("AST_STATE_HOME");
        unsafe {
            std::env::set_var("AST_STATE_HOME", state_home);
        }
        Self { previous }
    }
}

impl Drop for StateHomeGuard {
    fn drop(&mut self) {
        unsafe {
            if let Some(previous) = &self.previous {
                std::env::set_var("AST_STATE_HOME", previous);
            } else {
                std::env::remove_var("AST_STATE_HOME");
            }
        }
    }
}

impl Drop for WarmOwnerFixture {
    fn drop(&mut self) {
        if let Some(shutdown) = self.server_shutdown.take() {
            let _ = shutdown.send(true);
        }
        if let Some(task) = self.server_task.take() {
            let _ = self.runtime.block_on(task);
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}
