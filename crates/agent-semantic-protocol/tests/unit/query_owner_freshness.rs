use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[tokio::test(flavor = "multi_thread")]
async fn ensure_workspace_reports_generation_changes_for_live_sources() {
    let root = temp_project_root("ensure-workspace-generation-receipt");
    establish_rust_package(&root);
    let owner = root.join("src/lib.rs");
    fs::write(&owner, "pub fn baseline() -> u8 { 1 }\n").expect("write baseline owner");
    let runtime = ExactQueryRuntime::start(&root).await;
    let request = |id: &str| {
        let request_id = id.to_owned();
        let endpoint = runtime.endpoint.clone();
        let project_root = root.clone();
        async move {
            agent_semantic_client_db::runtime_server_control::ensure_runtime_server_workspace(
                &endpoint,
                &project_root,
                request_id,
            )
            .await
            .expect("ensure workspace")
        }
    };

    let baseline = request("ensure-baseline").await;
    let baseline_generation = baseline
        .workspace_generation
        .expect("baseline generation receipt");
    assert!(baseline_generation.generation_changed);

    fs::write(
        root.join("src/new_owner.rs"),
        "pub fn new_owner() -> u8 { 2 }\n",
    )
    .expect("write untracked owner");
    let advanced = request("ensure-untracked").await;
    let advanced_generation = advanced
        .workspace_generation
        .expect("advanced generation receipt");
    assert!(advanced_generation.generation_changed);
    assert_eq!(
        advanced_generation.active_generation_digest,
        advanced_generation.candidate_digest
    );

    let unchanged = request("ensure-unchanged").await;
    assert!(
        !unchanged
            .workspace_generation
            .expect("unchanged receipt")
            .generation_changed
    );

    fs::create_dir_all(root.join("target")).expect("create ignored target");
    fs::write(root.join("target/hidden.rs"), "pub fn hidden() {}\n").expect("write ignored owner");
    let ignored = request("ensure-ignored").await;
    assert!(
        !ignored
            .workspace_generation
            .expect("ignored receipt")
            .generation_changed
    );

    fs::remove_file(&owner).expect("delete tracked owner");
    let deleted = request("ensure-deleted").await;
    assert!(
        deleted
            .workspace_generation
            .expect("deleted receipt")
            .generation_changed
    );
    runtime.shutdown().await;
    let _ = fs::remove_dir_all(root);
}

#[tokio::test(flavor = "multi_thread")]
async fn exact_source_projection_consumes_the_pretool_admitted_owner_change() {
    let root = temp_project_root("exact-selector-freshness");
    establish_rust_package(&root);
    let owner = root.join("src/lib.rs");
    fs::write(&owner, "pub fn alpha() {\n    let value = 1;\n}\n").expect("write first source");
    let runtime = ExactQueryRuntime::start(&root).await;

    runtime
        .admit("alpha-initial", vec!["src/lib.rs".to_owned()], false)
        .await;
    let first = run_exact_selector_query(&root, &runtime.state_home).await;
    assert!(first.contains("let value = 1;"), "{first}");

    fs::write(&owner, "pub fn alpha() {\n    let value = 2;\n}\n").expect("write second source");
    runtime
        .admit("alpha-pretool-rewrite", vec!["src/lib.rs".to_owned()], true)
        .await;

    let refresh_started = std::time::Instant::now();
    let second = run_exact_selector_query(&root, &runtime.state_home).await;
    let refresh_elapsed = refresh_started.elapsed();
    assert!(second.contains("let value = 2;"), "{second}");
    assert!(
        !second.contains("let value = 1;"),
        "exact selector query returned stale source after owner rewrite: {second}"
    );
    assert!(
        refresh_elapsed < std::time::Duration::from_millis(250),
        "pre-tool admitted owner refresh exceeded the 250ms atomic publication budget: {refresh_elapsed:?}"
    );
    let warm_started = std::time::Instant::now();
    let warm = run_exact_selector_query(&root, &runtime.state_home).await;
    let warm_elapsed = warm_started.elapsed();
    assert!(warm.contains("let value = 2;"), "{warm}");
    assert!(
        warm_elapsed < std::time::Duration::from_millis(100),
        "unchanged owner exact query exceeded the 100ms warm budget: {warm_elapsed:?}"
    );
    println!(
        "[pretool-admitted-exact-performance] reconcileMicros={} reconcileBudgetMicros=250000 warmMicros={} warmBudgetMicros=100000",
        refresh_elapsed.as_micros(),
        warm_elapsed.as_micros()
    );

    runtime.shutdown().await;
    let _ = fs::remove_dir_all(root);
}

