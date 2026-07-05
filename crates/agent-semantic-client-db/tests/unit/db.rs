use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, path::PathBuf, process::Command};

use agent_semantic_client_core::{
    CacheGenerationId, ClientCacheFileHash, LanguageId, ProviderId, SemanticSchemaId,
    SemanticSchemaVersion, state_core::ResolvedState,
};
use agent_semantic_client_db::{
    AGENT_SESSION_REGISTRY_DB_NAME, AgentSessionRegisterRequest, AgentSessionRegistry,
    AgentSessionToolEventRequest, CLIENT_DB_SOURCE_INDEX_PROVIDER_ID,
    CLIENT_DB_SOURCE_INDEX_SCHEMA_ID, CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION,
    ClientDbSourceIndexImportAssemblyRequest, ClientDbSourceIndexImportFile,
    ClientDbSourceIndexImportRequest, ClientDbSourceIndexRefreshRequest,
    ClientDbSourceIndexScopeFile, ClientDbSourceIndexSource, build_source_index_import,
    client_db_source_index_file_count, source_index_evidence_graph, source_index_relative_path,
    source_index_scope_dirs,
};

static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct EnvVarGuard {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvVarGuard {
    fn set_path(key: &'static str, value: &std::path::Path) -> Self {
        let previous = std::env::var_os(key);
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        unsafe {
            match self.previous.as_ref() {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }
}

#[test]
fn schema_version_stays_on_first_turso_release_contract() {
    assert_eq!(
        agent_semantic_client_db::AGENT_SEMANTIC_CLIENT_DB_SCHEMA_VERSION,
        1
    );
}

#[test]
fn agent_session_registry_storage_is_turso_owned() {
    let root = temp_root("agent-session-registry");
    let state_root = root.join("agent");
    let db_path = AgentSessionRegistry::db_path_for_state_root(&state_root);

    assert_eq!(
        db_path.file_name().and_then(|name| name.to_str()),
        Some(AGENT_SESSION_REGISTRY_DB_NAME)
    );
    assert_eq!(AGENT_SESSION_REGISTRY_DB_NAME, "session-registry.turso");
    assert!(
        AgentSessionRegistry::open_existing_state_root(&state_root)
            .expect("open missing session registry")
            .is_none()
    );

    let registry =
        AgentSessionRegistry::open_or_create_state_root(&state_root).expect("create registry");
    let record = registry
        .register_session(AgentSessionRegisterRequest {
            project_id: "project-1",
            root_session_id: "root-session",
            session_id: "child-session",
            parent_session_id: Some("parent-session"),
            name: "asp-explore",
            role: "search",
            model: Some("gpt-test"),
            status: "active",
            expires_at: Some(1_900_000_000),
            metadata_json: "{\"route\":\"db-owned\"}",
            now: 1_800_000_000,
        })
        .expect("register session through Turso DB crate");

    assert_eq!(record.root_session_id, "root-session");
    assert_eq!(record.session_id, "child-session");
    assert!(record.is_routable_at(1_800_000_001));
    assert_eq!(
        registry
            .query_sessions("project-1", Some("root-session"), Some("asp-explore"))
            .expect("query session")
            .len(),
        1
    );

    assert!(
        registry
            .record_tool_event(AgentSessionToolEventRequest {
                session_id: "child-session",
                tool_event: "search",
                command: Some("asp rust search owner"),
                evidence_ref: Some("receipt:1"),
                now: 1_800_000_010,
            })
            .expect("record tool event")
    );
    let updated = registry
        .session_by_id("project-1", "child-session")
        .expect("lookup updated session")
        .expect("session exists");
    assert_eq!(updated.last_tool_event.as_deref(), Some("search"));
    assert_eq!(
        updated.last_command.as_deref(),
        Some("asp rust search owner")
    );
    assert_eq!(updated.last_evidence_ref.as_deref(), Some("receipt:1"));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn agent_session_registry_project_open_uses_asp_home_db() {
    let root = temp_root("agent-session-registry-state-home");
    let state_home = root.join("state");
    let project_root = root.join("project");
    std::fs::create_dir_all(&project_root).expect("create project root");

    let state =
        ResolvedState::resolve_with_state_home(&project_root, &state_home).expect("resolve state");
    state.ensure_minimal_layout().expect("ensure state layout");

    let _env_lock = ENV_LOCK.lock().expect("lock env");
    let _state_home_guard = EnvVarGuard::set_path("ASP_STATE_HOME", &state_home);
    let state_root =
        AgentSessionRegistry::state_root_for_project(&project_root).expect("resolve project root");
    assert_eq!(state_root, state.state_home);
    let registry =
        AgentSessionRegistry::open_or_create_project(&project_root).expect("create registry");

    assert_eq!(
        registry.db_path(),
        &state_root.join(AGENT_SESSION_REGISTRY_DB_NAME)
    );
    assert!(registry.db_path().is_file());
    assert_eq!(
        registry.db_path(),
        &state.state_home.join(AGENT_SESSION_REGISTRY_DB_NAME)
    );
    assert!(
        !state
            .paths
            .project_dir
            .join(AGENT_SESSION_REGISTRY_DB_NAME)
            .exists(),
        "agent session registry must not create a project-id DB"
    );
    assert!(
        !state
            .paths
            .client_dir
            .join("agent")
            .join(AGENT_SESSION_REGISTRY_DB_NAME)
            .exists(),
        "agent session registry must not create a project-local DB"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn agent_session_register_moves_same_child_from_stale_root_mapping() {
    let root = temp_root("agent-session-registry-move-child");
    let state_root = root.join("agent");
    let registry =
        AgentSessionRegistry::open_or_create_state_root(&state_root).expect("create registry");
    registry
        .register_session(AgentSessionRegisterRequest {
            project_id: "project-1",
            root_session_id: "old-root",
            session_id: "child-session",
            parent_session_id: Some("old-root"),
            name: "asp-explore",
            role: "asp-explore",
            model: None,
            status: "closed",
            expires_at: None,
            metadata_json: "{}",
            now: 1_800_000_000,
        })
        .expect("register stale mapping");

    let record = registry
        .register_session(AgentSessionRegisterRequest {
            project_id: "project-1",
            root_session_id: "new-root",
            session_id: "child-session",
            parent_session_id: Some("new-root"),
            name: "asp-explore",
            role: "asp-explore",
            model: None,
            status: "active",
            expires_at: None,
            metadata_json: "{}",
            now: 1_800_000_010,
        })
        .expect("move stale child mapping to new root");

    assert_eq!(record.root_session_id, "new-root");
    assert_eq!(
        registry
            .query_sessions("project-1", Some("old-root"), Some("asp-explore"))
            .expect("query old root")
            .len(),
        0
    );
    assert_eq!(
        registry
            .session_by_id("project-1", "child-session")
            .expect("lookup moved child")
            .expect("child exists")
            .root_session_id,
        "new-root"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn agent_session_registry_process_register_helper() {
    if env::var("ASP_TURSO_SESSION_PROCESS_STRESS_CHILD")
        .ok()
        .as_deref()
        != Some("1")
    {
        return;
    }
    let state_root = PathBuf::from(
        env::var("ASP_TURSO_SESSION_PROCESS_STRESS_STATE_ROOT")
            .expect("ASP_TURSO_SESSION_PROCESS_STRESS_STATE_ROOT"),
    );
    let writer_id: usize = env::var("ASP_TURSO_SESSION_PROCESS_STRESS_WRITER_ID")
        .expect("ASP_TURSO_SESSION_PROCESS_STRESS_WRITER_ID")
        .parse()
        .expect("parse ASP_TURSO_SESSION_PROCESS_STRESS_WRITER_ID");
    let operation_started = std::time::Instant::now();
    let registry =
        AgentSessionRegistry::open_or_create_state_root(&state_root).expect("open registry");
    let shared_route = env::var("ASP_TURSO_SESSION_PROCESS_STRESS_SHARED_ROUTE")
        .ok()
        .as_deref()
        == Some("1");
    let root_session_id = if shared_route {
        "root-session".to_string()
    } else {
        format!("root-session-{writer_id}")
    };
    registry
        .register_session(AgentSessionRegisterRequest {
            project_id: "project-process-stress",
            root_session_id: &root_session_id,
            session_id: &format!("child-session-{writer_id}"),
            parent_session_id: Some("main-session"),
            name: "asp-explore",
            role: "asp-explore",
            model: Some("gpt-test"),
            status: "active",
            expires_at: None,
            metadata_json: "{\"route\":\"process-stress\"}",
            now: 1_800_001_000 + writer_id as i64,
        })
        .expect("register process stress session");
    assert!(
        operation_started.elapsed() <= std::time::Duration::from_millis(500),
        "process registry DB operation exceeded subsecond target: writer={writer_id} elapsed={:?}",
        operation_started.elapsed()
    );
}

#[test]
fn agent_session_registry_survives_concurrent_process_register_stress() {
    let root = temp_root("agent-session-registry-process-stress");
    let state_root = root.join("agent");
    let writer_count = 6usize;
    let current_exe = env::current_exe().expect("locate current test binary");
    let mut children = Vec::new();

    for writer_id in 0..writer_count {
        children.push(
            Command::new(&current_exe)
                .arg("--exact")
                .arg("db::agent_session_registry_process_register_helper")
                .arg("--nocapture")
                .env("ASP_TURSO_SESSION_PROCESS_STRESS_CHILD", "1")
                .env("ASP_TURSO_SESSION_PROCESS_STRESS_STATE_ROOT", &state_root)
                .env(
                    "ASP_TURSO_SESSION_PROCESS_STRESS_WRITER_ID",
                    writer_id.to_string(),
                )
                .spawn()
                .expect("spawn process registry writer"),
        );
    }

    for mut child in children {
        let status = child.wait().expect("wait for process registry writer");
        assert!(status.success(), "process registry writer failed: {status}");
    }

    let registry =
        AgentSessionRegistry::open_or_create_state_root(&state_root).expect("open registry");
    let sessions = registry
        .query_sessions("project-process-stress", None, Some("asp-explore"))
        .expect("query process stress sessions");
    assert_eq!(sessions.len(), writer_count);
    for writer_id in 0..writer_count {
        assert!(
            sessions.iter().any(|session| session.session_id
                == format!("child-session-{writer_id}")
                && session.root_session_id == format!("root-session-{writer_id}")),
            "missing process writer {writer_id} session in {sessions:?}"
        );
    }

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn agent_session_registry_concurrent_process_register_shared_route_does_not_unique_fail() {
    let root = temp_root("agent-session-registry-process-shared-route-stress");
    let state_root = root.join("agent");
    let writer_count = 6usize;
    let current_exe = env::current_exe().expect("locate current test binary");
    let mut children = Vec::new();

    for writer_id in 0..writer_count {
        children.push(
            Command::new(&current_exe)
                .arg("--exact")
                .arg("db::agent_session_registry_process_register_helper")
                .arg("--nocapture")
                .env("ASP_TURSO_SESSION_PROCESS_STRESS_CHILD", "1")
                .env("ASP_TURSO_SESSION_PROCESS_STRESS_SHARED_ROUTE", "1")
                .env("ASP_TURSO_SESSION_PROCESS_STRESS_STATE_ROOT", &state_root)
                .env(
                    "ASP_TURSO_SESSION_PROCESS_STRESS_WRITER_ID",
                    writer_id.to_string(),
                )
                .spawn()
                .expect("spawn process registry shared-route writer"),
        );
    }

    for mut child in children {
        let status = child.wait().expect("wait for shared-route registry writer");
        assert!(
            status.success(),
            "process registry shared-route writer failed: {status}"
        );
    }

    let registry =
        AgentSessionRegistry::open_or_create_state_root(&state_root).expect("open registry");
    let sessions = registry
        .query_sessions(
            "project-process-stress",
            Some("root-session"),
            Some("asp-explore"),
        )
        .expect("query shared route process stress session");
    assert_eq!(
        sessions.len(),
        1,
        "shared route register should converge to one routable row"
    );
    assert!(
        sessions[0].session_id.starts_with("child-session-"),
        "unexpected shared route winner: {:?}",
        sessions[0]
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn source_index_import_assembly_uses_turso_ready_contract_rows() {
    let root = temp_root("source-index-import");
    let src = root.join("src");
    std::fs::create_dir_all(&src).expect("create src dir");
    let lib = src.join("lib.rs");
    std::fs::write(&lib, "pub fn turso_source_index_fixture() {}\n").expect("write source");
    let scope_file = ClientDbSourceIndexScopeFile {
        path: lib.clone(),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        selector_receipts: Vec::new(),
    };

    let import = agent_semantic_client_db::assemble_source_index_import(
        ClientDbSourceIndexImportAssemblyRequest {
            generation_id: CacheGenerationId::from("source-index-generation"),
            project_root: root.clone(),
            schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
            schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
            selector_source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
            file_text_bytes_limit: 4096,
            previous_file_hashes: None,
            registry_fingerprint: "registry:v1".to_string(),
            extra_scope_dirs: Vec::new(),
            files: vec![scope_file.clone()],
        },
    )
    .expect("assemble source-index import");

    assert_eq!(source_index_relative_path(&root, &lib), "src/lib.rs");
    assert_eq!(
        source_index_scope_dirs(&root, &[scope_file]),
        [".", "src"].into_iter().map(str::to_string).collect()
    );
    assert_eq!(client_db_source_index_file_count(usize::MAX), u32::MAX);
    assert_eq!(import.owners.len(), 1);
    assert_eq!(import.selectors.len(), 1);
    assert!(
        import
            .file_hashes
            .iter()
            .any(|hash| hash.path == "src/lib.rs")
    );
    assert!(
        import
            .owners
            .first()
            .expect("owner")
            .query_keys
            .iter()
            .any(|key| key.as_str() == "turso_source_index_fixture")
    );

    let graph = source_index_evidence_graph(&import);
    assert!(graph.nodes.iter().any(|node| node.kind == "source-owner"));
    assert!(
        graph
            .nodes
            .iter()
            .any(|node| { node.selector.as_deref() == Some("rust://src/lib.rs#file") })
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn source_index_refresh_request_remains_db_engine_owned() {
    let root = temp_root("source-index-refresh");
    let import = build_source_index_import(ClientDbSourceIndexImportRequest {
        generation_id: CacheGenerationId::from("source-index-refresh-generation"),
        project_root: root.clone(),
        schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
        selector_source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
        file_hashes: vec![ClientCacheFileHash {
            path: "src/lib.rs".to_string(),
            sha256: "0123456789abcdef".repeat(4),
            byte_len: 37,
            mtime_ms: 42,
        }],
        files: vec![ClientDbSourceIndexImportFile {
            relative_path: "src/lib.rs".to_string(),
            language_id: LanguageId::from("rust"),
            provider_id: ProviderId::from("rs-harness"),
            text: "pub fn source_index_refresh_fixture() {}\n".to_string(),
            selectors: Vec::new(),
        }],
    })
    .expect("build source-index import");

    let request = ClientDbSourceIndexRefreshRequest {
        file_count: 1,
        import,
    };
    assert_eq!(request.file_count, 1);
    assert_eq!(
        request.import.schema_id.as_str(),
        CLIENT_DB_SOURCE_INDEX_SCHEMA_ID
    );

    let _ = std::fs::remove_dir_all(root);
}

fn temp_root(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::temp_dir().join(format!("asp-client-db-{name}-{nanos}"))
}
