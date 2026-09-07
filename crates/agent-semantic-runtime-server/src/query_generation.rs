// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Immutable Ready-generation query executor owned by Runtime Server.

use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tokio_stream::Stream;

use crate::query_generation_calibration::RuntimeSearchCalibrationDecision;
use crate::query_generation_calibration::RuntimeSearchCalibrationStore;
use crate::query_generation_calibration::RuntimeSearchGenerationBuildResourceInput;
pub use crate::query_generation_calibration::RuntimeSearchGenerationBuildResourceReceipt;
use crate::query_generation_calibration::RuntimeSearchGenerationWorkloadKey;
use crate::query_generation_calibration::cached_calibration_decisions;
use crate::query_generation_calibration::load_runtime_search_calibration_store;
use crate::query_generation_calibration::persist_runtime_search_calibration_store;
use crate::query_generation_calibration::resident_index_minimum_memory_per_worker;
use crate::query_generation_calibration::resident_index_server_worker_ceiling;
use crate::query_generation_calibration::runtime_search_calibration_key;
use crate::query_generation_calibration::select_runtime_search_build_resources;
use crate::query_generation_calibration::select_single_segment_bulk;
use crate::query_generation_calibration::upsert_runtime_search_calibration_decision;
use crate::query_generation_calibration::workload_bucket;
pub use crate::runtime_query_generation::RuntimeQueryGeneration;
pub use crate::runtime_query_generation_key::RuntimeProjectWorkspaceKey;
pub use agent_semantic_search::RuntimeSearchDerivedAttachmentEvent;
use agent_semantic_search::RuntimeSearchDerivedAttachmentHub;
use agent_semantic_search::RuntimeSearchDerivedAttachmentIdentity;
pub use agent_semantic_search::RuntimeSearchDerivedAttachmentKind;
pub use agent_semantic_search::RuntimeSearchDerivedAttachmentSnapshot;
pub use agent_semantic_search::RuntimeSearchDerivedAttachmentState;

pub(super) struct RuntimeSearchGenerationBuilder {
    sender: tokio::sync::mpsc::Sender<RuntimeSearchGenerationBuilderCommand>,
    task: tokio::sync::Mutex<
        Option<
            agent_semantic_client_db::runtime_server_runtime::RuntimeServerOwnedTask<
                Result<(), String>,
            >,
        >,
    >,
    accepting: std::sync::atomic::AtomicBool,
    resource_supervisor:
        agent_semantic_client_db::runtime_server_runtime::RuntimeServerResourceSupervisor,
    throughput_by_workload: Arc<
        std::sync::Mutex<
            std::collections::BTreeMap<
                RuntimeSearchGenerationWorkloadKey,
                std::collections::BTreeMap<usize, u64>,
            >,
        >,
    >,
    calibration_store: Arc<std::sync::Mutex<RuntimeSearchCalibrationStore>>,
    calibration_store_path: Option<std::path::PathBuf>,
    engine_digest: String,
    effective_cpu: usize,
    process_memory_budget_bytes: usize,
    minimum_memory_per_worker_bytes: usize,
    attachment_hub: RuntimeSearchDerivedAttachmentHub,
}

type RuntimeSearchGenerationBuild = Box<
    dyn FnOnce() -> Result<
            agent_semantic_client_db::runtime_server_workspace::RuntimeDerivedAttachmentBuildTiming,
            String,
        > + Send
        + 'static,
>;

struct RuntimeSearchGenerationBuildJob {
    graph: RuntimeSearchGenerationBuildOperation,
    lexical: RuntimeSearchGenerationBuildOperation,
}

struct RuntimeSearchGenerationBuildOperation {
    name: &'static str,
    identity: RuntimeSearchDerivedAttachmentIdentity,
    resources: agent_semantic_client_db::runtime_server_runtime::RuntimeServerResourceRequest,
    build: RuntimeSearchGenerationBuild,
    fail: Box<dyn FnOnce(String) + Send + 'static>,
}