#[tokio::test(flavor = "multi_thread")]
async fn exact_source_projection_reconciles_unadmitted_owner_change_before_cached_hit() {
    let root = temp_project_root("exact-selector-read-side-freshness");
    establish_rust_package(&root);
    let owner = root.join("src/lib.rs");
    fs::write(&owner, "pub fn alpha() {\n    let value = 1;\n}\n").expect("write first source");
    let runtime = ExactQueryRuntime::start(&root).await;

    runtime
        .admit(
            "alpha-initial-read-side",
            vec!["src/lib.rs".to_owned()],
            false,
        )
        .await;
    let first = run_exact_selector_query(&root, &runtime.state_home).await;
    assert!(first.contains("let value = 1;"), "{first}");

    fs::write(&owner, "pub fn alpha() {\n    let value = 2;\n}\n")
        .expect("write unadmitted owner change");

    let refresh_started = std::time::Instant::now();
    let second = run_exact_selector_query(&root, &runtime.state_home).await;
    let refresh_elapsed = refresh_started.elapsed();
    assert!(second.contains("let value = 2;"), "{second}");
    assert!(
        !second.contains("let value = 1;"),
        "exact selector query returned a cached owner after an unadmitted checkout change: {second}"
    );
    assert!(
        refresh_elapsed < std::time::Duration::from_millis(250),
        "read-side owner reconciliation exceeded the 250ms publication budget: {refresh_elapsed:?}"
    );

    let warm_started = std::time::Instant::now();
    let warm = run_exact_selector_query(&root, &runtime.state_home).await;
    let warm_elapsed = warm_started.elapsed();
    assert!(warm.contains("let value = 2;"), "{warm}");
    assert!(
        warm_elapsed < std::time::Duration::from_millis(100),
        "unchanged cached owner exact query exceeded the 100ms warm budget: {warm_elapsed:?}"
    );
    println!(
        "[read-side-exact-performance] reconcileMicros={} reconcileBudgetMicros=250000 warmMicros={} warmBudgetMicros=100000",
        refresh_elapsed.as_micros(),
        warm_elapsed.as_micros()
    );

    runtime.shutdown().await;
    let _ = fs::remove_dir_all(root);
}

