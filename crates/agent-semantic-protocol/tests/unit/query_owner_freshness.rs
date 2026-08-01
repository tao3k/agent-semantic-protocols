use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[tokio::test(flavor = "multi_thread")]
async fn exact_selector_code_reads_modified_source_without_stale_index() {
    let root = temp_project_root("exact-selector-freshness");
    establish_rust_package(&root);
    let owner = root.join("src/lib.rs");
    fs::write(&owner, "pub fn alpha() {\n    let value = 1;\n}\n").expect("write first source");
    let runtime = ExactQueryRuntime::start(&root).await;

    runtime.admit().await;
    let first = run_exact_selector_query(&root, &runtime.state_home).await;
    assert!(first.contains("let value = 1;"), "{first}");

    fs::write(&owner, "pub fn alpha() {\n    let value = 2;\n}\n").expect("write second source");
    runtime.admit().await;

    let second = run_exact_selector_query(&root, &runtime.state_home).await;
    assert!(second.contains("let value = 2;"), "{second}");
    assert!(
        !second.contains("let value = 1;"),
        "exact selector query returned stale source after owner rewrite: {second}"
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

    runtime.admit().await;
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
        item_missing_elapsed < std::time::Duration::from_millis(250),
        "missing item exact query exceeded 250ms: {item_missing_elapsed:?}"
    );

    fs::write(&original_owner, "pub fn live_owner() -> u8 { 2 }\n").expect("rewrite live owner");
    runtime.admit().await;
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
    runtime.admit().await;
    let (missing, missing_elapsed) = run_exact_selector_query_for(
        &root,
        &runtime.state_home,
        "rust://src/lib.rs#item/function/live_owner",
    )
    .await;
    assert!(missing.contains("state=selector-stale"), "{missing}");
    assert!(
        missing.contains("reasonKind=selector-not-in-active-generation"),
        "{missing}"
    );
    assert!(missing.contains("activeGenerationDigest="), "{missing}");
    assert!(
        missing.contains(
            "next=asp rust search lexical --query 'live_owner' --query 'function live_owner'"
        ),
        "{missing}"
    );
    assert!(!missing.contains("No such file or directory"), "{missing}");
    assert!(
        missing_elapsed < std::time::Duration::from_millis(100),
        "missing live-owner exact query exceeded 100ms: {missing_elapsed:?}"
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
        "[exact-live-owner-performance] cliArtifactPrewarmed=1 initialRuntimeGenerationCold=1 initialMicros={} itemMissingMicros={} changedMicros={} missingMicros={} movedMicros={} syncCount=0 wrapperByteAuthority=0",
        initial_elapsed.as_micros(),
        item_missing_elapsed.as_micros(),
        changed_elapsed.as_micros(),
        missing_elapsed.as_micros(),
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
            &agent_semantic_client_db::runtime_server_endpoint_path(&state_home),
            &endpoint,
        )
        .await
        .expect("publish exact-query Runtime Server endpoint");
        let catalog = agent_semantic_client_db::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog::load(
            state_home.join("runtime/server/workspace-admissions.v1.json"),
        )
        .await
        .expect("load exact-query admission catalog");
        let builder: agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCandidateBuilder = {
            Arc::new(move |workspace_identity, project_root| {
                Box::pin(build_exact_query_generation(workspace_identity, project_root))
            })
        };
        let server =
            agent_semantic_client_db::runtime_server::RuntimeServer::bind_with_artifact_catalog(
                endpoint.clone(),
                Arc::new(
                    agent_semantic_client_db::WorkspaceDbRegistry::with_state_home(
                        agent_semantic_client_db::runtime_server_runtime_base().join("workspaces"),
                    ),
                ),
                Arc::new(artifact_catalog),
            )
            .await
            .expect("bind exact-query Runtime Server")
            .with_workspace_generation_builder_and_catalog(builder, catalog)
            .with_workspace_owner_projection_builder(Arc::new(
                |_language_id, _project_root, owner| {
                    Box::pin(build_exact_query_owner_projection(owner))
                },
            ));
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
            server,
        }
    }

    async fn admit(&self) {
        let session =
            agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession::for_runtime_server(
                &self.endpoint,
                self.workspace_identity.clone(),
                self.project_root.clone(),
            );
        let admitted = session
            .admit_runtime_generation()
            .await
            .expect("admit exact-query fixture generation");
        assert!(admitted.accepted, "{admitted:?}");
        let receipt = session
            .ensure_runtime_generation()
            .await
            .expect("finish exact-query fixture generation");
        assert_eq!(
            receipt.state,
            agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionState::Ready,
            "{receipt:?}"
        );
    }

    async fn shutdown(self) {
        self.shutdown.shutdown();
        self.server
            .await
            .expect("join exact-query Runtime Server")
            .expect("serve exact-query Runtime Server");
        let _ = tokio::fs::remove_dir_all(self.state_home).await;
    }
}

