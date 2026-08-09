//! Runtime-supervisor-owned lifecycle for the optional Graph Turbo resident.

use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use agent_semantic_client_db::runtime_server::{
    GraphTurboEvaluationBuilder, GraphTurboResidentStatusHandle,
};
use agent_semantic_client_db::runtime_server_control::{
    GraphTurboResidentState, GraphTurboResidentStatus,
};
use agent_semantic_search::{
    GraphTurboResidentReceipt, GraphTurboResidentRequest, GraphTurboServerState,
};
use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot, watch};

use crate::{GraphTurboResidentLaunchSpec, GraphTurboResidentProcess};

/// Optional Graph Turbo actor whose process starts only after typed demand.
pub struct GraphTurboDaemon {
    status: GraphTurboResidentStatusHandle,
    commands: Option<mpsc::Sender<GraphTurboActorCommand>>,
    shutdown: Option<watch::Sender<bool>>,
    actor: Option<
        agent_semantic_client_db::runtime_server_runtime::RuntimeServerOwnedTask<
            Result<(), String>,
        >,
    >,
}

enum GraphTurboActorCommand {
    Bootstrap,
    Evaluate {
        request: Value,
        response: oneshot::Sender<Result<Value, String>>,
    },
}

type GraphTurboStartup = Pin<
    Box<dyn Future<Output = Result<(GraphTurboResidentProcess, PathBuf, String), String>> + Send>,
>;
type GraphTurboStartupFactory = Arc<dyn Fn() -> GraphTurboStartup + Send + Sync>;

impl GraphTurboDaemon {
    pub async fn start_from_managed_config(state_home: &Path) -> Self {
        let status = GraphTurboResidentStatusHandle::new(resident_status(
            GraphTurboResidentState::Unavailable,
            None,
            "graph-turbo-demand-admission-pending",
        ));
        let state_home = state_home.to_path_buf();
        let startup: GraphTurboStartupFactory = Arc::new(move || -> GraphTurboStartup {
            let state_home = state_home.clone();
            Box::pin(async move {
                let configured = crate::server::runtime_server_supervisor::configured_graph_turbo_artifact_at_state_home(
                    &state_home,
                )
                .await?
                .ok_or_else(|| "Graph Turbo managed Runtime artifact is not configured".to_owned())?;
                start_resident(configured, &state_home).await
            })
        });
        Self::start_configured(status, None, startup)
    }