enum RuntimeSearchGenerationBuilderCommand {
    BuildAndWait(
        RuntimeSearchGenerationBuildJob,
        tokio::sync::oneshot::Sender<Result<(), String>>,
    ),
    Shutdown(tokio::sync::oneshot::Sender<()>),
}

fn emit_runtime_search_build_failure(
    task: &'static str,
    reason_kind: &'static str,
    error: &str,
    permit: Option<
        agent_semantic_client_db::runtime_server_runtime::RuntimeServerResourcePermitReceipt,
    >,
) {
    eprintln!(
        "[runtime-search-generation-build-resource] {}",
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.runtime-search-generation-build-resource-use-receipt",
            "schemaVersion": "1",
            "state": "failed",
            "task": task,
            "reasonKind": reason_kind,
            "error": error,
            "permit": permit,
        })
    );
}

async fn run_runtime_search_generation_build(
    task_scope: agent_semantic_client_db::runtime_server_runtime::RuntimeServerTaskScope,
    resource_supervisor: agent_semantic_client_db::runtime_server_runtime::RuntimeServerResourceSupervisor,
    attachment_hub: RuntimeSearchDerivedAttachmentHub,
    operation: RuntimeSearchGenerationBuildOperation,
) -> Result<(), String> {
    let RuntimeSearchGenerationBuildOperation {
        name,
        identity,
        resources,
        build,
        fail,
    } = operation;
    let permit = match resource_supervisor.acquire(resources).await {
        Ok(permit) => permit,
        Err(error) => {
            fail(error.clone());
            attachment_hub.publish(
                &identity,
                RuntimeSearchDerivedAttachmentState::Failed,
                None,
                Some("resource-admission-failed"),
            );
            emit_runtime_search_build_failure(name, "resource-admission-failed", &error, None);
            return Err(error);
        }
    };
    let permit_receipt = permit.receipt();
    attachment_hub.publish(
        &identity,
        RuntimeSearchDerivedAttachmentState::Building,
        None,
        None,
    );
    let task = match task_scope.spawn_blocking(name, build) {
        Ok(task) => task,
        Err(error) => {
            fail(error.clone());
            attachment_hub.publish(
                &identity,
                RuntimeSearchDerivedAttachmentState::Failed,
                None,
                Some("task-admission-failed"),
            );
            emit_runtime_search_build_failure(
                name,
                "task-admission-failed",
                &error,
                Some(permit_receipt),
            );
            return Err(error);
        }
    };
    let timing = match task.join().await {
        Ok(Ok(timing)) => timing,
        Ok(Err(error)) => {
            fail(error.clone());
            attachment_hub.publish(
                &identity,
                RuntimeSearchDerivedAttachmentState::Failed,
                None,
                Some("build-failed"),
            );
            emit_runtime_search_build_failure(name, "build-failed", &error, Some(permit_receipt));
            return Err(error);
        }
        Err(error) => {
            fail(error.clone());
            attachment_hub.publish(
                &identity,
                RuntimeSearchDerivedAttachmentState::Failed,
                None,
                Some("task-join-failed"),
            );
            emit_runtime_search_build_failure(
                name,
                "task-join-failed",
                &error,
                Some(permit_receipt),
            );
            return Err(error);
        }
    };
    attachment_hub.publish(
        &identity,
        RuntimeSearchDerivedAttachmentState::Ready,
        Some((timing.build_micros, timing.finalize_micros)),
        None,
    );
    eprintln!(
        "[runtime-search-generation-build-resource] {}",
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.runtime-search-generation-build-resource-use-receipt",
            "schemaVersion": "1",
            "state": "ready",
            "task": name,
            "permit": permit_receipt,
            "buildMicros": timing.build_micros,
            "finalizeMicros": timing.finalize_micros,
        })
    );
    Ok(())
}