#[tokio::test(flavor = "multi_thread")]
async fn exact_selector_changed_and_moved_owner_never_requires_sync() {
    prewarm_cli_artifact().await;
    let root = temp_project_root("exact-selector-live-owner");
    establish_rust_package(&root);
    let original_owner = root.join("src/lib.rs");
    fs::write(&original_owner, "pub fn live_owner() -> u8 { 1 }\n")
        .expect("write initial live owner");
    let runtime = ExactQueryRuntime::start(&root).await;

    runtime
        .admit("live-owner-initial", vec!["src/lib.rs".to_owned()], false)
        .await;
    let (initial, initial_elapsed) = run_exact_selector_query_for(
        &root,
        &runtime.state_home,
        "rust://src/lib.rs#item/function/live_owner",
    )
    .await;
    assert!(initial.contains("{ 1 }"), "{initial}");
    assert!(
        initial_elapsed < std::time::Duration::from_millis(250),
        "initial live-owner exact query exceeded 250ms: {initial_elapsed:?}"
    );

    let (item_missing, item_missing_elapsed) = run_exact_selector_query_for(
        &root,
        &runtime.state_home,
        "rust://src/lib.rs#item/function/definitely_missing_live_owner_item",
    )
    .await;
    assert!(
        item_missing.contains("state=item-missing"),
        "{item_missing}"
    );
    assert!(
        item_missing.contains("reasonKind=item-not-in-live-owner"),
        "{item_missing}"
    );
    assert!(
        !item_missing.contains("exact-selector provider failed"),
        "{item_missing}"
    );
    assert!(
        !item_missing.contains(" next="),
        "a live-owner item miss is terminal and must not repeat discovery: {item_missing}"
    );
    assert!(
        item_missing_elapsed < std::time::Duration::from_millis(250),
        "missing item exact query exceeded 250ms: {item_missing_elapsed:?}"
    );

    fs::write(&original_owner, "pub fn live_owner() -> u8 { 2 }\n").expect("rewrite live owner");
    runtime
        .admit("live-owner-rewrite", vec!["src/lib.rs".to_owned()], true)
        .await;
    let (changed, changed_elapsed) = run_exact_selector_query_for(
        &root,
        &runtime.state_home,
        "rust://src/lib.rs#item/function/live_owner",
    )
    .await;
    assert!(changed.contains("{ 2 }"), "{changed}");
    assert!(!changed.contains("{ 1 }"), "{changed}");
    assert!(
        changed_elapsed < std::time::Duration::from_millis(100),
        "changed live-owner exact query exceeded 100ms: {changed_elapsed:?}"
    );

    let moved_owner = root.join("src/moved.rs");
    fs::rename(&original_owner, &moved_owner).expect("move live owner");
    runtime
        .admit(
            "live-owner-move",
            vec!["src/lib.rs".to_owned(), "src/moved.rs".to_owned()],
            true,
        )
        .await;
    let (relocated, relocated_elapsed) = run_exact_selector_query_for(
        &root,
        &runtime.state_home,
        "rust://src/lib.rs#item/function/live_owner",
    )
    .await;
    assert!(relocated.contains("{ 2 }"), "{relocated}");
    assert!(!relocated.contains("selector-stale"), "{relocated}");
    assert!(
        relocated_elapsed < std::time::Duration::from_millis(100),
        "relocated live-owner exact query exceeded 100ms: {relocated_elapsed:?}"
    );

    let (moved, moved_elapsed) = run_exact_selector_query_for(
        &root,
        &runtime.state_home,
        "rust://src/moved.rs#item/function/live_owner",
    )
    .await;
    assert!(moved.contains("{ 2 }"), "{moved}");
    assert!(
        moved_elapsed < std::time::Duration::from_millis(100),
        "moved live-owner exact query exceeded 100ms: {moved_elapsed:?}"
    );
    println!(
        "[exact-live-owner-performance] cliArtifactPrewarmed=1 initialRuntimeGenerationCold=1 initialMicros={} itemMissingMicros={} changedMicros={} relocatedMicros={} movedMicros={} syncCount=0 wrapperByteAuthority=0",
        initial_elapsed.as_micros(),
        item_missing_elapsed.as_micros(),
        changed_elapsed.as_micros(),
        relocated_elapsed.as_micros(),
        moved_elapsed.as_micros()
    );

    runtime.shutdown().await;
    let _ = fs::remove_dir_all(root);
}

