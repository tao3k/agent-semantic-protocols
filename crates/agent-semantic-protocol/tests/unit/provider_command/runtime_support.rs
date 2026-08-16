use std::{path::Path, process::Command};

pub(super) fn runtime_client_command(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_asp"));
    command.env_clear();
    for variable in ["PATH", "TMPDIR", "CARGO_HOME", "RUSTUP_HOME"] {
        if let Some(value) = std::env::var_os(variable) {
            command.env(variable, value);
        }
    }
    command
        .env("HOME", root.join("home"))
        .env("ASP_STATE_HOME", super::support::state_home(root))
        .current_dir(root);
    command
}

pub(super) fn publish_runtime_server_artifact(root: &Path) {
    let state_home = super::support::state_home(root);
    let artifact_is_published =
        agent_semantic_protocol::published_runtime_server_artifact_digest(&state_home).is_some();
    if artifact_is_published
        && state_home
            .join("runtime/provider-catalog.v1.json")
            .is_file()
    {
        return;
    }
    let _prepared_command = super::support::asp_command(root);
    agent_semantic_protocol::prepare_runtime_server_provider_catalog(&state_home)
        .expect("publish Runtime Server provider catalog");
    if artifact_is_published {
        return;
    }
    agent_semantic_protocol::publish_runtime_server_artifact(
        Path::new(env!("CARGO_BIN_EXE_asp")),
        &state_home,
    )
    .expect("publish digest-addressed Runtime Server test artifact");
}

pub(super) fn run_resident_search(
    root: &Path,
    language_id: &str,
    args: &[&str],
) -> agent_semantic_client_db::runtime_search_service::RuntimeProviderSearchReceipt {
    let state_home = super::support::state_home(root);
    let endpoint = agent_semantic_client_db::read_runtime_server_endpoint(&state_home)
        .expect("read Runtime Server endpoint")
        .expect("Runtime Server endpoint is available");
    let workspace_identity = agent_semantic_client_core::state_core::ResolvedState::resolve(root)
        .expect("resolve Runtime provider search workspace")
        .workspace
        .workspace_id
        .to_string();
    let session = agent_semantic_client_db::WorkspaceDbIpcSession::for_runtime_server_client(
        &endpoint,
        workspace_identity,
        std::fs::canonicalize(root).expect("canonicalize Runtime provider search workspace"),
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build Runtime provider search test runtime");
    runtime
        .block_on(
            session.provider_search(
                format!("resident-search-{language_id}"),
                agent_semantic_client_core::LanguageId::try_from(language_id)
                    .expect("registered provider search language"),
                args.iter().map(|arg| (*arg).to_owned()).collect(),
            ),
        )
        .expect("run Runtime resident search")
}

pub(super) fn admit_runtime_resident_generation(root: &Path) {
    let state_home = super::support::state_home(root);
    let endpoint = agent_semantic_client_db::read_runtime_server_endpoint(&state_home)
        .expect("read Runtime Server endpoint")
        .expect("Runtime Server endpoint is available");
    let workspace_identity = agent_semantic_client_core::state_core::ResolvedState::resolve(root)
        .expect("resolve Runtime provider search workspace")
        .workspace
        .workspace_id
        .to_string();
    let session = agent_semantic_client_db::WorkspaceDbIpcSession::for_runtime_server_client(
        &endpoint,
        workspace_identity,
        std::fs::canonicalize(root).expect("canonicalize Runtime provider search workspace"),
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build Runtime provider search admission runtime");
    runtime.block_on(async {
        agent_semantic_client_db::runtime_server_control::ensure_runtime_server_workspace(
            &endpoint,
            root,
            "provider-search-fixture-admission".to_owned(),
        )
        .await
        .expect("ensure Runtime provider search workspace");
        session
            .admit_runtime_generation("resident-fixture-complete-generation", Vec::new())
            .await
            .expect("admit complete Runtime resident generation");
    });
}

pub(super) fn run_runtime_server_start(root: &Path) -> std::io::Result<std::process::Output> {
    publish_runtime_server_artifact(root);
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    let stopped = Arc::new(AtomicBool::new(false));
    let monitor_stopped = Arc::clone(&stopped);
    let monitor_started = std::time::Instant::now();
    let receipt_path = super::support::state_home(root)
        .join("runtime")
        .join("server")
        .join("daemon-startup.v1.json");
    let monitor = std::thread::spawn(move || {
        let mut last_stage: Option<String> = None;
        while !monitor_stopped.load(Ordering::Acquire) {
            if let Ok(bytes) = std::fs::read(&receipt_path)
                && let Ok(receipt) = serde_json::from_slice::<serde_json::Value>(&bytes)
            {
                let stage = receipt.get("stage").and_then(serde_json::Value::as_str);
                if stage != last_stage.as_deref() {
                    eprintln!(
                        "[runtime-server-startup-phase] stage={} state={} elapsedMicros={} wallElapsedMs={:.3}",
                        stage.unwrap_or("missing"),
                        receipt
                            .get("state")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("missing"),
                        receipt
                            .get("elapsedMicros")
                            .and_then(serde_json::Value::as_u64)
                            .unwrap_or_default(),
                        monitor_started.elapsed().as_secs_f64() * 1_000.0
                    );
                    last_stage = stage.map(str::to_owned);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    });

    let output = runtime_client_command(root)
        .args(["server", "start"])
        .output();
    stopped.store(true, Ordering::Release);
    let _ = monitor.join();
    output
}