fn spawn_runtime_search_generation_build_and_wait(
    builds: &mut tokio::task::JoinSet<Result<(), String>>,
    task_scope: agent_semantic_client_db::runtime_server_runtime::RuntimeServerTaskScope,
    resource_supervisor: agent_semantic_client_db::runtime_server_runtime::RuntimeServerResourceSupervisor,
    attachment_hub: RuntimeSearchDerivedAttachmentHub,
    job: RuntimeSearchGenerationBuildJob,
    terminal: tokio::sync::oneshot::Sender<Result<(), String>>,
) {
    builds.spawn(async move {
        let (graph, lexical) = tokio::join!(
            run_runtime_search_generation_build(
                task_scope.clone(),
                resource_supervisor.clone(),
                attachment_hub.clone(),
                job.graph,
            ),
            run_runtime_search_generation_build(
                task_scope,
                resource_supervisor,
                attachment_hub,
                job.lexical,
            ),
        );
        let result = graph.and(lexical);
        let _ = terminal.send(result.clone());
        result
    });
}

impl RuntimeSearchGenerationBuilder {
    #[cfg(test)]
    fn new(
        task_scope: agent_semantic_client_db::runtime_server_runtime::RuntimeServerTaskScope,
        resource_supervisor: agent_semantic_client_db::runtime_server_runtime::RuntimeServerResourceSupervisor,
    ) -> Result<Self, String> {
        Self::new_with_calibration_store(task_scope, resource_supervisor, None)
    }

    pub(super) fn new_with_calibration_store(
        task_scope: agent_semantic_client_db::runtime_server_runtime::RuntimeServerTaskScope,
        resource_supervisor: agent_semantic_client_db::runtime_server_runtime::RuntimeServerResourceSupervisor,
        calibration_store_path: Option<std::path::PathBuf>,
    ) -> Result<Self, String> {
        let effective_cpu = resource_supervisor.effective_cpu();
        let process_memory_budget_bytes = resource_supervisor.memory_budget_bytes();
        let engine_digest = agent_semantic_search::resident_index_engine_digest();
        let calibration_store = Arc::new(std::sync::Mutex::new(
            load_runtime_search_calibration_store(calibration_store_path.as_deref()),
        ));
        let minimum_memory_per_worker_bytes =
            resident_index_minimum_memory_per_worker(process_memory_budget_bytes)?;
        let (sender, mut receiver) =
            tokio::sync::mpsc::channel::<RuntimeSearchGenerationBuilderCommand>(32);
        let attachment_hub = RuntimeSearchDerivedAttachmentHub::new();
        let build_attachment_hub = attachment_hub.clone();
        let build_task_scope = task_scope.clone();
        let build_resources = resource_supervisor.clone();
        let task = task_scope.spawn("search-generation-builder", async move {
            let mut builds = tokio::task::JoinSet::new();
            let mut shutdown_receipt = None;
            loop {
                tokio::select! {
                    command = receiver.recv() => {
                        match command {
                            Some(RuntimeSearchGenerationBuilderCommand::BuildAndWait(job, terminal)) => {
                                spawn_runtime_search_generation_build_and_wait(
                                    &mut builds,
                                    build_task_scope.clone(),
                                    build_resources.clone(),
                                    build_attachment_hub.clone(),
                                    job,
                                    terminal,
                                );
                            }
                            Some(RuntimeSearchGenerationBuilderCommand::Shutdown(receipt)) => {
                                receiver.close();
                                shutdown_receipt = Some(receipt);
                                break;
                            }
                            None => break,
                        }
                    }
                    completed = builds.join_next(), if !builds.is_empty() => {
                        let _ = completed;
                    }
                }
            }
            while let Ok(command) = receiver.try_recv() {
                match command {
                    RuntimeSearchGenerationBuilderCommand::BuildAndWait(job, terminal) => {
                        spawn_runtime_search_generation_build_and_wait(
                            &mut builds,
                            build_task_scope.clone(),
                            build_resources.clone(),
                            build_attachment_hub.clone(),
                            job,
                            terminal,
                        );
                    }
                    RuntimeSearchGenerationBuilderCommand::Shutdown(_) => {}
                }
            }
            while builds.join_next().await.is_some() {}
            if let Some(receipt) = shutdown_receipt {
                let _ = receipt.send(());
            }
            Ok::<(), String>(())
        })?;
        Ok(Self {
            sender,
            task: tokio::sync::Mutex::new(Some(task)),
            accepting: std::sync::atomic::AtomicBool::new(true),
            resource_supervisor,
            throughput_by_workload: Arc::new(std::sync::Mutex::new(
                std::collections::BTreeMap::new(),
            )),
            calibration_store,
            calibration_store_path,
            engine_digest,
            effective_cpu,
            process_memory_budget_bytes,
            minimum_memory_per_worker_bytes,
            attachment_hub,
        })
    }