async fn prewarm_cli_artifact() {
    let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .env_clear()
        .arg("--version")
        .output()
        .await
        .expect("prewarm freshly linked asp test artifact");
    assert!(
        output.status.success(),
        "asp test artifact prewarm failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

async fn run_exact_selector_query(root: &Path, state_home: &Path) -> String {
    run_exact_selector_query_for(root, state_home, "rust://src/lib.rs#item/function/alpha")
        .await
        .0
}

fn establish_rust_package(root: &Path) {
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"exact-selector-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write Cargo project entry");
    let git_init = std::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(root)
        .status()
        .expect("initialize exact-query Git fixture");
    assert!(
        git_init.success(),
        "exact-query Git fixture initialization failed"
    );
}

async fn run_exact_selector_query_for(
    root: &Path,
    state_home: &Path,
    selector: &str,
) -> (String, std::time::Duration) {
    eprintln!(
        "[exact-query-process] binary={} root={}",
        env!("CARGO_BIN_EXE_asp"),
        root.display()
    );
    let started = std::time::Instant::now();
    let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(root)
        .env_clear()
        .env("HOME", std::env::var_os("HOME").unwrap_or_default())
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("ASP_STATE_HOME", state_home)
        .env("ASP_EXACT_QUERY_TRACE", "1")
        .arg("rust")
        .arg("query")
        .arg("--selector")
        .arg(selector)
        .arg("--workspace")
        .arg(root)
        .arg("--projection")
        .arg("source")
        .output()
        .await
        .expect("run asp query");
    let process_elapsed = started.elapsed();
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprint!("{stderr}");
    eprintln!(
        "[exact-query-process] processMicros={} pipeDrainMicros={}",
        process_elapsed.as_micros(),
        0
    );
    let response = if output.status.success() {
        String::from_utf8(output.stdout).expect("query stdout")
    } else {
        stderr.into_owned()
    };
    (response, started.elapsed())
}

struct ExactQueryRuntime {
    state_home: PathBuf,
    endpoint: agent_semantic_client_db::RuntimeServerEndpoint,
    workspace_identity: String,
    project_root: PathBuf,
    shutdown: agent_semantic_client_db::runtime_server::RuntimeServerShutdownHandle,
    hook_admission_locator: agent_semantic_client_db::runtime_server_hook_admission_locator::RuntimeHookAdmissionLocatorTask,
    server: tokio::task::JoinHandle<
        Result<agent_semantic_client_db::runtime_server::RuntimeServerExit, String>,
    >,
}

impl ExactQueryRuntime {
    async fn start(project_root: &Path) -> Self {
        let project_root =
            fs::canonicalize(project_root).expect("canonicalize exact-query fixture");
        let state_id = blake3::hash(project_root.to_string_lossy().as_bytes())
            .to_hex()
            .to_string();
        let state_home = Path::new("/tmp").join(format!("asp-q-{}", &state_id[..16]));
        std::fs::create_dir_all(&state_home).expect("create exact-query Runtime state directory");
        #[cfg(unix)]
        std::fs::set_permissions(
            &state_home,
            std::os::unix::fs::PermissionsExt::from_mode(0o700),
        )
        .expect("secure exact-query Runtime state directory");
        let artifact_catalog =
            agent_semantic_runtime::runtime_artifact_catalog::RuntimeArtifactCatalog::new(
                agent_semantic_config::runtime_dev::RuntimeArtifactMode::Release,
            );
        let endpoint =
            agent_semantic_client_db::runtime_server_control::prepare_runtime_server_endpoint_in(
                &state_home,
                Path::new(env!("CARGO_BIN_EXE_asp")),
                "exact-query-test-artifact",
                artifact_catalog.mode_label(),
                &artifact_catalog.digest(),
                1,
                "exact-query-test-binding",
            )
            .await
            .expect("prepare exact-query Runtime Server endpoint");
        agent_semantic_client_db::runtime_server_control::publish_runtime_server_endpoint(
            &agent_semantic_client_db::runtime_server_endpoint_path(&state_home)
                .expect("resolve host-local Runtime endpoint authority"),
            &endpoint,
        )
        .await
        .expect("publish exact-query Runtime Server endpoint");
        let catalog = agent_semantic_client_db::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog::load(
            state_home.join("runtime/server/workspace-admissions.v1.json"),
        )
        .await
        .expect("load exact-query admission catalog");
        let hook_admission_locator = agent_semantic_client_db::runtime_server_hook_admission_locator::spawn_runtime_hook_admission_locator(
            state_home.clone(),
            endpoint.clone(),
            catalog.clone(),
        )
        .await
        .expect("start exact-query Hook admission locator");
        let builder: agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCandidateBuilder = {
        Arc::new(move |workspace_identity, project_root, _cancellation, _provider_target| {
                Box::pin(build_exact_query_generation(workspace_identity, project_root))
            })
        };
        let owner_builder: agent_semantic_client_db::runtime_server_admission::WorkspaceOwnerProjectionBuilder =
            Arc::new(move |_workspace_identity, project_root, owner_path| {
                Box::pin(async move {
                    let bytes = tokio::fs::read(project_root.join(&owner_path))
                        .await
                        .map_err(|error| format!("read exact-query owner fixture: {error}"))?;
                    build_exact_query_generation_owner(
                        agent_semantic_client_db::runtime_server_workspace::WorkspaceOwnerSnapshot {
                            content_digest: format!(
                                "blake3-256:{}",
                                blake3::hash(&bytes).to_hex()
                            ),
                            owner_path,
                            bytes,
                            selectors: Vec::new(),
                        },
                    )
                    .await
                })
            });
        let workspace_store = agent_semantic_client_db::runtime_server_workspace::prepare_runtime_server_workspace_store_at_root(
            endpoint.workspace_store_path.clone().into(),
        )
        .await
        .expect("prepare exact-query Runtime Server workspace store");
        let server =
            agent_semantic_client_db::runtime_server::RuntimeServer::bind_with_artifact_catalog(
                endpoint.clone(),
                Arc::new(
                    agent_semantic_client_db::WorkspaceDbRegistry::with_state_home(PathBuf::from(
                        &endpoint.workspace_store_path,
                    )),
                ),
                workspace_store,
                Arc::new(artifact_catalog),
            )
            .await
            .expect("bind exact-query Runtime Server")
            .with_workspace_generation_and_owner_builders_and_catalog(
                builder,
                owner_builder,
                catalog,
            );
        let shutdown = server.shutdown_handle();
        let server = tokio::spawn(server.serve());
        let workspace_identity =
            agent_semantic_client_core::state_core::ResolvedState::resolve(&project_root)
                .expect("resolve exact-query workspace identity")
                .workspace
                .workspace_id
                .to_string();
        Self {
            state_home,
            endpoint,
            workspace_identity,
            project_root,
            shutdown,
            hook_admission_locator,
            server,
        }
    }

    async fn admit(
        &self,
        mutation_id: &str,
        changed_paths: Vec<String>,
        require_generation_advance: bool,
    ) {
        let session =
            agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession::for_runtime_server(
                &self.endpoint,
                self.workspace_identity.clone(),
                self.project_root.clone(),
            );
        let previous_generation_digest = session
            .runtime_search_generation_authority()
            .await
            .ok()
            .map(|authority| authority.generation_digest);
        let expected_generation_digest = if previous_generation_digest.is_none() {
            session
                .admit_runtime_generation_for_read("rust", "asp-rust")
                .await
                .expect("admit initial exact-query fixture generation for read");
            session
                .runtime_search_generation_authority()
                .await
                .expect("read initial exact-query fixture authority")
                .generation_digest
        } else {
            let admitted = session
                .admit_runtime_generation(mutation_id, changed_paths)
                .await
                .expect("admit exact-query fixture mutation");
            assert!(admitted.changed_path_count > 0, "{admitted:?}");
            assert_eq!(
                admitted.affected_workspace_count,
                admitted.receipts.len(),
                "{admitted:?}"
            );
            let receipt = admitted
                .receipts
                .first()
                .expect("exact-query mutation affects one workspace");
            let commit = receipt.commit.as_ref().unwrap_or_else(|| {
                panic!(
                    "exact-query mutation did not commit: state={:?} error={:?} failureStage={:?}",
                    receipt.state, receipt.error, receipt.failure_stage
                )
            });
            if require_generation_advance {
                assert_ne!(
                    previous_generation_digest.as_deref(),
                    Some(commit.generation_digest.as_str()),
                    "exact-query mutation reused its previous generation: {receipt:?}"
                );
            }
            commit.generation_digest.clone()
        };
        let authority = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                if let Ok(authority) = session.runtime_search_generation_authority().await {
                    if authority.generation_digest == expected_generation_digest {
                        break authority;
                    }
                }
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            }
        })
        .await
        .unwrap_or_else(|error| {
            panic!(
                "exact-query fixture mutation {mutation_id} did not publish {expected_generation_digest} from {previous_generation_digest:?}: {error:?}"
            )
        });
        assert_eq!(authority.workspace_identity, self.workspace_identity);
    }

    async fn shutdown(self) {
        self.hook_admission_locator
            .shutdown()
            .await
            .expect("stop exact-query Hook admission locator");
        self.shutdown.shutdown();
        self.server
            .await
            .expect("join exact-query Runtime Server")
            .expect("serve exact-query Runtime Server");
        let _ = tokio::fs::remove_dir_all(self.state_home).await;
    }
}

