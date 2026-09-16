// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Live Corpus qualification runner over the shared ASP Client application boundary.

use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use super::client_protocol::ResidentSearchLatencyBudget;
use super::client_protocol::qualify_public_client_case;
use super::client_protocol::search_receipt_for_scheme;
use super::contract::{
    AgentOrgTopologyEvidence, ClientProtocolReceipt, QualificationCase, QualificationCaseReceipt,
    QualificationReceipt,
};
use super::query_protocol::public_query_set;
#[cfg(test)]
use super::runner_contract::{parse_args, select_qualification_cases};
use super::runner_prepare::prepare_run;
#[cfg(test)]
use super::runner_prepare::{
    artifact_current_pointer, expected_coverage_certificate_count, validate_topology_scenarios,
};
use super::search_receipt::qualification_result_string;
use crate::command::live_corpus::LiveCorpusQualification;
use crate::command::live_corpus::live_corpus_lock_digest;

pub(super) const DEFAULT_PLAN_PATH: &str = "benchmarks/live-corpus-scheme-scenarios.v1.toml";
pub(super) const DEFAULT_TOPOLOGY_PLAN_PATH: &str =
    "benchmarks/live-corpus-agent-org-topology-scenarios.v1.toml";

#[derive(Debug)]
pub(super) struct QualifyArgs {
    pub(super) plan_path: PathBuf,
    pub(super) resource_id: Option<String>,
    pub(super) language_id: Option<String>,
    pub(super) json: bool,
}

pub(super) struct PreparedCase {
    pub(super) case: QualificationCase,
    pub(super) agent_prompt: String,
    pub(super) required_relation_kinds: Vec<String>,
    pub(super) composed_search: String,
    pub(super) multi_source_query: String,
    pub(super) multi_callable_skeleton_query: String,
    pub(super) minimum_composed_candidates: usize,
    pub(super) expected_coverage_certificate_count: usize,
    pub(super) checkout_path: PathBuf,
    pub(super) remote: String,
    pub(super) qualification: LiveCorpusQualification,
    pub(super) artifact_digest: String,
}

pub(super) struct PreparedRun {
    pub(super) args: QualifyArgs,
    pub(super) plan_bytes: Vec<u8>,
    pub(super) lock_bytes: Vec<u8>,
    pub(super) runtime_state_home: PathBuf,
    pub(super) resource_state_home: PathBuf,
    pub(super) resident_sample_count: usize,
    pub(super) sequential_sample_count: usize,
    pub(super) concurrent_sample_count: usize,
    pub(super) cold_load_sample_count: usize,
    pub(super) protocol_qualified_case_count: usize,
    pub(super) resident_search_latency_budget: ResidentSearchLatencyBudget,
    pub(super) cases: Vec<PreparedCase>,
}

pub(in crate::command::live_corpus) struct IsolatedBenchmarkWorkspace {
    pub(in crate::command::live_corpus) path: PathBuf,
    workspace_identity: String,
    materialization: &'static str,
    materialization_elapsed_micros: u64,
}

impl IsolatedBenchmarkWorkspace {
    pub(in crate::command::live_corpus) fn materialize(
        state_home: &Path,
        source: &Path,
        resource_id: &str,
        artifact_digest: &str,
        remote: &str,
    ) -> Result<Self, String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("read benchmark workspace clock: {error}"))?
            .as_nanos();
        let path = state_home
            .join("runtime")
            .join("live-corpus")
            .join("benchmark-workspaces")
            .join(resource_id)
            .join(artifact_digest)
            .join(format!("{}-{nonce}", std::process::id()));
        let parent = path
            .parent()
            .ok_or_else(|| "isolated benchmark workspace has no parent".to_owned())?;
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "create isolated benchmark workspace parent {}: {error}",
                parent.display()
            )
        })?;
        let started = std::time::Instant::now();
        let materialization = clone_immutable_tree(source, &path)?;
        write_benchmark_topology_manifest(&path, resource_id, remote)?;
        let materialization_elapsed_micros =
            started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
        let workspace_identity =
            agent_semantic_client_db::AgentSessionRegistry::workspace_id(&path)?;
        Ok(Self {
            path,
            workspace_identity,
            materialization,
            materialization_elapsed_micros,
        })
    }

    pub(in crate::command::live_corpus) fn cleanup(self) -> Result<u64, String> {
        let started = std::time::Instant::now();
        std::fs::remove_dir_all(&self.path).map_err(|error| {
            format!(
                "remove isolated benchmark workspace {}: {error}",
                self.path.display()
            )
        })?;
        Ok(started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64)
    }
}