    fn start_configured(
        status: GraphTurboResidentStatusHandle,
        configured_path: Option<PathBuf>,
        startup: GraphTurboStartupFactory,
    ) -> Self {
        let (commands, command_receiver) = mpsc::channel(graph_turbo_actor_capacity());
        let (shutdown, shutdown_receiver) = watch::channel(false);
        let actor_status = status.clone();
        status.update(resident_status(
            GraphTurboResidentState::Starting,
            configured_path
                .as_ref()
                .map(|path| path.display().to_string()),
            "graph-turbo-supervisor-bootstrap",
        ));
        let actor = agent_semantic_client_db::runtime_server_runtime::RuntimeServerOwnedTask::spawn(
            "graph-turbo-resident-actor",
            run_graph_turbo_actor(
                configured_path,
                startup,
                actor_status,
                command_receiver,
                shutdown_receiver,
            ),
        );
        if let Err(error) = commands.try_send(GraphTurboActorCommand::Bootstrap) {
            status.update(resident_status(
                GraphTurboResidentState::Failed,
                None,
                &format!("Graph Turbo supervisor bootstrap failed: {error}"),
            ));
        }
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
                    // Rank work is bounded by the Runtime Server connection
                    // lease and caller deadline.  The actor itself must apply
                    // Tokio backpressure instead of rejecting ordinary
                    // concurrent workspace requests because its short local
                    // queue happened to fill.
                    commands
                        .send(GraphTurboActorCommand::Evaluate {
                            request: message.clone(),
                            response,
                        })
                        .await
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
            actor.join().await??;
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

fn resident_generation_key(message: &Value) -> Result<String, String> {
    let workspace = message
        .get("workspaceIdentity")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Graph Turbo rank intent omitted workspaceIdentity".to_owned())?;
    let generation = message
        .get("generationDigest")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Graph Turbo rank intent omitted generationDigest".to_owned())?;
    let page_roots = message
        .get("pageRoots")
        .ok_or_else(|| "Graph Turbo rank intent omitted pageRoots".to_owned())?;
    let page_roots = serde_json::to_string(page_roots)
        .map_err(|error| format!("encode Graph Turbo page roots: {error}"))?;
    Ok(format!("{workspace}\u{1f}{generation}\u{1f}{page_roots}"))
}

fn resident_generation_load_message(message: &Value) -> Result<Value, String> {
    validate_rank_request(message)?;
    let graph = message
        .get("rankPayload")
        .and_then(Value::as_object)
        .and_then(|payload| payload.get("graph"))
        .cloned()
        .filter(Value::is_object)
        .ok_or_else(|| "Graph Turbo rank intent omitted canonical graph page".to_owned())?;
    let mut load = message.clone();
    let object = load
        .as_object_mut()
        .ok_or_else(|| "Graph Turbo rank intent must be an object".to_owned())?;
    object.insert(
        "messageKind".to_owned(),
        Value::String("load-generation".to_owned()),
    );
    object.insert("generationPayload".to_owned(), json!({"graph": graph}));
    object.remove("rankPayload");
    Ok(load)
}

fn resident_rank_message(message: &Value) -> Result<Value, String> {
    validate_rank_request(message)?;
    let mut rank = message.clone();
    let payload = rank
        .get_mut("rankPayload")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "Graph Turbo rank intent omitted rankPayload".to_owned())?;
    payload.remove("graph");
    Ok(rank)
}

fn admit_generation_load(request: &Value, receipt: Value) -> Result<(), String> {
    let request: GraphTurboResidentRequest = serde_json::from_value(request.clone())
        .map_err(|error| format!("decode Graph Turbo load-generation request: {error}"))?;
    request.validate()?;
    let validated: GraphTurboResidentReceipt = serde_json::from_value(receipt)
        .map_err(|error| format!("decode Graph Turbo load-generation receipt: {error}"))?;
    validated.validate_for(&request)?;
    if validated.state != GraphTurboServerState::Ready {
        return Err("Graph Turbo resident generation load did not become ready".to_owned());
    }
    Ok(())
}

async fn run_graph_turbo_actor(
    configured_path: Option<PathBuf>,
    startup: GraphTurboStartupFactory,
    status: GraphTurboResidentStatusHandle,
    mut commands: mpsc::Receiver<GraphTurboActorCommand>,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), String> {
    let mut process = None;
    let mut loaded_generations = std::collections::BTreeSet::new();
    loop {
        tokio::select! {
            biased;
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
            command = commands.recv() => {
                let Some(command) = command else {
                    break;
                };
            if matches!(&command, GraphTurboActorCommand::Bootstrap) {
                    if let Some(existing) = process.as_mut() {
                        let _ = shutdown_resident_process(existing).await;
                    }
                    process = None;
                    loaded_generations.clear();
                    status.update(resident_status(
                        GraphTurboResidentState::Starting,
                        configured_path
                            .as_ref()
                            .map(|path| path.display().to_string()),
                    "graph-turbo-supervisor-starting",
                    ));
                    let started = tokio::select! {
                        biased;
                        changed = shutdown.changed() => {
                            let _ = changed;
                            return Ok(());
                        }
                        started = (startup)() => started,
                    };
                    match started {
                        Ok((resident, artifact, command_digest)) => {
                            status.update(GraphTurboResidentStatus {
                                state: GraphTurboResidentState::Healthy,
                                process_id: None,
                                runtime_artifact: Some(artifact.display().to_string()),
                                execution_command_digest: Some(command_digest),
                                reason: None,
                            });
                            process = Some(resident);
                        }
                        Err(error) => {
                            status.update(resident_status(
                                GraphTurboResidentState::Failed,
                                configured_path
                                    .as_ref()
                                    .map(|path| path.display().to_string()),
                                &error,
                            ));
                        }
                    }
                    continue;
                }
                let GraphTurboActorCommand::Evaluate { request, response } = command else {
                    continue;
                };
                let result = match process.as_mut() {
                    Some(process) => async {
                        let generation_key = resident_generation_key(&request)?;
                        if loaded_generations.insert(generation_key) {
                            let load = resident_generation_load_message(&request)?;
                            process
                                .request(&load)
                                .await
                                .and_then(|receipt| admit_generation_load(&load, receipt))?;
                        }
                        let rank = resident_rank_message(&request)?;
                        process
                            .request(&rank)
                            .await
                            .and_then(|receipt| admit_exact_candidate(&rank, receipt))
                    }
                    .await,
                    None => Err("Graph Turbo resident process is not admitted".to_owned()),
                };
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
    if let Some(process) = process.as_mut() {
        shutdown_resident_process(process).await?;
    }
    Ok(())
}

async fn start_resident(
    configured: crate::server::runtime_server_supervisor::ConfiguredGraphTurboArtifact,
    state_home: &Path,
) -> Result<(GraphTurboResidentProcess, PathBuf, String), String> {
    let command_digest = command_digest(&configured.locator);
    let state_home = state_home.to_path_buf();
    let mut process = GraphTurboResidentProcess::spawn(GraphTurboResidentLaunchSpec {
        program: configured.locator.clone(),
        args: Vec::new(),
        cwd: state_home,
        env: BTreeMap::new(),
        execution_command_digest: command_digest.clone(),
        request_timeout: super::AGENT_FACING_EXECUTION_BUDGET,
    })
    .await?;
    let receipt = process
        .request_with_timeout(
            &json!({
                "schemaId": "agent.semantic-protocols.graph-turbo-resident-server",
                "schemaVersion": "1",
                "messageKind": "hello",
                "requestId": 1,
                "runtimeArtifactDigest": configured.runtime_artifact_digest,
                "executionCommandDigest": command_digest
            }),
            super::OPERATOR_RUNTIME_SERVER_STARTUP_BUDGET,
        )
        .await?;
    validate_handshake(
        &receipt,
        &configured.runtime_artifact_digest,
        &command_digest,
    )?;
    Ok((process, configured.locator, command_digest))
}

async fn shutdown_resident_process(process: &mut GraphTurboResidentProcess) -> Result<(), String> {
    let receipt = process
        .shutdown(&json!({
            "schemaId": "agent.semantic-protocols.graph-turbo-resident-server",
            "schemaVersion": "1",
            "messageKind": "shutdown",
            "requestId": 2
        }))
        .await?;
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
    let command = format!("{}\0", artifact.display());
    format!("blake3-256:{}", blake3::hash(command.as_bytes()).to_hex())
}

fn validate_handshake(
    receipt: &Value,
    artifact_digest: &str,
    command_digest: &str,
) -> Result<(), String> {
    let identity = receipt.get("processIdentity");
    let exact = receipt.get("schemaId").and_then(Value::as_str)
        == Some("agent.semantic-protocols.graph-turbo-resident-server")
        && receipt.get("schemaVersion").and_then(Value::as_str) == Some("1")
        && receipt.get("messageKind").and_then(Value::as_str) == Some("receipt")
        && receipt.get("state").and_then(Value::as_str) == Some("ready")
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
    let request: GraphTurboResidentRequest = serde_json::from_value(message.clone())
        .map_err(|error| format!("decode Graph Turbo resident v1 rank request: {error}"))?;
    request.validate()
}

fn admit_exact_candidate(request: &Value, receipt: Value) -> Result<Value, String> {
    let request: GraphTurboResidentRequest = serde_json::from_value(request.clone())
        .map_err(|error| format!("decode Graph Turbo resident v1 rank request: {error}"))?;
    let validated: GraphTurboResidentReceipt = serde_json::from_value(receipt.clone())
        .map_err(|error| format!("decode Graph Turbo resident v1 rank receipt: {error}"))?;
    validated.validate_for(&request)?;
    if validated.state != GraphTurboServerState::Completed {
        return Err("Graph Turbo resident rank did not complete".to_owned());
    }
    receipt
        .get("result")
        .cloned()
        .ok_or_else(|| "Graph Turbo resident receipt omitted parser-owned result".to_owned())
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
