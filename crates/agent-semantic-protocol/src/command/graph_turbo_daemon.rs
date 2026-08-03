use std::collections::BTreeMap;
use std::ffi::OsString;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use agent_semantic_client_db::runtime_server::{
    GraphTurboEvaluationBuilder, GraphTurboResidentStatusHandle,
};
use agent_semantic_client_db::runtime_server_control::{
    GraphTurboResidentState, GraphTurboResidentStatus,
};
use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot, watch};

use crate::{
    GraphTurboResidentLaunchSpec, GraphTurboResidentProcess, admit_candidate_rank_receipt,
};

const PYTHON_ENV: &str = "ASP_GRAPH_TURBO_PYTHON";
const RESIDENT_MODULE: &str = "asp_graph_turbo.resident_server";
const IDENTITY_FIELDS: [&str; 5] = [
    "sessionId",
    "nodeId",
    "snapshotDigest",
    "workspaceGenerationRootDigest",
    "routeId",
];

pub struct GraphTurboDaemon {
    status: GraphTurboResidentStatusHandle,
    commands: Option<mpsc::Sender<GraphTurboActorCommand>>,
    shutdown: Option<watch::Sender<bool>>,
    actor: Option<tokio::task::JoinHandle<Result<(), String>>>,
}

enum GraphTurboActorCommand {
    Evaluate {
        request: Value,
        response: oneshot::Sender<Result<Value, String>>,
    },
}

type GraphTurboStartup = Pin<
    Box<
        dyn Future<Output = Result<(GraphTurboResidentProcess, PathBuf, String), String>>
            + Send,
    >,
>;

impl GraphTurboDaemon {
    pub async fn start_from_environment(state_home: &Path) -> Self {
        let status = GraphTurboResidentStatusHandle::new(resident_status(
            GraphTurboResidentState::Unavailable,
            None,
            "graph-turbo-runtime-artifact-not-configured",
        ));
        let Some(configured) = std::env::var_os(PYTHON_ENV) else {
            return Self {
                status,
                commands: None,
                shutdown: None,
                actor: None,
            };
        };
        let configured_path = PathBuf::from(&configured);
        status.update(resident_status(
            GraphTurboResidentState::Starting,
            Some(configured_path.display().to_string()),
            "",
        ));
        let state_home = state_home.to_path_buf();
        let startup = Box::pin(async move { start_resident(configured, &state_home).await });
        Self::start_configured(status, configured_path, startup)
    }

    fn start_configured(
        status: GraphTurboResidentStatusHandle,
        configured_path: PathBuf,
        startup: GraphTurboStartup,
    ) -> Self {
        let (commands, command_receiver) = mpsc::channel(graph_turbo_actor_capacity());
        let (shutdown, shutdown_receiver) = watch::channel(false);
        let actor_status = status.clone();
        let actor = tokio::spawn(run_graph_turbo_actor(
            configured_path,
            startup,
            actor_status,
            command_receiver,
            shutdown_receiver,
        ));
        Self {
            status,
            commands: Some(commands),
            shutdown: Some(shutdown),
            actor: Some(actor),
        }
    }

    pub fn status(&self) -> GraphTurboResidentStatusHandle {
        self.status.clone()
    }

    pub fn evaluation_builder(&self) -> Option<GraphTurboEvaluationBuilder> {
        let commands = self.commands.as_ref()?.clone();
        let status = self.status.clone();
        Some(Arc::new(
            move |_workspace_identity, _project_root, message| {
                let commands = commands.clone();
                let status = status.clone();
                Box::pin(async move {
                    validate_rank_request(&message)?;
                    let snapshot = status.snapshot();
                    if snapshot.state != GraphTurboResidentState::Healthy {
                        return Err(format!(
                            "Graph Turbo resident is not ready: state={} reason={}",
                            resident_state_label(snapshot.state),
                            snapshot.reason.as_deref().unwrap_or("none")
                        ));
                    }
                    let (response, receipt) = oneshot::channel();
                    commands
                        .try_send(GraphTurboActorCommand::Evaluate {
                            request: message.clone(),
                            response,
                        })
                        .map_err(|error| {
                            format!("Graph Turbo resident actor is unavailable: {error}")
                        })?;
                    receipt.await.map_err(|_| {
                        "Graph Turbo resident actor dropped the evaluation receipt".to_owned()
                    })?
                })
            },
        ))
    }