    pub(super) async fn build_and_wait(
        &self,
        key: &RuntimeProjectWorkspaceKey,
        _project_root: &std::path::Path,
        generation: Arc<RuntimeQueryGeneration>,
        previous: Option<&RuntimeQueryGeneration>,
    ) -> Result<(), String> {
        let (owner_count, lexical_bytes, changed_owner_count) = generation
            .resident()
            .derived_build_workload(previous.map(RuntimeQueryGeneration::resident));
        let server_worker_ceiling = resident_index_server_worker_ceiling(
            self.resource_supervisor.background_cpu(),
            self.process_memory_budget_bytes,
        )?;
        let bulk_workload_key =
            workload_bucket(lexical_bytes, owner_count, changed_owner_count, true);
        let parallel_workload_key =
            workload_bucket(lexical_bytes, owner_count, changed_owner_count, false);
        let (mut bulk_history, mut parallel_history) = {
            let throughput_by_workload = self
                .throughput_by_workload
                .lock()
                .map_err(|_| "search build throughput history is poisoned".to_owned())?;
            (
                throughput_by_workload
                    .get(&bulk_workload_key)
                    .cloned()
                    .unwrap_or_default(),
                throughput_by_workload
                    .get(&parallel_workload_key)
                    .cloned()
                    .unwrap_or_default(),
            )
        };
        let cached_decisions = {
            let store = self
                .calibration_store
                .lock()
                .map_err(|_| "Runtime search calibration store is poisoned".to_owned())?;
            cached_calibration_decisions(
                &store,
                &self.engine_digest,
                self.effective_cpu,
                self.process_memory_budget_bytes,
                bulk_workload_key,
                parallel_workload_key,
            )
        };
        for decision in cached_decisions {
            if decision.workers == 0
                || decision.workers > server_worker_ceiling
                || decision.memory_budget_bytes > self.process_memory_budget_bytes
            {
                return Err(
                    "Runtime search calibration exceeds current machine authority".to_owned(),
                );
            }
            match decision.strategy.as_str() {
                "single-segment-bulk" => {
                    bulk_history.insert(decision.workers, decision.observed_owners_per_second);
                }
                "parallel-segments" => {
                    parallel_history.insert(decision.workers, decision.observed_owners_per_second);
                }
                _ => return Err("Runtime search calibration strategy is invalid".to_owned()),
            }
        }
        let (strategy, strategy_name, workload_key, throughput_by_workers) =
            if select_single_segment_bulk(owner_count, &bulk_history, &parallel_history) {
                (
                    agent_semantic_search::ResidentIndexBuildStrategy::SingleSegmentBulk,
                    "single-segment-bulk",
                    bulk_workload_key,
                    bulk_history,
                )
            } else {
                (
                    agent_semantic_search::ResidentIndexBuildStrategy::ParallelSegments,
                    "parallel-segments",
                    parallel_workload_key,
                    parallel_history,
                )
            };
        let receipt = select_runtime_search_build_resources(
            RuntimeSearchGenerationBuildResourceInput {
                effective_cpu: self.effective_cpu,
                server_worker_ceiling,
                process_memory_budget_bytes: self.process_memory_budget_bytes,
                blocking_lane_pressure: self.resource_supervisor.active_background_cpu(),
                lexical_bytes,
                owner_count,
                changed_owner_count,
            },
            self.minimum_memory_per_worker_bytes,
            &throughput_by_workers,
            strategy_name,
        )?;
        let selected = (
            agent_semantic_search::ResidentIndexBuildResources::new(
                receipt.chosen_workers,
                receipt.memory_budget_bytes,
                strategy,
            )?,
            receipt,
            workload_key,
        );
        generation
            .build_resource_receipt
            .set(selected.1.clone())
            .map_err(|_| "search generation build resources were already published".to_owned())?;
        let content_generation_digest = generation.content_generation_digest().to_owned();
        let generation_token = generation.generation_token();
        if generation_token == 0 {
            return Err(
                "search derived attachments require a published generation token".to_owned(),
            );
        }
        let graph_build_generation = Arc::clone(&generation);
        let graph_fail_generation = Arc::clone(&generation);
        let lexical_build_generation = Arc::clone(&generation);
        let lexical_fail_generation = Arc::clone(&generation);
        let graph_content_generation_digest = content_generation_digest.clone();
        let throughput_by_workload = Arc::clone(&self.throughput_by_workload);
        let calibration_store = Arc::clone(&self.calibration_store);
        let calibration_store_path = self.calibration_store_path.clone();
        let engine_digest = self.engine_digest.clone();
        let effective_cpu = self.effective_cpu;
        let process_memory_budget_bytes = self.process_memory_budget_bytes;
        let (lexical_cpu, lexical_memory) =
            (selected.1.chosen_workers, selected.1.memory_budget_bytes);
        self.build_job_and_wait(RuntimeSearchGenerationBuildJob {
            graph: RuntimeSearchGenerationBuildOperation {
                name: "search-generation-graph-build",
                identity: RuntimeSearchDerivedAttachmentIdentity {
                    project_id: key.project_id().as_str().to_owned(),
                    workspace_id: key.workspace_id().as_str().to_owned(),
                    generation_token,
                    content_generation_digest: content_generation_digest.clone(),
                    attachment: RuntimeSearchDerivedAttachmentKind::Graph,
                },
                resources:
                    agent_semantic_client_db::runtime_server_runtime::RuntimeServerResourceRequest {
                        cpu: 1,
                        memory_bytes: lexical_bytes.max(1).min(self.process_memory_budget_bytes),
                    },
                build: Box::new(move || {
                    graph_build_generation
                        .resident()
                        .build_graph_attachment(&graph_content_generation_digest)
                }),
                fail: Box::new(move |error| {
                    graph_fail_generation
                        .resident()
                        .fail_graph_attachment(&error)
                }),
            },
            lexical: RuntimeSearchGenerationBuildOperation {
                name: "search-generation-lexical-build",
                identity: RuntimeSearchDerivedAttachmentIdentity {
                    project_id: key.project_id().as_str().to_owned(),
                    workspace_id: key.workspace_id().as_str().to_owned(),
                    generation_token,
                    content_generation_digest: content_generation_digest.clone(),
                    attachment: RuntimeSearchDerivedAttachmentKind::Tantivy,
                },
                resources:
                    agent_semantic_client_db::runtime_server_runtime::RuntimeServerResourceRequest {
                        cpu: lexical_cpu,
                        memory_bytes: lexical_memory,
                    },
                build: Box::new(move || {
                    let (resources, resource_receipt, workload_key) = selected;
                    let started = std::time::Instant::now();
                    let timing = lexical_build_generation
                        .resident()
                        .build_lexical_attachment(&content_generation_digest, resources)?;
                    let elapsed_nanos = started.elapsed().as_nanos().max(1);
                    if lexical_build_generation
                        .resident()
                        .lexical_accelerator_is_ready()
                    {
                        let owners_per_second = u64::try_from(
                            (owner_count as u128)
                                .saturating_mul(1_000_000_000)
                                .checked_div(elapsed_nanos)
                                .unwrap_or(0),
                        )
                        .unwrap_or(u64::MAX);
                        if let Ok(mut history) = throughput_by_workload.lock() {
                            history
                                .entry(workload_key)
                                .or_default()
                                .entry(resource_receipt.chosen_workers)
                                .and_modify(|observed| {
                                    *observed = observed.saturating_add(owners_per_second) / 2;
                                })
                                .or_insert(owners_per_second);
                        }
                        let mut store = calibration_store.lock().map_err(|_| {
                            "Runtime search calibration store is poisoned".to_owned()
                        })?;
                        store.schema_id =
                            "agent.semantic-protocols.runtime-search-calibration-store".to_owned();
                        store.schema_version = "1".to_owned();
                        let key = runtime_search_calibration_key(
                            &engine_digest,
                            effective_cpu,
                            process_memory_budget_bytes,
                            workload_key,
                        );
                        upsert_runtime_search_calibration_decision(
                            &mut store,
                            key,
                            RuntimeSearchCalibrationDecision {
                                strategy: resource_receipt.strategy.to_owned(),
                                workers: resource_receipt.chosen_workers,
                                memory_budget_bytes: resource_receipt.memory_budget_bytes,
                                observed_owners_per_second: owners_per_second,
                                sample_identity: content_generation_digest.clone(),
                            },
                        );
                        if let Some(path) = calibration_store_path.as_deref() {
                            persist_runtime_search_calibration_store(path, &store)?;
                        }
                    }
                    Ok(timing)
                }),
                fail: Box::new(move |error| {
                    lexical_fail_generation
                        .resident()
                        .fail_lexical_attachment(&error)
                }),
            },
        })
        .await
    }