async fn build_exact_query_generation_owner(
    mut owner: agent_semantic_client_db::runtime_server_workspace::WorkspaceOwnerSnapshot,
) -> Result<agent_semantic_client_db::runtime_server_workspace::WorkspaceOwnerSnapshot, String> {
    let source = String::from_utf8_lossy(&owner.bytes);
    let item_name = if source.contains("live_owner") {
        "live_owner"
    } else if source.contains("alpha") {
        "alpha"
    } else {
        return Err(format!(
            "exact-query freshness owner has no known item: {}",
            owner.owner_path
        ));
    };
    let structural_selector = format!("rust://{}#item/function/{item_name}", owner.owner_path);
    let projection_payload = serde_json::json!({
        "rootNodeId": "callable:root",
        "callable": {
            "kind": "function",
            "displayName": item_name,
            "signature": format!("fn {item_name}()")
        },
        "nodes": [{
            "nodeId": "callable:root",
            "kind": "callable",
            "label": item_name,
            "order": 0,
            "queryable": false
        }],
        "relations": [],
        "cost": {
            "sourceBytes": owner.bytes.len(),
            "projectedBytes": owner.bytes.len(),
            "omittedBytes": 0
        }
    });
    let generation_identity_digest = "0".repeat(64);
    let parser_identity_digest = "1".repeat(64);
    let query_pack_digest = "2".repeat(64);
    let mut evidence_hasher = blake3::Hasher::new();
    evidence_hasher.update(b"asp.projection-evidence-context.v1\0");
    for component in [
        "rust",
        "asp-rust",
        generation_identity_digest.as_str(),
        parser_identity_digest.as_str(),
        query_pack_digest.as_str(),
    ] {
        evidence_hasher.update(&(component.len() as u64).to_le_bytes());
        evidence_hasher.update(component.as_bytes());
    }
    let evidence_context_ref = format!("blake3-256:{}", evidence_hasher.finalize().to_hex());
    let projection = agent_semantic_content_identity::semantic_projection::SemanticProjection::new(
        "callable-skeleton",
        "rust",
        "asp-rust",
        structural_selector.as_str(),
        evidence_context_ref.clone(),
        "agent.semantic-protocols.callable-skeleton",
        projection_payload,
    )
    .map_err(|error| format!("build callable skeleton fixture: {error}"))?;
    let mut projection = serde_json::to_value(projection)
        .map_err(|error| format!("encode callable skeleton fixture: {error}"))?;
    let typed_payload: agent_semantic_content_identity::callable_skeleton_projection::CallableSkeletonPayload =
        serde_json::from_value(projection["payload"].clone())
            .map_err(|error| format!("decode callable skeleton payload: {error}"))?;
    let typed_payload_bytes = serde_json::to_vec(&typed_payload)
        .map_err(|error| format!("encode callable skeleton payload: {error}"))?;
    projection["payloadDigest"] =
        format!("blake3-256:{}", blake3::hash(&typed_payload_bytes).to_hex()).into();
    owner.selectors = vec![
        agent_semantic_client_db::runtime_server_workspace::WorkspaceSelectorSnapshot {
            selector: structural_selector,
            byte_start: 0,
            byte_end: owner.bytes.len(),
            derived_projections: vec![
            agent_semantic_client_db::runtime_server_workspace::WorkspaceDerivedProjectionSnapshot {
                projection_kind:
                    agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind::CallableSkeleton,
                bytes: serde_json::to_vec(&projection)
                    .map_err(|error| format!("serialize callable skeleton fixture: {error}"))?,
                evidence_context: Some(
                    agent_semantic_content_identity::projection_evidence_context::ProjectionEvidenceContext {
                        schema_id: "agent.semantic-protocols.projection-evidence-context".to_owned(),
                        schema_version: "1".to_owned(),
                        evidence_context_ref,
                        language_id: "rust".to_owned(),
                        provider_id: "asp-rust".to_owned(),
                        generation_identity_digest,
                        parser_identity_digest,
                        query_pack_digest,
                    },
                ),
            },
        ],
        },
    ];
    Ok(owner)
}