    pub async fn shutdown(&mut self) -> Result<(), String> {
        self.status.mutate(|current| {
            current.state = GraphTurboResidentState::Draining;
            current.reason = None;
        });
        self.commands.take();
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(true);
        }
        if let Some(actor) = self.actor.take() {
            actor
                .await
                .map_err(|error| format!("Graph Turbo resident actor failed: {error}"))??;
        }
        self.status.update(resident_status(
            GraphTurboResidentState::Unavailable,
            None,
            "graph-turbo-resident-shutdown",
        ));
        Ok(())
    }
}

fn graph_turbo_actor_capacity() -> usize {
    tokio::runtime::Handle::current()
        .metrics()
        .num_workers()
        .saturating_mul(2)
        .max(1)
}

async fn run_graph_turbo_actor(
    configured_path: PathBuf,
    startup: GraphTurboStartup,
    status: GraphTurboResidentStatusHandle,
    mut commands: mpsc::Receiver<GraphTurboActorCommand>,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), String> {
    let (mut process, artifact, command_digest) = match startup.await {
        Ok(resident) => resident,
        Err(error) => {
            let state = if *shutdown.borrow() {
                GraphTurboResidentState::Unavailable
            } else {
                GraphTurboResidentState::Failed
            };
            status.update(resident_status(
                state,
                Some(configured_path.display().to_string()),
                &error,
            ));
            return Ok(());
        }
    };
    if *shutdown.borrow() {
        tokio::task::spawn_blocking(move || shutdown_resident_process(&mut process))
            .await
            .map_err(|error| format!("Graph Turbo resident shutdown task failed: {error}"))??;
        return Ok(());
    }
    status.update(GraphTurboResidentStatus {
        state: GraphTurboResidentState::Healthy,
        process_id: Some(process.process_id()),
        runtime_artifact: Some(artifact.display().to_string()),
        execution_command_digest: Some(command_digest),
        reason: None,
    });
    loop {
        tokio::select! {
            biased;
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
            command = commands.recv() => {
                let Some(GraphTurboActorCommand::Evaluate { request, response }) = command else {
                    break;
                };
                let (returned_process, result) = tokio::task::spawn_blocking(move || {
                    let result = process
                        .request(&request)
                        .and_then(|receipt| admit_exact_candidate(&request, receipt));
                    (process, result)
                })
                .await
                .map_err(|error| format!("Graph Turbo resident task failed: {error}"))?;
                process = returned_process;
                if let Err(error) = &result {
                    status.mutate(|current| {
                        current.state = GraphTurboResidentState::Failed;
                        current.reason = Some(error.clone());
                    });
                }
                let _ = response.send(result);
            }
        }
    }
    status.mutate(|current| {
        current.state = GraphTurboResidentState::Draining;
        current.reason = None;
    });
    tokio::task::spawn_blocking(move || shutdown_resident_process(&mut process))
        .await
        .map_err(|error| format!("Graph Turbo resident shutdown task failed: {error}"))??;
    Ok(())
}

async fn start_resident(
    configured: OsString,
    state_home: &Path,
) -> Result<(GraphTurboResidentProcess, PathBuf, String), String> {
    let configured = PathBuf::from(configured);
    if !configured.is_absolute() {
        return Err(format!("{PYTHON_ENV} must be an absolute path"));
    }
    let artifact = tokio::fs::canonicalize(&configured)
        .await
        .map_err(|error| {
            format!(
                "failed to resolve Graph Turbo Python artifact `{}`: {error}",
                configured.display()
            )
        })?;
    let artifact_digest = format!(
        "blake3-256:{}",
        super::super::protocol_binary::canonical_protocol_binary_artifact_digest(&artifact).await?
    );
    let command_digest = command_digest(&configured);
    let state_home = state_home.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let mut process = GraphTurboResidentProcess::spawn(GraphTurboResidentLaunchSpec {
            program: configured.clone(),
            args: vec![OsString::from("-m"), OsString::from(RESIDENT_MODULE)],
            cwd: state_home,
            env: BTreeMap::new(),
            execution_command_digest: command_digest.clone(),
            request_timeout: Duration::from_secs(30),
        })?;
        let receipt = process.request(&json!({
            "schemaId": "agent.semantic-protocols.semantic-graph-turbo-resident-message",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.semantic-language",
            "protocolVersion": "1",
            "messageKind": "process-handshake",
            "requestId": "graph-turbo-daemon-handshake",
            "runtimeArtifactDigest": artifact_digest,
            "executionCommandDigest": command_digest
        }))?;
        validate_handshake(
            &receipt,
            process.process_id(),
            &artifact_digest,
            &command_digest,
        )?;
        Ok((process, configured, command_digest))
    })
    .await
    .map_err(|error| format!("Graph Turbo resident startup task failed: {error}"))?
}