fn write_benchmark_topology_manifest(
    workspace: &Path,
    resource_id: &str,
    remote: &str,
) -> Result<(), String> {
    let path = workspace.join(agent_semantic_topology::PROJECT_TOPOLOGY_MANIFEST_PATH);
    let parent = path
        .parent()
        .ok_or_else(|| "Live Corpus topology manifest has no parent".to_owned())?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("create Live Corpus topology manifest directory: {error}"))?;
    let source = format!(
        "#+TITLE: Live Corpus Project Workspace\n:PROPERTIES:\n:CONTRACT_ORG: [[../../../org/contracts/project.workspace-manifest.v1.org][project.workspace-manifest.v1]]\n:END:\n\n* Project Workspace\n:PROPERTIES:\n:PROJECT_WORKSPACE_ID: {resource_id}\n:PROJECT_WORKSPACE_IDENTITY: git+{remote}#workspace/root\n:WORKSPACE_ROOT_PATH: .\n:PORTABILITY: cross-machine\n:REPOSITORY_ALIASES: []\n:END:\n"
    );
    std::fs::write(&path, source)
        .map_err(|error| format!("write Live Corpus topology manifest: {error}"))?;
    agent_semantic_topology::ProjectTopologyManifest::load_from_project_root(workspace)
        .map_err(|error| format!("admit Live Corpus topology manifest: {error}"))?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn clone_immutable_tree(source: &Path, destination: &Path) -> Result<&'static str, String> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let source = CString::new(source.as_os_str().as_bytes())
        .map_err(|_| "Live Corpus source path contains NUL".to_owned())?;
    let destination = CString::new(destination.as_os_str().as_bytes())
        .map_err(|_| "Live Corpus benchmark path contains NUL".to_owned())?;
    // SAFETY: both C strings remain alive for the call and identify a source
    // tree plus a destination that does not yet exist.  clonefile is the APFS
    // copy-on-write authority; it does not mutate the immutable source tree.
    let result = unsafe { libc::clonefile(source.as_ptr(), destination.as_ptr(), 0) };
    if result != 0 {
        return Err(format!(
            "clone immutable Live Corpus tree with clonefile: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok("apfs-clonefile")
}

#[cfg(not(target_os = "macos"))]
fn clone_immutable_tree(source: &Path, destination: &Path) -> Result<&'static str, String> {
    hardlink_tree(source, destination)?;
    Ok("hardlink-tree")
}

#[cfg(not(target_os = "macos"))]
fn hardlink_tree(source: &Path, destination: &Path) -> Result<(), String> {
    std::fs::create_dir(destination).map_err(|error| {
        format!(
            "create isolated benchmark directory {}: {error}",
            destination.display()
        )
    })?;
    for entry in std::fs::read_dir(source).map_err(|error| {
        format!(
            "read immutable Live Corpus tree {}: {error}",
            source.display()
        )
    })? {
        let entry = entry.map_err(|error| format!("read Live Corpus directory entry: {error}"))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let metadata = std::fs::symlink_metadata(&source_path).map_err(|error| {
            format!("read Live Corpus entry {}: {error}", source_path.display())
        })?;
        if metadata.is_dir() {
            hardlink_tree(&source_path, &destination_path)?;
        } else if metadata.file_type().is_symlink() {
            let target = std::fs::read_link(&source_path).map_err(|error| {
                format!(
                    "read Live Corpus symlink {}: {error}",
                    source_path.display()
                )
            })?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(&target, &destination_path).map_err(|error| {
                format!(
                    "clone Live Corpus symlink {}: {error}",
                    source_path.display()
                )
            })?;
            #[cfg(windows)]
            {
                let _ = target;
                return Err(
                    "Live Corpus hardlink-tree does not support Windows symlinks".to_owned(),
                );
            }
        } else if std::fs::hard_link(&source_path, &destination_path).is_err() {
            std::fs::copy(&source_path, &destination_path).map_err(|error| {
                format!("copy Live Corpus file {}: {error}", source_path.display())
            })?;
        }
    }
    Ok(())
}

fn publish_qualification_receipt(
    state_home: &Path,
    language_id: Option<&str>,
    resource_id: Option<&str>,
    encoded: &str,
) -> Result<PathBuf, String> {
    let receipt_path = qualification_receipt_path(state_home, language_id, resource_id);
    let parent = receipt_path
        .parent()
        .ok_or_else(|| "Live Corpus qualification receipt has no parent".to_owned())?;
    std::fs::create_dir_all(parent).map_err(|error| {
        format!(
            "create Live Corpus qualification receipt directory {}: {error}",
            parent.display()
        )
    })?;
    let temporary = parent.join(format!(
        ".search-query-qualification.{}.tmp",
        std::process::id()
    ));
    std::fs::write(&temporary, encoded).map_err(|error| {
        format!(
            "write Live Corpus qualification receipt {}: {error}",
            temporary.display()
        )
    })?;
    std::fs::rename(&temporary, &receipt_path).map_err(|error| {
        format!(
            "publish Live Corpus qualification receipt {}: {error}",
            receipt_path.display()
        )
    })?;
    Ok(receipt_path)
}

fn publish_topology_evidence(
    state_home: &Path,
    evidence: &AgentOrgTopologyEvidence,
) -> Result<PathBuf, String> {
    let receipt_path = agent_semantic_artifacts::StateHomeLayout::new(state_home)
        .resources()
        .live_corpus()
        .join("receipts")
        .join("agent-org-topology-evidence")
        .join("by-resource")
        .join(format!("{}.json", evidence.resource_id));
    let parent = receipt_path
        .parent()
        .ok_or_else(|| "Live Corpus topology evidence has no parent".to_owned())?;
    std::fs::create_dir_all(parent).map_err(|error| {
        format!(
            "create Live Corpus topology evidence directory {}: {error}",
            parent.display()
        )
    })?;
    let encoded = serde_json::to_vec(evidence)
        .map_err(|error| format!("encode Live Corpus topology evidence: {error}"))?;
    let temporary = parent.join(format!(
        ".agent-org-topology-evidence.{}.tmp",
        std::process::id()
    ));
    std::fs::write(&temporary, encoded).map_err(|error| {
        format!(
            "write Live Corpus topology evidence {}: {error}",
            temporary.display()
        )
    })?;
    std::fs::rename(&temporary, &receipt_path).map_err(|error| {
        format!(
            "publish Live Corpus topology evidence {}: {error}",
            receipt_path.display()
        )
    })?;
    Ok(receipt_path)
}

fn qualification_receipt_path(
    state_home: &Path,
    language_id: Option<&str>,
    resource_id: Option<&str>,
) -> PathBuf {
    let receipt_root = agent_semantic_artifacts::StateHomeLayout::new(state_home)
        .resources()
        .live_corpus()
        .join("receipts");
    match (language_id, resource_id) {
        (_, Some(resource_id)) => receipt_root
            .join("search-query-qualification")
            .join("by-resource")
            .join(format!("{resource_id}.json")),
        (Some(language_id), None) => receipt_root
            .join("search-query-qualification")
            .join("by-language")
            .join(format!("{language_id}.json")),
        (None, None) => receipt_root.join("search-query-qualification.json"),
    }
}

pub(crate) async fn run(
    args: &[String],
    runtime_handoff: agent_semantic_client::AspClientRuntimeHandoff,
    runtime_state_home: PathBuf,
    resource_state_home: PathBuf,
) -> Result<(), String> {
    let args = args.to_vec();
    let prepared = agent_semantic_workspace_scheduler::RuntimeServerOwnedTask::spawn_blocking(
        "live-corpus-qualification-prepare",
        move || prepare_run(&args, runtime_state_home, resource_state_home),
    )
    .join()
    .await
    .map_err(|error| format!("prepare Live Corpus qualification task: {error}"))??;
    let PreparedRun {
        args,
        plan_bytes,
        lock_bytes,
        runtime_state_home,
        resource_state_home,
        resident_sample_count,
        sequential_sample_count,
        concurrent_sample_count,
        cold_load_sample_count,
        protocol_qualified_case_count,
        resident_search_latency_budget,
        cases,
    } = prepared;
    let selected_case_count = cases.len();
    let mut case_tasks = tokio::task::JoinSet::new();
    for (case_index, prepared_case) in cases.into_iter().enumerate() {
        let runtime_state_home = runtime_state_home.clone();
        let runtime_handoff = runtime_handoff.clone();
        case_tasks.spawn(async move {
        let source_path = prepared_case.checkout_path.clone();
        let state_home_for_materialization = runtime_state_home.clone();
        let resource_id = prepared_case.case.resource_id.clone();
        let artifact_digest = prepared_case.artifact_digest.clone();
        let remote = prepared_case.remote.clone();
        let isolated = agent_semantic_workspace_scheduler::RuntimeServerOwnedTask::spawn_blocking(
            "live-corpus-qualification-materialization",
            move || IsolatedBenchmarkWorkspace::materialize(
                &state_home_for_materialization,
                &source_path,
                &resource_id,
                &artifact_digest,
                &remote,
            ),
        )
        .join()
        .await
        .map_err(|error| format!("materialize isolated Live Corpus workspace task: {error}"))??;
        let checkout_path = isolated.path.clone();
        let workspace_identity = isolated.workspace_identity.clone();
        let materialization = isolated.materialization.to_owned();
        let materialization_elapsed_micros = isolated.materialization_elapsed_micros;
        let retained_path = checkout_path.clone();
        let case_result: Result<(QualificationCaseReceipt, AgentOrgTopologyEvidence, u64), String> = async {
            let cache_client = agent_semantic_client::AspClient::new_from_runtime_handoff(
                runtime_state_home.clone(),
                checkout_path.clone(),
                runtime_handoff.clone(),
            )
            .admit_runtime_workspace("Live Corpus qualification")
            .await?;
            let client = agent_semantic_client::RuntimeLanguageSessionClient::new(
                runtime_state_home.clone(),
                checkout_path.clone(),
                runtime_handoff.clone(),
            );
            let cold_build = cache_client
                .prepare_live_corpus_cache_state(
                    agent_semantic_client_protocol::LiveCorpusCacheStateRequest {
                        schema_id: agent_semantic_client_protocol::LIVE_CORPUS_CACHE_STATE_REQUEST_SCHEMA_ID
                            .to_owned(),
                        schema_version: "1".to_owned(),
                        operation_id: format!(
                            "live-corpus-{}-cold-build-0",
                            prepared_case.case.case_id
                        ),
                        resource_id: prepared_case.case.resource_id.clone(),
                        language_id: prepared_case.case.language_id.clone(),
                        provider_id: prepared_case.case.provider_id.clone(),
                        artifact_digest: prepared_case.artifact_digest.clone(),
                        cache_state: "cold-build".to_owned(),
                        prepare_action: "new-isolated-workspace-generation".to_owned(),
                        mutation_scope: "benchmark-workspace-generation".to_owned(),
                        expected_generation_digest: None,
                        expected_root_digest: None,
                    },
                )
                .await?;
            if cold_build.resident_generation_evicted
                || cold_build.client_session_evicted
                || cold_build.generation_digest.is_some()
                || cold_build.root_digest.is_some()
            {
                return Err(format!(
                    "Live Corpus cold-build preparation was not isolated: case={}",
                    prepared_case.case.case_id
                ));
            }
            let cancellation_elapsed =
                agent_semantic_client::AspClient::new_from_runtime_handoff(
                    runtime_state_home.clone(),
                    checkout_path.clone(),
                    runtime_handoff.clone(),
                )
                .cancellation_probe()
                .await?;
            let backpressure =
                agent_semantic_client::AspClient::new_from_runtime_handoff(
                    runtime_state_home.clone(),
                    checkout_path.clone(),
                    runtime_handoff.clone(),
                )
                .backpressure_probe()
                .await?;
            let source_merkle_root = prepared_case.qualification.source_merkle_root.clone();
            let agent_prompt = prepared_case.agent_prompt;
            let required_relation_kinds = prepared_case.required_relation_kinds;
            let composed_search = prepared_case.composed_search;
            let multi_source_query = prepared_case.multi_source_query;
            let multi_callable_skeleton_query = prepared_case.multi_callable_skeleton_query;
            let minimum_composed_candidates = prepared_case.minimum_composed_candidates;
            let expected_coverage_certificate_count =
                prepared_case.expected_coverage_certificate_count;
            let (mut qualified, topology_evidence) = qualify_case(
                &client,
                &checkout_path,
                prepared_case.case,
                prepared_case.qualification.head_revision,
                prepared_case.qualification.git_tree,
                resident_sample_count,
                sequential_sample_count,
                concurrent_sample_count,
                cold_load_sample_count,
                &cache_client,
                &prepared_case.artifact_digest,
                workspace_identity,
                materialization,
                materialization_elapsed_micros,
                cancellation_elapsed,
                backpressure,
                agent_prompt,
                required_relation_kinds,
                composed_search,
                multi_source_query,
                multi_callable_skeleton_query,
                minimum_composed_candidates,
                expected_coverage_certificate_count,
                resident_search_latency_budget,
            )
            .await?;
            if qualified.root_digest != source_merkle_root {
                return Err(format!(
                    "Live Corpus public route root does not match immutable artifact: case={} artifactRoot={} publicRoot={}",
                    qualified.case_id, source_merkle_root, qualified.root_digest
                ));
            }
            let stale_started = tokio::time::Instant::now();
            let stale_generation_digest = if qualified.generation_digest
                == format!("blake3-256:{}", "0".repeat(64))
            {
                format!("blake3-256:{}", "1".repeat(64))
            } else {
                format!("blake3-256:{}", "0".repeat(64))
            };
            let stale_error = cache_client
                .prepare_live_corpus_cache_state(
                    agent_semantic_client_protocol::LiveCorpusCacheStateRequest {
                        schema_id: agent_semantic_client_protocol::LIVE_CORPUS_CACHE_STATE_REQUEST_SCHEMA_ID
                            .to_owned(),
                        schema_version: "1".to_owned(),
                        operation_id: format!(
                            "live-corpus-{}-stale-content-binding-0",
                            qualified.case_id
                        ),
                        resource_id: qualified.resource_id.clone(),
                        language_id: qualified.language_id.clone(),
                        provider_id: qualified.provider_id.clone(),
                        artifact_digest: qualified.source_artifact_digest.clone(),
                        cache_state: "cold-load".to_owned(),
                        prepare_action: "evict-resident-generation-only".to_owned(),
                        mutation_scope: "benchmark-workspace-generation".to_owned(),
                        expected_generation_digest: Some(stale_generation_digest),
                        expected_root_digest: Some(qualified.root_digest.clone()),
                    },
                )
                .await
                .expect_err("stale generation binding must fail closed");
            if !stale_error.contains("reasonKind=cache-state-content-binding-mismatch") {
                return Err(format!(
                    "Live Corpus stale-content probe returned an untyped failure: case={} error={stale_error}",
                    qualified.case_id
                ));
            }
            let post_stale = cache_client
                .prepare_live_corpus_cache_state(
                    agent_semantic_client_protocol::LiveCorpusCacheStateRequest {
                        schema_id: agent_semantic_client_protocol::LIVE_CORPUS_CACHE_STATE_REQUEST_SCHEMA_ID
                            .to_owned(),
                        schema_version: "1".to_owned(),
                        operation_id: format!(
                            "live-corpus-{}-post-stale-warm-read-0",
                            qualified.case_id
                        ),
                        resource_id: qualified.resource_id.clone(),
                        language_id: qualified.language_id.clone(),
                        provider_id: qualified.provider_id.clone(),
                        artifact_digest: qualified.source_artifact_digest.clone(),
                        cache_state: "warm-read".to_owned(),
                        prepare_action: "reuse-exact-resident-generation".to_owned(),
                        mutation_scope: "none".to_owned(),
                        expected_generation_digest: Some(qualified.generation_digest.clone()),
                        expected_root_digest: Some(qualified.root_digest.clone()),
                    },
                )
                .await?;
            if post_stale.resident_generation_evicted
                || post_stale.client_session_evicted
                || post_stale.generation_digest.as_deref()
                    != Some(qualified.generation_digest.as_str())
                || post_stale.root_digest.as_deref() != Some(qualified.root_digest.as_str())
            {
                return Err(format!(
                    "Live Corpus stale-content probe mutated active identity: case={}",
                    qualified.case_id
                ));
            }
            qualified.stale_content_binding_rejected = true;
            qualified.stale_content_binding_probe_elapsed_micros = stale_started
                .elapsed()
                .as_micros()
                .min(u128::from(u64::MAX)) as u64;
            let release = cache_client
                .prepare_live_corpus_cache_state(
                    agent_semantic_client_protocol::LiveCorpusCacheStateRequest {
                        schema_id: agent_semantic_client_protocol::LIVE_CORPUS_CACHE_STATE_REQUEST_SCHEMA_ID
                            .to_owned(),
                        schema_version: "1".to_owned(),
                        operation_id: format!("live-corpus-{}-release-0", qualified.case_id),
                        resource_id: qualified.resource_id.clone(),
                        language_id: qualified.language_id.clone(),
                        provider_id: qualified.provider_id.clone(),
                        artifact_digest: qualified.source_artifact_digest.clone(),
                        cache_state: "released".to_owned(),
                        prepare_action: "release-exact-benchmark-generation".to_owned(),
                        mutation_scope: "benchmark-workspace-generation".to_owned(),
                        expected_generation_digest: Some(qualified.generation_digest.clone()),
                        expected_root_digest: Some(qualified.root_digest.clone()),
                    },
                )
                .await?;
            if !release.resident_generation_evicted || !release.client_session_evicted {
                return Err(format!(
                    "Live Corpus release did not evict exact Runtime/client state: case={}",
                    qualified.case_id
                ));
            }
            qualified.release_prepare_elapsed_micros = release.elapsed_micros;
            Ok((qualified, topology_evidence, cancellation_elapsed))
        }
        .await;
        let (mut qualified, topology_evidence, cancellation_elapsed) = case_result.map_err(|error| {
            format!(
                "{error}; isolated benchmark workspace retained for diagnosis at {}",
                retained_path.display()
            )
        })?;
        let cleanup_elapsed_micros = agent_semantic_workspace_scheduler::RuntimeServerOwnedTask::spawn_blocking(
            "live-corpus-qualification-cleanup",
            move || isolated.cleanup(),
        )
            .join()
            .await
            .map_err(|error| format!("cleanup isolated Live Corpus workspace task: {error}"))??;
        qualified.benchmark_workspace_cleanup_elapsed_micros = cleanup_elapsed_micros;
        qualified.benchmark_workspace_retained = false;
            Ok::<_, String>((case_index, (qualified, topology_evidence, cancellation_elapsed)))
        });
    }
    let completed_cases = agent_semantic_workspace_scheduler::join_tasks_in_plan_order(
        case_tasks,
        selected_case_count,
    )
    .await?;
    let mut receipts = Vec::with_capacity(selected_case_count);
    let mut topology_evidence = Vec::with_capacity(selected_case_count);
    let mut cancellation_samples = Vec::with_capacity(selected_case_count);
    for (_, (qualified, evidence, cancellation_elapsed)) in completed_cases {
        cancellation_samples.push(cancellation_elapsed);
        receipts.push(qualified);
        topology_evidence.push(evidence);
    }
    if cancellation_samples.len() != receipts.len() {
        return Err(
            "Live Corpus cancellation matrix did not cover every qualified case".to_owned(),
        );
    }
    let receipt = QualificationReceipt {
        schema_id: "agent.semantic-protocols.live-corpus-search-query-qualification-receipt",
        schema_version: "1",
        plan_digest: live_corpus_lock_digest(&plan_bytes),
        lock_digest: live_corpus_lock_digest(&lock_bytes),
        client_protocol: ClientProtocolReceipt {
            protocol_id: "agent.semantic-protocols.client",
            protocol_version: "1",
            transport: "grpc-tokio-streams",
            workspace_scheduling: "tokio-join-set",
            concurrent_workspace_count: selected_case_count,
            phases: [
                "initialize",
                "catalog",
                "request",
                "cancel",
                "cancelled",
                "shutdown",
            ],
            session_policy: "one-initialize-per-session",
            ready_effects: ["mpsc", "oneshot", "cancel", "response"],
            forbidden_ready_effects: [
                "process",
                "filesystem",
                "dbWrite",
                "generationMutation",
                "providerActivation",
                "controlPoll",
            ],
            non_ready_dispatch_count: 0,
            residual_task_count: 0,
            cancel_outcome: "cancelled",
            request_outcome: "cancelled",
            required_telemetry_events: [
                "client_protocol_initialize",
                "client_protocol_catalog",
                "client_protocol_request",
                "client_protocol_cancel",
                "client_protocol_cancelled",
                "client_protocol_shutdown",
            ],
            maximum_resident_micros: 1_000,
            p50_maximum_micros: 250,
            p99_maximum_micros: 700,
            max_maximum_micros: 1_000,
            qualified_case_count: protocol_qualified_case_count,
        },
        qualified_case_count: receipts.len(),
        cases: receipts,
        status: "qualified",
    };
    let encoded = serde_json::to_string(&receipt)
        .map_err(|error| format!("encode Live Corpus qualification receipt: {error}"))?;
    let topology_state_home = resource_state_home.clone();
    let topology_paths =
        agent_semantic_workspace_scheduler::RuntimeServerOwnedTask::spawn_blocking(
            "live-corpus-topology-evidence-publication",
            move || {
                topology_evidence
                    .iter()
                    .map(|evidence| publish_topology_evidence(&topology_state_home, evidence))
                    .collect::<Result<Vec<_>, _>>()
            },
        )
        .join()
        .await
        .map_err(|error| format!("publish Live Corpus topology evidence task: {error}"))??;
    let publish_state_home = resource_state_home.clone();
    let publish_encoded = encoded.clone();
    let publish_language_id = args.language_id.clone();
    let publish_resource_id = args.resource_id.clone();
    let receipt_path = agent_semantic_workspace_scheduler::RuntimeServerOwnedTask::spawn_blocking(
        "live-corpus-qualification-receipt-publication",
        move || {
            publish_qualification_receipt(
                &publish_state_home,
                publish_language_id.as_deref(),
                publish_resource_id.as_deref(),
                &publish_encoded,
            )
        },
    )
    .join()
    .await
    .map_err(|error| format!("publish Live Corpus qualification task: {error}"))??;
    if args.json {
        println!("{encoded}");
    } else {
        println!(
            "[live-corpus-qualification] cases={} planDigest={} lockDigest={} receipt={} topologyEvidenceCount={} status=qualified",
            receipt.qualified_case_count,
            receipt.plan_digest,
            receipt.lock_digest,
            receipt_path.display(),
            topology_paths.len()
        );
    }
    Ok(())
}

#[expect(
    clippy::too_many_arguments,
    reason = "qualification binds case identity, corpus, budget, client, and evidence sinks explicitly"
)]
async fn qualify_case<C>(
    client: &C,
    project_root: &std::path::Path,
    case: QualificationCase,
    revision: String,
    git_tree: String,
    resident_sample_count: usize,
    sequential_sample_count: usize,
    concurrent_sample_count: usize,
    cold_load_sample_count: usize,
    cache_client: &agent_semantic_client::AspClient,
    artifact_digest: &str,
    benchmark_workspace_identity: String,
    benchmark_workspace_materialization: String,
    benchmark_workspace_materialization_elapsed_micros: u64,
    cancellation_probe_elapsed_micros: u64,
    backpressure: agent_semantic_client::ClientBackpressureProbeReceipt,
    agent_prompt: String,
    required_relation_kinds: Vec<String>,
    composed_search: String,
    multi_source_query: String,
    multi_callable_skeleton_query: String,
    minimum_composed_candidates: usize,
    expected_coverage_certificate_count: usize,
    resident_search_latency_budget: ResidentSearchLatencyBudget,
) -> Result<(QualificationCaseReceipt, AgentOrgTopologyEvidence), String>
where
    C: agent_semantic_client::LanguageCommandClient + Clone + Send + Sync + 'static,
{
    let evidence = qualify_public_client_case(
        client,
        project_root,
        &case,
        resident_sample_count,
        sequential_sample_count,
        concurrent_sample_count,
        cold_load_sample_count,
        cache_client,
        artifact_digest,
        resident_search_latency_budget,
    )
    .await?;
    let selector = evidence.selected_selector.clone();
    let composed =
        search_receipt_for_scheme(client, project_root, &case.language_id, &composed_search)
            .await?;
    if composed.coverage_certificate_count != expected_coverage_certificate_count {
        return Err(format!(
            "reasonKind=live-corpus-search-coverage-witness-mismatch case={} observed={} expected={expected_coverage_certificate_count}",
            case.case_id, composed.coverage_certificate_count
        ));
    }
    if composed.selectors.len() < minimum_composed_candidates {
        return Err(format!(
            "Live Corpus composed Scheme Search returned too few selectors: case={} observed={} minimum={minimum_composed_candidates}",
            case.case_id,
            composed.selectors.len()
        ));
    }
    let composed_selectors = composed
        .selectors
        .iter()
        .take(minimum_composed_candidates)
        .cloned()
        .collect::<Vec<_>>();
    let composed_source = public_query_set(
        client,
        project_root,
        &case.language_id,
        &composed_selectors,
        "source",
        &multi_source_query,
    )
    .await?;
    let composed_callable_skeleton = public_query_set(
        client,
        project_root,
        &case.language_id,
        &composed_selectors,
        "callable-skeleton",
        &multi_callable_skeleton_query,
    )
    .await?;
    for query in [&composed_source, &composed_callable_skeleton] {
        if query.content_generation_digest != composed.source_generation_digest
            || query.root_digest != evidence.source.root_digest
            || query.materializations.len() != composed_selectors.len()
        {
            return Err(format!(
                "Live Corpus composed Scheme Search/Query authority drift: case={} searchContentGeneration={} queryContentGeneration={} queryParserGeneration={} expectedRoot={} queryRoot={} expectedSelectors={} materializations={}",
                case.case_id,
                composed.source_generation_digest,
                query.content_generation_digest,
                query.generation_digest,
                evidence.source.root_digest,
                query.root_digest,
                composed_selectors.len(),
                query.materializations.len(),
            ));
        }
    }
    for query in [&evidence.source, &evidence.callable_skeleton] {
        if query.generation_digest != evidence.source.generation_digest
            || query.root_digest != evidence.source.root_digest
            || query.provider_id != case.provider_id
        {
            return Err(format!(
                "Live Corpus public Query authority drift: case={} expectedGeneration={} queryGeneration={} expectedRoot={} queryRoot={}",
                case.case_id,
                evidence.source.generation_digest,
                query.generation_digest,
                evidence.source.root_digest,
                query.root_digest
            ));
        }
    }
    if evidence.zero_match.source_generation_digest != evidence.search.source_generation_digest
        || evidence.zero_match.provider_catalog_digest != evidence.search.provider_catalog_digest
        || evidence.zero_match.topology_generation_digest
            != evidence.search.topology_generation_digest
    {
        return Err(format!(
            "Live Corpus public zero-match route crossed generation authority: case={} baselineGeneration={} zeroGeneration={} baselineProviderCatalog={} zeroProviderCatalog={} baselineTopology={} zeroTopology={}",
            case.case_id,
            evidence.search.source_generation_digest,
            evidence.zero_match.source_generation_digest,
            evidence.search.provider_catalog_digest,
            evidence.zero_match.provider_catalog_digest,
            evidence.search.topology_generation_digest,
            evidence.zero_match.topology_generation_digest,
        ));
    }
    let merkle_owner_path = evidence
        .merkle_proof
        .owner_path
        .clone()
        .ok_or_else(|| "Live Corpus Merkle proof omitted ownerPath".to_owned())?;
    let merkle_source_blob_digest = evidence
        .merkle_proof
        .owner_content_digest
        .clone()
        .ok_or_else(|| "Live Corpus Merkle proof omitted ownerContentDigest".to_owned())?;
    let merkle_owner_subtree_digest = evidence
        .merkle_proof
        .owner_subtree_digest
        .clone()
        .ok_or_else(|| "Live Corpus Merkle proof omitted ownerSubtreeDigest".to_owned())?;
    let merkle_proof_digest = evidence
        .merkle_proof
        .proof_digest
        .clone()
        .ok_or_else(|| "Live Corpus Merkle proof omitted proofDigest".to_owned())?;
    let merkle_proof_step_count = evidence
        .merkle_proof
        .proof_step_count
        .ok_or_else(|| "Live Corpus Merkle proof omitted proofStepCount".to_owned())?;
    let callable_result = &evidence.callable_skeleton.result;
    let semantic_projection_schema_id = qualification_result_string(callable_result, "schemaId")?;
    let payload_schema_id = qualification_result_string(callable_result, "payloadSchemaId")?;
    let payload_digest = qualification_result_string(callable_result, "payloadDigest")?;
    let search_binding = serde_json::json!({
        "sourceGenerationDigest": evidence.search.source_generation_digest,
        "providerCatalogDigest": evidence.search.provider_catalog_digest,
        "topologyGenerationDigest": evidence.search.topology_generation_digest,
    });
    let composed_search_operation_id = composed.operation_id.clone();
    let composed_search_elapsed_micros = composed.elapsed_micros;
    let composed_candidate_count = composed.selectors.len();
    let composed_source_query_operation_id = composed_source.operation_id.clone();
    let composed_source_query_elapsed_micros = composed_source.elapsed_micros;
    let composed_callable_skeleton_operation_id = composed_callable_skeleton.operation_id.clone();
    let composed_callable_skeleton_elapsed_micros = composed_callable_skeleton.elapsed_micros;
    let topology_evidence = AgentOrgTopologyEvidence::admit(
        case.case_id.clone(),
        case.resource_id.clone(),
        evidence.search.source_generation_digest.clone(),
        evidence.search.topology_generation_digest.clone(),
        composed_selectors[0].clone(),
        composed_selectors.clone(),
        composed.operation_id,
        &composed_search,
        composed_source.operation_id,
        composed_source
            .materializations
            .into_iter()
            .map(|materialization| (materialization.selector, materialization.bytes))
            .collect(),
        composed_callable_skeleton.operation_id,
        composed_callable_skeleton
            .materializations
            .into_iter()
            .map(|materialization| (materialization.selector, materialization.bytes))
            .collect(),
        agent_prompt,
        required_relation_kinds,
    )?;
    let receipt = QualificationCaseReceipt {
        case_id: case.case_id,
        resource_id: case.resource_id,
        scenario_id: case.scenario_id,
        language_id: case.language_id,
        provider_id: case.provider_id,
        revision,
        git_tree,
        source_artifact_digest: artifact_digest.to_owned(),
        benchmark_workspace_identity,
        benchmark_workspace_materialization,
        benchmark_workspace_materialization_elapsed_micros,
        benchmark_workspace_retained: true,
        benchmark_workspace_cleanup_elapsed_micros: 0,
        release_prepare_elapsed_micros: 0,
        cancellation_probe_elapsed_micros,
        backpressure_capacity: backpressure.capacity,
        backpressure_held_call_count: backpressure.held_call_count,
        backpressure_rejected_call_count: backpressure.rejected_call_count,
        backpressure_probe_elapsed_micros: backpressure.elapsed_micros,
        stale_content_binding_rejected: false,
        stale_content_binding_probe_elapsed_micros: 0,
        generation_digest: evidence
            .merkle_proof
            .generation_digest
            .clone()
            .ok_or_else(|| "Live Corpus Merkle proof omitted generationDigest".to_owned())?,
        root_digest: evidence.source.root_digest.clone(),
        search_operation_id: evidence.search.operation_id,
        search_elapsed_micros: evidence.search.elapsed_micros,
        search_response_decode_elapsed_micros: evidence.search.response_decode_elapsed_micros,
        search_packet_bytes: evidence.search.packet_bytes,
        search_node_count: evidence.search.node_count,
        search_edge_count: evidence.search.edge_count,
        search_frontier_count: evidence.search.frontier_count,
        search_coverage_certificate_count: evidence.search.coverage_certificate_count,
        resident_sample_count,
        search_total_latency_micros: evidence.search_total_latency_micros,
        candidate_count: evidence.search.selectors.len(),
        composed_search_operation_id,
        composed_search_elapsed_micros,
        composed_search_response_decode_elapsed_micros: composed.response_decode_elapsed_micros,
        composed_search_packet_bytes: composed.packet_bytes,
        composed_search_node_count: composed.node_count,
        composed_search_edge_count: composed.edge_count,
        composed_search_frontier_count: composed.frontier_count,
        composed_search_coverage_certificate_count: composed.coverage_certificate_count,
        composed_candidate_count,
        composed_selector_count: composed_selectors.len(),
        composed_source_query_operation_id,
        composed_source_query_elapsed_micros,
        composed_callable_skeleton_operation_id,
        composed_callable_skeleton_elapsed_micros,
        selector,
        query_operation_id: evidence.source.operation_id,
        query_elapsed_micros: evidence.source.elapsed_micros,
        exact_source_latency_micros: evidence.exact_source_latency_micros,
        callable_skeleton_operation_id: evidence.callable_skeleton.operation_id,
        callable_skeleton_elapsed_micros: evidence.callable_skeleton.elapsed_micros,
        callable_skeleton_latency_micros: evidence.callable_skeleton_latency_micros,
        cold_build_sample_count: 1,
        cold_build_search_query_latency_micros: evidence.cold_build_search_query_latency_micros,
        cold_load_sample_count,
        cold_load_prepare_latency_micros: evidence.cold_load_prepare_latency_micros,
        cold_load_search_query_latency_micros: evidence.cold_load_search_query_latency_micros,
        warm_read_prepare_elapsed_micros: evidence.warm_read_prepare_elapsed_micros,
        sequential_sample_count,
        sequential_search_query_latency_micros: evidence.sequential_search_query_latency_micros,
        concurrent_sample_count,
        concurrent_search_query_latency_micros: evidence.concurrent_search_query_latency_micros,
        merkle_owner_path,
        merkle_source_blob_digest,
        merkle_owner_subtree_digest,
        merkle_proof_digest,
        merkle_proof_step_count,
        zero_match_operation_id: evidence.zero_match.operation_id,
        runtime_ecosystem: "tokio",
        search_execution_mode: "workspace-search-playbook",
        search_binding,
        query_materialization_mode: "workspace-query-playbook",
        route: "public-typed-asp-client",
        search_terminal: "ready",
        query_terminal: "ready",
        callable_skeleton_terminal: "ready",
        zero_match_terminal: "ready",
        semantic_projection_schema_id,
        payload_schema_id,
        payload_digest,
        status: "qualified",
    };
    Ok((receipt, topology_evidence))
}

#[cfg(test)]
#[path = "../../../tests/unit/command/live_corpus_qualification.rs"]
mod tests;