async fn build_exact_query_owner_projection(
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
    owner.selectors = vec![
        agent_semantic_client_db::runtime_server_workspace::WorkspaceSelectorSnapshot {
            selector: format!(
                "rust://{}#item/function/{item_name}",
                owner.owner_path
            ),
            byte_start: 0,
            byte_end: owner.bytes.len(),
            derived_projections: vec![
                agent_semantic_client_db::runtime_server_workspace::WorkspaceDerivedProjectionSnapshot {
                    projection_kind: "callable-skeleton".to_owned(),
                    bytes: format!("fn {item_name}(...)" ).into_bytes(),
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
            provider_id: "rs-harness".into(),
            selector_id: selector_id.as_str().into(),
            symbol: Some(item_name.into()),
            kind: Some("function".into()),
            source: source.clone().into(),
            query_keys: vec![item_name.into()],
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
            relative_path: owner_path.to_owned(),
            language_id: "rust".into(),
            provider_id: "rs-harness".into(),
            text: source,
            selectors: vec![selector_receipt],
        });
        owners.push(
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
        );
    }
    let workspace_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_bytes(
        owners
            .iter()
            .map(|owner| (owner.owner_path.as_str(), owner.bytes.as_slice())),
    );
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        format!(
            "blake3-256:{}",
            blake3::hash(b"exact-query-test-provider").to_hex()
        ),
    );
    let import = agent_semantic_client_db::build_source_index_import(
        agent_semantic_client_db::ClientDbSourceIndexImportRequest {
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
    let materialization =
        agent_semantic_client_db::runtime_server_workspace::WorkspaceCanonicalMaterialization::new(
            workspace_identity,
            source_snapshot.clone(),
            &import,
            [1, 0],
            owners,
            Vec::new(),
        )?;
    Ok(
        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationBuild::new(
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
        ExactProjectionModeV1, blake3_content_digest_v1, canonical_content_digest_v1,
    };
    let source_digest = blake3_content_digest_v1(source);
    let tree = agent_semantic_content_identity::workspace_merkle_v1::WorkspacePathMerkleTreeV1::from_file_digests([(
        owner_path.to_owned(),
        source_digest,
    )])
    .expect("exact-query workspace tree");
    let language_id = agent_semantic_content_identity::exact_selector_projection_packet::ProjectionPacketLanguageIdV1::from("rust");
    let provider_id = agent_semantic_content_identity::exact_selector_projection_packet::ProjectionPacketProviderIdV1::from("rs-harness");
    let packet_owner = agent_semantic_content_identity::exact_selector_projection_packet::ProjectionPacketOwnerPathV1::from(owner_path);
    let packet_selector = agent_semantic_content_identity::exact_selector_projection_packet::ProjectionPacketStructuralSelectorV1::from(structural_selector);
    let parser_identity = canonical_content_digest_v1(b"parser", &[b"rs-harness"]);
    let query_pack_identity = canonical_content_digest_v1(b"query-pack", &[b"rust"]);
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
    std::env::temp_dir().join(format!("asp-{name}-{nonce}"))
}