fn shutdown_resident_process(process: &mut GraphTurboResidentProcess) -> Result<(), String> {
    let receipt = process.shutdown(&json!({
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-resident-message",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "messageKind": "shutdown",
        "requestId": "graph-turbo-daemon-shutdown"
    }))?;
    if receipt.get("schemaVersion").and_then(Value::as_str) != Some("1") {
        return Err("Graph Turbo resident shutdown receipt version mismatch".to_owned());
    }
    Ok(())
}

fn resident_state_label(state: GraphTurboResidentState) -> &'static str {
    match state {
        GraphTurboResidentState::Unavailable => "unavailable",
        GraphTurboResidentState::Starting => "starting",
        GraphTurboResidentState::Healthy => "healthy",
        GraphTurboResidentState::Draining => "draining",
        GraphTurboResidentState::Failed => "failed",
    }
}

fn command_digest(artifact: &Path) -> String {
    let command = format!("{}\0-m\0{RESIDENT_MODULE}", artifact.display());
    format!("blake3-256:{}", blake3::hash(command.as_bytes()).to_hex())
}

fn validate_handshake(
    receipt: &Value,
    process_id: u32,
    artifact_digest: &str,
    command_digest: &str,
) -> Result<(), String> {
    let identity = receipt.get("processIdentity");
    let exact = receipt.get("schemaVersion").and_then(Value::as_str) == Some("1")
        && receipt.get("status").and_then(Value::as_str) == Some("process-handshake-accepted")
        && identity
            .and_then(|value| value.get("processId"))
            .and_then(Value::as_u64)
            == Some(u64::from(process_id))
        && identity
            .and_then(|value| value.get("runtimeArtifactDigest"))
            .and_then(Value::as_str)
            == Some(artifact_digest)
        && identity
            .and_then(|value| value.get("executionCommandDigest"))
            .and_then(Value::as_str)
            == Some(command_digest);
    exact
        .then_some(())
        .ok_or_else(|| "Graph Turbo resident process handshake identity mismatch".to_owned())
}

fn validate_rank_request(message: &Value) -> Result<(), String> {
    if message.get("schemaVersion").and_then(Value::as_str) != Some("1")
        || message.get("messageKind").and_then(Value::as_str) != Some("rank")
    {
        return Err("Graph Turbo daemon accepts only resident rank message v1".to_owned());
    }
    for field in IDENTITY_FIELDS {
        if message
            .get(field)
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        {
            return Err(format!("Graph Turbo rank message requires {field}"));
        }
    }
    Ok(())
}

fn admit_exact_candidate(request: &Value, receipt: Value) -> Result<Value, String> {
    if receipt.get("schemaVersion").and_then(Value::as_str) != Some("1") {
        return Err("Graph Turbo resident rank receipt version mismatch".to_owned());
    }
    let identity = receipt
        .get("graphSessionIdentity")
        .ok_or_else(|| "Graph Turbo rank receipt omitted graphSessionIdentity".to_owned())?;
    for field in IDENTITY_FIELDS {
        if identity.get(field) != request.get(field) {
            return Err(format!(
                "Graph Turbo rank receipt substituted graph-session field {field}"
            ));
        }
    }
    admit_candidate_rank_receipt(receipt)
}

fn resident_status(
    state: GraphTurboResidentState,
    artifact: Option<String>,
    reason: &str,
) -> GraphTurboResidentStatus {
    GraphTurboResidentStatus {
        state,
        process_id: None,
        runtime_artifact: artifact,
        execution_command_digest: None,
        reason: (!reason.is_empty()).then(|| reason.to_owned()),
    }
}

#[cfg(test)]
#[path = "../../tests/unit/command/graph_turbo_daemon.rs"]
mod graph_turbo_daemon_tests;