async fn build_exact_query_generation(
    workspace_identity: String,
    project_root: PathBuf,
) -> Result<agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationBuild, String> {
    use sha2::Digest as _;

    let candidate = agent_semantic_client_db::runtime_server_admission::discover_workspace_generation_candidate(
        &project_root,
    )
    .await?;
    let mut owners = Vec::new();
    let mut import_hashes = Vec::new();
    let mut import_files = Vec::new();
    for owner_path in ["src/lib.rs", "src/moved.rs"] {
        let bytes = match tokio::fs::read(project_root.join(owner_path)).await {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("read exact-query owner {owner_path}: {error}")),
        };
        let content_digest = format!("blake3-256:{}", blake3::hash(&bytes).to_hex());
        let sha256 = format!("{:x}", sha2::Sha256::digest(&bytes));
        let source = String::from_utf8_lossy(&bytes).into_owned();
        let item_name = if source.contains("live_owner") {
            "live_owner"
        } else if source.contains("alpha") {
            "alpha"
        } else {
            return Err(format!(
                "exact-query fixture owner has no known item: {owner_path}"
            ));
        };
        let selector_id = format!("rust://{owner_path}#item/function/{item_name}");
        let selector_receipt = agent_semantic_client_db::ClientDbSourceIndexSelector {
            owner_path: owner_path.into(),
            provider_id: "asp-rust".into(),
            selector_id: selector_id.as_str().into(),
            symbol: Some(item_name.into()),
            kind: Some("function".into()),
            source: source.clone().into(),
            query_keys: vec![item_name.into()],
            derived_projections: Vec::new(),
            projection_record: exact_projection_record(
                owner_path,
                selector_id.as_str(),
                bytes.as_slice(),
            ),
        };
        import_hashes.push(agent_semantic_client_core::ClientCacheFileHash {
            path: owner_path.to_owned(),
            sha256,
            byte_len: bytes.len() as u64,
            mtime_ms: 0,
        });
        import_files.push(agent_semantic_client_db::ClientDbSourceIndexImportFile {
            relations: Vec::new(),
            relative_path: owner_path.to_owned(),
            language_id: "rust".into(),
            provider_id: "asp-rust".into(),
            text: source,
            selectors: vec![selector_receipt],
        });
        owners.push(
            build_exact_query_generation_owner(
                agent_semantic_client_db::runtime_server_workspace::WorkspaceOwnerSnapshot {
                    owner_path: owner_path.to_owned(),
                    content_digest,
                    selectors: vec![
                    agent_semantic_client_db::runtime_server_workspace::WorkspaceSelectorSnapshot {
                        selector: selector_id,
                        byte_start: 0,
                        byte_end: bytes.len(),
                        derived_projections: Vec::new(),
                    },
                ],
                    bytes,
                },
            )
            .await?,
        );
    }
    let source_blobs = agent_semantic_client_db::ClientDbSourceIndexSourceBlobs::from_normalized(
        owners.iter().map(|owner| {
            (
                agent_semantic_client_db::ClientDbSourceIndexPath::new(&owner.owner_path),
                owner.bytes.clone(),
            )
        }),
    );
    let workspace_snapshot =
        agent_semantic_content_identity::WorkspaceSnapshot::from_file_bytes(source_blobs.iter());
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        format!(
            "blake3-256:{}",
            blake3::hash(b"exact-query-test-provider").to_hex()
        ),
    );
    let import = agent_semantic_client_db::build_source_index_import(
        agent_semantic_client_db::ClientDbSourceIndexImportRequest {
            source_blobs: source_blobs.clone(),
            generation_id:
                agent_semantic_client_db::client_db_source_index_generation_id_for_snapshot(
                    &source_snapshot,
                ),
            project_root,
            schema_id: agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_SCHEMA_ID.into(),
            schema_version: agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION.into(),
            selector_source: agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_PROVIDER_ID.into(),
            file_hashes: import_hashes,
            files: import_files,
        },
    )?;
    let file_count = u32::try_from(import.file_hashes.len())
        .map_err(|_| "exact-query fixture file count overflow".to_owned())?;
    let materialization = agent_semantic_client_db::runtime_server_workspace::WorkspaceCanonicalMaterialization::from_source_index(
        workspace_identity,
        &source_snapshot,
        &import,
        &source_blobs,
        Vec::new(),
    )?;
    Ok(
        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationBuild::new(
            candidate,
            agent_semantic_client_db::ClientDbSourceIndexRefreshRequest {
                import,
                file_count,
                source_snapshot,
            },
            materialization,
        ),
    )
}

