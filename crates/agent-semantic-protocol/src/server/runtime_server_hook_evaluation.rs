//! Immutable, in-memory Hook policy evaluation owned by the Runtime Server.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_semantic_client_db::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalogEntry;
use agent_semantic_hook::{
    HookClassificationRequest, HookDecision, HookRuntime, classify_hook_with_config, parse_payload,
    render_platform_response,
};

struct ResidentHookSnapshot {
    project_root: PathBuf,
    runtime: HookRuntime,
    config: Arc<agent_semantic_hook::ClientHookConfig>,
}

const DURABLE_HOOK_SNAPSHOT_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-hook-snapshot";
const DURABLE_HOOK_SNAPSHOT_SCHEMA_VERSION: &str = "1";
const DURABLE_HOOK_SNAPSHOT_MEMORY_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DurableResidentHookSnapshot {
    project_root: PathBuf,
    runtime: HookRuntime,
    config: agent_semantic_hook::DurableHookConfigArtifact,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DurableResidentHookSnapshotGeneration {
    schema_id: String,
    schema_version: String,
    workspace_identity_by_root: BTreeMap<PathBuf, String>,
    snapshots: BTreeMap<String, DurableResidentHookSnapshot>,
}

impl DurableResidentHookSnapshotGeneration {
    fn from_generation(generation: &ResidentHookSnapshotGeneration) -> Self {
        Self {
            schema_id: DURABLE_HOOK_SNAPSHOT_SCHEMA_ID.to_owned(),
            schema_version: DURABLE_HOOK_SNAPSHOT_SCHEMA_VERSION.to_owned(),
            workspace_identity_by_root: generation
                .snapshots
                .iter()
                .map(|(workspace_identity, snapshot)| {
                    (snapshot.project_root.clone(), workspace_identity.clone())
                })
                .collect(),
            snapshots: generation
                .snapshots
                .iter()
                .map(|(workspace_identity, snapshot)| {
                    (
                        workspace_identity.clone(),
                        DurableResidentHookSnapshot {
                            project_root: snapshot.project_root.clone(),
                            runtime: snapshot.runtime.clone(),
                            config: snapshot.config.durable_snapshot_config(),
                        },
                    )
                })
                .collect(),
        }
    }
}

type ResidentHookSnapshots = BTreeMap<String, Arc<ResidentHookSnapshot>>;
type ResidentHookSnapshotFailures = BTreeMap<String, String>;

#[derive(Clone, Default)]
struct ResidentHookSnapshotGeneration {
    admitted_roots: BTreeMap<String, PathBuf>,
    snapshots: ResidentHookSnapshots,
    failures: ResidentHookSnapshotFailures,
}

pub(super) struct ResidentHookSnapshotAuthority {
    generation: tokio::sync::watch::Sender<Arc<ResidentHookSnapshotGeneration>>,
    shutdown: tokio::sync::watch::Sender<bool>,
    task: Option<
        agent_semantic_client_db::runtime_server_runtime::RuntimeServerOwnedTask<
            Result<(), String>,
        >,
    >,
}

impl ResidentHookSnapshotAuthority {
    pub(super) async fn start(
        catalog: agent_semantic_client_db::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog,
        active_workspace_store_root: PathBuf,
        durable_snapshot_path: PathBuf,
    ) -> Result<Self, String> {
        let initial =
            materialize_generation(catalog.snapshot(), active_workspace_store_root.clone()).await?;
        if initial.snapshots.is_empty() {
            return Err(
                "Runtime Server has no active workspace with a materializable Hook snapshot"
                    .to_owned(),
            );
        }
        let mut durable_writer =
            agent_semantic_client_db::seqlock_json_memory::SeqlockJsonMemoryWriter::create(
                &durable_snapshot_path,
                DURABLE_HOOK_SNAPSHOT_MEMORY_BYTES,
            )
            .await?;
        durable_writer.publish(&DurableResidentHookSnapshotGeneration::from_generation(
            &initial,
        ))?;
        let (generation, _) = tokio::sync::watch::channel(Arc::new(initial));
        let (shutdown, mut shutdown_receiver) = tokio::sync::watch::channel(false);
        let generation_publisher = generation.clone();
        let mut admissions = catalog.subscribe();
        let task = agent_semantic_client_db::runtime_server_runtime::RuntimeServerOwnedTask::spawn(
            "resident-hook-snapshot-authority",
            async move {
                let mut current = Arc::clone(&generation_publisher.borrow());
                loop {
                    tokio::select! {
                    biased;
                    changed = shutdown_receiver.changed() => {
                        if changed.is_err() || *shutdown_receiver.borrow() {
                            return Ok(());
                        }
                    }
                    changed = admissions.changed() => {
                        if changed.is_err() {
                            return Err("Runtime Server Hook snapshot admission stream closed".to_owned());
                        }
                        let entries = Arc::clone(&admissions.borrow_and_update());
                        let next = materialize_generation_delta(
                            Arc::clone(&current),
                            entries,
                            active_workspace_store_root.clone(),
                        )
                        .await?;
                        current = Arc::new(next);
                        durable_writer.publish(
                            &DurableResidentHookSnapshotGeneration::from_generation(&current),
                        )?;
                        generation_publisher.send_replace(Arc::clone(&current));
                    }
                    }
                }
            },
        );
        Ok(Self {
            generation,
            shutdown,
            task: Some(task),
        })
    }

    pub(super) fn evaluator(
        &self,
    ) -> agent_semantic_client_db::runtime_server::HookEvaluationBuilder {
        let generation = self.generation.clone();
        Arc::new(move |workspace_identity, project_root, arguments, input| {
            let current = Arc::clone(&generation.borrow());
            let snapshot = current.snapshots.get(&workspace_identity).cloned();
            let failure = current.failures.get(&workspace_identity).cloned();
            Box::pin(async move {
                let snapshot = snapshot.ok_or_else(|| match failure {
                    Some(error) => format!(
                        "resident Hook snapshot admission failed: workspaceIdentity={workspace_identity} error={error}"
                    ),
                    None => format!(
                        "resident Hook snapshot is not admitted: workspaceIdentity={workspace_identity}"
                    ),
                })?;
                if snapshot.project_root != project_root {
                    return Err(format!(
                        "resident Hook snapshot root mismatch: workspaceIdentity={workspace_identity} admitted={} requested={}",
                        snapshot.project_root.display(),
                        project_root.display()
                    ));
                }
                evaluate_snapshot(&snapshot, &arguments, &input)
            })
        })
    }

    pub(super) async fn shutdown(mut self) -> Result<(), String> {
        self.shutdown.send_replace(true);
        match self.task.take() {
            Some(task) => task.join().await?,
            None => Ok(()),
        }
    }
}

impl Drop for ResidentHookSnapshotAuthority {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

async fn materialize_generation(
    admitted: Arc<BTreeSet<RuntimeWorkspaceAdmissionCatalogEntry>>,
    active_workspace_store_root: PathBuf,
) -> Result<ResidentHookSnapshotGeneration, String> {
    agent_semantic_client_db::runtime_server_runtime::RuntimeServerOwnedTask::spawn_blocking(
        "resident-hook-snapshot-initial-materialization",
        move || materialize_generation_blocking(admitted.as_ref(), &active_workspace_store_root),
    )
    .join()
    .await
}

async fn materialize_generation_delta(
    previous: Arc<ResidentHookSnapshotGeneration>,
    admitted: Arc<BTreeSet<RuntimeWorkspaceAdmissionCatalogEntry>>,
    active_workspace_store_root: PathBuf,
) -> Result<ResidentHookSnapshotGeneration, String> {
    agent_semantic_client_db::runtime_server_runtime::RuntimeServerOwnedTask::spawn_blocking(
        "resident-hook-snapshot-delta-materialization",
        move || {
            materialize_generation_delta_blocking(
                previous.as_ref(),
                admitted.as_ref(),
                &active_workspace_store_root,
            )
        },
    )
    .join()
    .await
}

fn materialize_generation_blocking(
    admitted: &BTreeSet<RuntimeWorkspaceAdmissionCatalogEntry>,
    active_workspace_store_root: &Path,
) -> ResidentHookSnapshotGeneration {
    materialize_generation_delta_blocking(
        &ResidentHookSnapshotGeneration::default(),
        admitted,
        active_workspace_store_root,
    )
}

fn materialize_generation_delta_blocking(
    previous: &ResidentHookSnapshotGeneration,
    admitted: &BTreeSet<RuntimeWorkspaceAdmissionCatalogEntry>,
    active_workspace_store_root: &Path,
) -> ResidentHookSnapshotGeneration {
    let mut admitted_roots = BTreeMap::new();
    let mut snapshots = ResidentHookSnapshots::new();
    let mut failures = ResidentHookSnapshotFailures::new();
    for entry in admitted {
        admitted_roots.insert(entry.workspace_identity.clone(), entry.project_root.clone());
        if previous
            .admitted_roots
            .get(&entry.workspace_identity)
            .is_some_and(|root| root == &entry.project_root)
        {
            if let Some(snapshot) = previous.snapshots.get(&entry.workspace_identity) {
                snapshots.insert(entry.workspace_identity.clone(), Arc::clone(snapshot));
            }
            if let Some(error) = previous.failures.get(&entry.workspace_identity) {
                failures.insert(entry.workspace_identity.clone(), error.clone());
            }
            continue;
        }
        if !active_workspace_store_root
            .join(&entry.workspace_identity)
            .is_dir()
            || !entry.project_root.is_dir()
            || !agent_semantic_hook::default_activation_path(&entry.project_root).is_file()
        {
            continue;
        }
        match materialize_snapshot(entry) {
            Ok(snapshot) => {
                snapshots.insert(entry.workspace_identity.clone(), Arc::new(snapshot));
            }
            Err(error) => {
                failures.insert(entry.workspace_identity.clone(), error);
            }
        }
    }
    ResidentHookSnapshotGeneration {
        admitted_roots,
        snapshots,
        failures,
    }
}

fn materialize_snapshot(
    entry: &RuntimeWorkspaceAdmissionCatalogEntry,
) -> Result<ResidentHookSnapshot, String> {
    entry.validate()?;
    let activation_path = agent_semantic_hook::default_activation_path(&entry.project_root);
    let mut runtime = agent_semantic_hook::load_activation(&activation_path).map_err(|error| {
        format!(
            "failed to materialize resident Hook activation {}: {error}",
            activation_path.display()
        )
    })?;
    runtime.project_root = entry.project_root.display().to_string();
    let config_path =
        agent_semantic_hook::default_client_config_path(&entry.project_root.to_string_lossy());
    let (config, _) = crate::command::hook_runtime::load_resident_hook_config(
        &config_path,
        &entry.project_root,
        &runtime,
    )?;
    crate::command::hook_runtime::apply_resident_language_provider_projection(
        &config.config,
        &mut runtime,
        &config_path,
        config.auto_refresh.as_deref().unwrap_or("not-required"),
    )?;
    Ok(ResidentHookSnapshot {
        project_root: entry.project_root.clone(),
        runtime,
        config: Arc::clone(&config.config),
    })
}

fn evaluate_snapshot(
    snapshot: &ResidentHookSnapshot,
    arguments: &[String],
    input: &str,
) -> Result<String, String> {
    let hook_arguments = match arguments.first().map(String::as_str) {
        Some("hook") => &arguments[1..],
        _ => arguments,
    };
    let client = crate::command::hook_runtime::flag_value(hook_arguments, "--client")
        .ok_or_else(|| "missing required --client <client>".to_owned())?;
    crate::command::hook_runtime::ensure_supported_client(client)?;
    let emit =
        crate::command::hook_runtime::flag_value(hook_arguments, "--emit").unwrap_or("platform");
    let event = crate::command::hook_runtime::first_positional(hook_arguments)
        .ok_or_else(|| "missing hook event".to_owned())?;
    let classification_event = if client == "codex" && event == "permission-request" {
        "pre-tool"
    } else {
        event
    };
    let payload = parse_payload(input)
        .map_err(|error| format!("invalid resident Hook payload JSON: {error:?}"))?;
    let mut decision = classify_hook_with_config(HookClassificationRequest {
        registry: &snapshot.runtime,
        config: &snapshot.config,
        platform: client,
        event: classification_event,
        payload: &payload,
    });
    decision.event = event.to_owned();
    decision.fields.insert(
        "hookEvaluationAuthority".to_owned(),
        serde_json::Value::String("runtime-server-immutable-snapshot".to_owned()),
    );
    decision.fields.insert(
        "hookPersistenceDependency".to_owned(),
        serde_json::Value::Bool(false),
    );
    crate::command::hook_runtime::enforce_resident_spawn_from_snapshot(
        &snapshot.config,
        client,
        classification_event,
        &payload,
        &mut decision,
    );
    crate::command::hook_runtime::materialize_resident_dispatch_from_snapshot(
        &payload,
        &mut decision,
    );
    crate::command::hook_runtime::materialize_source_access_from_snapshot(
        &mut decision,
        &snapshot.config,
    );
    render_decision(emit, &decision)
}

fn render_decision(emit: &str, decision: &HookDecision) -> Result<String, String> {
    match emit {
        "decision" => serde_json::to_string(decision)
            .map_err(|error| format!("failed to serialize Hook decision: {error}")),
        "platform" => {
            let value = render_platform_response(decision)
                .map_err(|error| format!("failed to render Hook response: {error:?}"))?;
            serde_json::to_string(&value)
                .map_err(|error| format!("failed to serialize resident Hook response: {error}"))
        }
        other => Err(format!(
            "unsupported --emit value: {other}; expected platform or decision"
        )),
    }
}

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_hook_evaluation.rs"]
mod tests;