    async fn build_job_and_wait(&self, job: RuntimeSearchGenerationBuildJob) -> Result<(), String> {
        if !self.accepting.load(Ordering::Acquire) {
            return Err("search generation builder is draining".to_owned());
        }
        let queued = [job.graph.identity.clone(), job.lexical.identity.clone()];
        let permit = self
            .sender
            .reserve()
            .await
            .map_err(|_| "search generation builder is closed".to_owned())?;
        for identity in queued {
            self.attachment_hub.publish(
                &identity,
                RuntimeSearchDerivedAttachmentState::Queued,
                None,
                None,
            );
        }
        let (terminal_sender, terminal_receiver) = tokio::sync::oneshot::channel();
        permit.send(RuntimeSearchGenerationBuilderCommand::BuildAndWait(
            job,
            terminal_sender,
        ));
        terminal_receiver.await.map_err(|_| {
            "search generation builder stopped before the joint attachment terminal".to_owned()
        })?
    }

    pub(super) fn subscribe_attachment_events(
        &self,
    ) -> Pin<Box<dyn Stream<Item = Result<RuntimeSearchDerivedAttachmentEvent, String>> + Send>>
    {
        self.attachment_hub.subscribe()
    }

    pub(super) fn derived_attachment_snapshot(
        &self,
    ) -> Arc<RuntimeSearchDerivedAttachmentSnapshot> {
        self.attachment_hub.snapshot()
    }

    pub(super) async fn shutdown(&self) -> Result<(), String> {
        if !self.accepting.swap(false, Ordering::AcqRel) {
            return Ok(());
        }
        let (receipt_sender, receipt_receiver) = tokio::sync::oneshot::channel();
        self.sender
            .send(RuntimeSearchGenerationBuilderCommand::Shutdown(
                receipt_sender,
            ))
            .await
            .map_err(|_| "search generation builder shutdown channel is closed".to_owned())?;
        receipt_receiver
            .await
            .map_err(|_| "search generation builder stopped before drain receipt".to_owned())?;
        let Some(task) = self.task.lock().await.take() else {
            return Ok(());
        };
        task.join().await??;
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/unit/query_generation.rs"]
mod tests;