fn exact_projection_record(
    owner_path: &str,
    structural_selector: &str,
    source: &[u8],
) -> agent_semantic_content_identity::ExactSelectorProjectionRecordV1 {
    use agent_semantic_content_identity::exact_selector_merkle::{
        ExactProjectionModeV1, blake3_content_digest_v1, canonical_content_digest,
    };
    let source_digest = blake3_content_digest_v1(source);
    let tree = agent_semantic_content_identity::workspace_merkle_v1::WorkspacePathMerkleTreeV1::from_file_digests([(
        owner_path.to_owned(),
        source_digest,
    )])
    .expect("exact-query workspace tree");
    let language_id = agent_semantic_content_identity::exact_selector_projection_packet::ProjectionPacketLanguageIdV1::from("rust");
    let provider_id = agent_semantic_content_identity::exact_selector_projection_packet::ProjectionPacketProviderIdV1::from("asp-rust");
    let packet_owner = agent_semantic_content_identity::exact_selector_projection_packet::ProjectionPacketOwnerPathV1::from(owner_path);
    let packet_selector = agent_semantic_content_identity::exact_selector_projection_packet::ProjectionPacketStructuralSelectorV1::from(structural_selector);
    let parser_identity = canonical_content_digest(b"parser", &[b"asp-rust"]);
    let query_pack_identity = canonical_content_digest(b"query-pack", &[b"rust"]);
    let canonical =
        agent_semantic_content_identity::CanonicalItemSelector::parse(structural_selector)
            .expect("canonical exact-query fixture selector");
    agent_semantic_content_identity::exact_selector_projection_packet::build_exact_selector_projection_packet_v1(
        agent_semantic_content_identity::exact_selector_projection_packet::ExactSelectorProjectionPacketV1Input {
            language_id: &language_id,
            provider_id: &provider_id,
            canonical_item_selector: canonical,
            parser_identity_digest: &parser_identity,
            query_pack_digest: &query_pack_identity,
            owner_path: &packet_owner,
            structural_selector: &packet_selector,
            projection_mode: ExactProjectionModeV1::Code,
            source_byte_start: 0,
            source_byte_end: source.len() as u64,
            source,
            normalized_parser_facts: structural_selector.as_bytes(),
            projection: source,
        },
    )
    .enrich_projection_record(&tree)
    .expect("exact-query projection record")
}

fn temp_project_root(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_nanos();
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace root")
        .join("target/asp-test-workspaces")
        .join(format!("asp-{name}-{nonce}"))
}
