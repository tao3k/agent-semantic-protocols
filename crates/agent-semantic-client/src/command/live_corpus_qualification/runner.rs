// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Live Corpus qualification runner over the shared ASP Client application boundary.

use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use super::client_protocol::qualify_public_client_case;
use crate::command::live_corpus::LiveCorpusQualification;
use crate::command::live_corpus::live_corpus_git_repository_paths;
use crate::command::live_corpus::live_corpus_lock_digest;
use crate::command::live_corpus::load_lock;
use crate::command::live_corpus::resolve_state_home;
use crate::command::live_corpus::unique_resource;

const DEFAULT_PLAN_PATH: &str = "benchmarks/live-corpus-search-query-qualification.json";

#[derive(Debug)]
pub(super) struct QualifyArgs {
    plan_path: PathBuf,
    resource_id: Option<String>,
    language_id: Option<String>,
    json: bool,
}

struct PreparedCase {
    case: QualificationCase,
    checkout_path: PathBuf,
    qualification: LiveCorpusQualification,
    artifact_digest: String,
}

struct PreparedRun {
    args: QualifyArgs,
    plan_bytes: Vec<u8>,
    lock_bytes: Vec<u8>,
    state_home: PathBuf,
    resident_sample_count: usize,
    sequential_sample_count: usize,
    concurrent_sample_count: usize,
    cold_load_sample_count: usize,
    cases: Vec<PreparedCase>,
}

struct IsolatedBenchmarkWorkspace {
    path: PathBuf,
    workspace_identity: String,
    materialization: &'static str,
    materialization_elapsed_micros: u64,
}

impl IsolatedBenchmarkWorkspace {
    fn materialize(
        state_home: &Path,
        source: &Path,
        resource_id: &str,
        artifact_digest: &str,
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

    fn cleanup(self) -> Result<u64, String> {
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
) -> Result<(), String> {
    let args = args.to_vec();
    let prepared = tokio::task::spawn_blocking(move || prepare_run(&args))
        .await
        .map_err(|error| format!("prepare Live Corpus qualification task: {error}"))??;
    let PreparedRun {
        args,
        plan_bytes,
        lock_bytes,
        state_home,
        resident_sample_count,
        sequential_sample_count,
        concurrent_sample_count,
        cold_load_sample_count,
        cases,
    } = prepared;
    let mut receipts = Vec::with_capacity(cases.len());
    let mut cancellation_samples = Vec::with_capacity(cases.len());
    for prepared_case in cases {
        let source_path = prepared_case.checkout_path.clone();
        let state_home_for_materialization = state_home.clone();
        let resource_id = prepared_case.case.resource_id.clone();
        let artifact_digest = prepared_case.artifact_digest.clone();
        let isolated = tokio::task::spawn_blocking(move || {
            IsolatedBenchmarkWorkspace::materialize(
                &state_home_for_materialization,
                &source_path,
                &resource_id,
                &artifact_digest,
            )
        })
        .await
        .map_err(|error| format!("materialize isolated Live Corpus workspace task: {error}"))??;
        let checkout_path = isolated.path.clone();
        let workspace_identity = isolated.workspace_identity.clone();
        let materialization = isolated.materialization.to_owned();
        let materialization_elapsed_micros = isolated.materialization_elapsed_micros;
        let retained_path = checkout_path.clone();
        let case_result: Result<(QualificationCaseReceipt, u64), String> = async {
            let client = agent_semantic_client::RuntimeLanguageCommandClient;
            let cache_client = agent_semantic_client::AspClient::new_from_runtime_handoff(
                state_home.clone(),
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
                    state_home.clone(),
                    checkout_path.clone(),
                    runtime_handoff.clone(),
                )
                .cancellation_probe()
                .await?;
            if cancellation_elapsed > 1_000 {
                return Err(format!(
                    "Live Corpus cancellation probe exceeded resident budget: case={} elapsedMicros={} maximumMicros=1000",
                    prepared_case.case.case_id, cancellation_elapsed
                ));
            }
            let backpressure =
                agent_semantic_client::AspClient::new_from_runtime_handoff(
                    state_home.clone(),
                    checkout_path.clone(),
                    runtime_handoff.clone(),
                )
                .backpressure_probe()
                .await?;
            let source_merkle_root = prepared_case.qualification.source_merkle_root.clone();
            let mut qualified = qualify_case(
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
            Ok((qualified, cancellation_elapsed))
        }
        .await;
        let (mut qualified, cancellation_elapsed) = case_result.map_err(|error| {
            format!(
                "{error}; isolated benchmark workspace retained for diagnosis at {}",
                retained_path.display()
            )
        })?;
        let cleanup_elapsed_micros = tokio::task::spawn_blocking(move || isolated.cleanup())
            .await
            .map_err(|error| format!("cleanup isolated Live Corpus workspace task: {error}"))??;
        qualified.benchmark_workspace_cleanup_elapsed_micros = cleanup_elapsed_micros;
        qualified.benchmark_workspace_retained = false;
        cancellation_samples.push(cancellation_elapsed);
        receipts.push(qualified);
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
            qualified_case_count: receipts.len(),
        },
        qualified_case_count: receipts.len(),
        cases: receipts,
        status: "qualified",
    };
    let encoded = serde_json::to_string(&receipt)
        .map_err(|error| format!("encode Live Corpus qualification receipt: {error}"))?;
    let publish_state_home = state_home.clone();
    let publish_encoded = encoded.clone();
    let publish_language_id = args.language_id.clone();
    let publish_resource_id = args.resource_id.clone();
    let receipt_path = tokio::task::spawn_blocking(move || {
        publish_qualification_receipt(
            &publish_state_home,
            publish_language_id.as_deref(),
            publish_resource_id.as_deref(),
            &publish_encoded,
        )
    })
    .await
    .map_err(|error| format!("publish Live Corpus qualification task: {error}"))??;
    if args.json {
        println!("{encoded}");
    } else {
        println!(
            "[live-corpus-qualification] cases={} planDigest={} lockDigest={} receipt={} status=qualified",
            receipt.qualified_case_count,
            receipt.plan_digest,
            receipt.lock_digest,
            receipt_path.display()
        );
    }
    Ok(())
}

pub(crate) fn validate_args(args: &[String]) -> Result<(), String> {
    parse_args(args).map(|_| ())
}

fn prepare_run(args: &[String]) -> Result<PreparedRun, String> {
    let args = parse_args(args)?;
    let plan_bytes = std::fs::read(&args.plan_path).map_err(|error| {
        format!(
            "failed to read Live Corpus qualification plan {}: {error}",
            args.plan_path.display()
        )
    })?;
    let plan = serde_json::from_slice::<QualificationPlan>(&plan_bytes)
        .map_err(|error| format!("failed to decode Live Corpus qualification plan: {error}"))?;
    validate_plan(&plan)?;
    let lock_bytes = std::fs::read(&plan.lock_path).map_err(|error| {
        format!(
            "failed to read Live Corpus lock {}: {error}",
            plan.lock_path.display()
        )
    })?;
    let lock = load_lock(&plan.lock_path)?;
    let state_home = resolve_state_home()?;

    let resident_sample_count = plan.resident_sample_count;
    let sequential_sample_count = plan.sequential_sample_count;
    let concurrent_sample_count = plan.concurrent_sample_count;
    let cold_load_sample_count = plan
        .cache_states
        .iter()
        .find(|state| state.state == "cold-load")
        .map(|state| state.sample_count)
        .ok_or_else(|| "Live Corpus plan omitted cold-load cache state".to_owned())?;
    let cases = select_qualification_cases(
        plan.cases,
        args.language_id.as_deref(),
        args.resource_id.as_deref(),
    )?;
    let mut prepared_cases = Vec::with_capacity(cases.len());
    for case in cases {
        let corpus = unique_resource(&lock.corpora, &case.resource_id)?;
        if corpus.scenario_id != case.scenario_id
            || corpus.language.as_str() != case.language_id
            || corpus.provider_id != case.provider_id
        {
            return Err(format!(
                "Live Corpus qualification identity drift: resource={} planScenario={} lockScenario={} planLanguage={} lockLanguage={} planProvider={} lockProvider={}",
                case.resource_id,
                case.scenario_id,
                corpus.scenario_id,
                case.language_id,
                corpus.language,
                case.provider_id,
                corpus.provider_id
            ));
        }
        let repository = live_corpus_git_repository_paths(&state_home, &corpus.git.remote)?;
        let checkout_path = repository
            .repository_dir
            .join("checkouts")
            .join(&corpus.git.revision);
        let current_pointer = state_home
            .join("artifacts")
            .join("live-corpus")
            .join("by-resource")
            .join(&case.resource_id)
            .join("current");
        let artifact_dir = current_pointer.canonicalize().map_err(|error| {
            format!(
                "Live Corpus qualification requires a prepublished immutable artifact: resource={} pointer={} error={error}",
                case.resource_id,
                current_pointer.display()
            )
        })?;
        let qualification_path = artifact_dir.join("qualification.json");
        let qualification = serde_json::from_slice::<LiveCorpusQualification>(
            &std::fs::read(&qualification_path).map_err(|error| {
                format!(
                    "failed to read Live Corpus immutable qualification {}: {error}",
                    qualification_path.display()
                )
            })?,
        )
        .map_err(|error| {
            format!(
                "failed to decode Live Corpus immutable qualification {}: {error}",
                qualification_path.display()
            )
        })?;
        let artifact_digest = artifact_dir
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .ok_or_else(|| {
                format!(
                    "Live Corpus immutable artifact has no digest identity: {}",
                    artifact_dir.display()
                )
            })?;
        if qualification.schema_id != "agent.semantic-protocols.live-corpus-artifact-qualification"
            || qualification.schema_version != "1"
            || qualification.status != "qualified"
            || !qualification.clean
            || qualification.artifact_digest != artifact_digest
            || qualification.head_revision != corpus.git.revision
            || Path::new(&qualification.source_path) != checkout_path
        {
            return Err(format!(
                "Live Corpus immutable artifact identity drift: case={} artifact={}",
                case.case_id,
                artifact_dir.display()
            ));
        }
        prepared_cases.push(PreparedCase {
            case,
            checkout_path,
            qualification,
            artifact_digest: artifact_digest.to_owned(),
        });
    }
    Ok(PreparedRun {
        args,
        plan_bytes,
        lock_bytes,
        state_home,
        resident_sample_count,
        sequential_sample_count,
        concurrent_sample_count,
        cold_load_sample_count,
        cases: prepared_cases,
    })
}

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
) -> Result<QualificationCaseReceipt, String>
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
    )
    .await?;
    let selector = evidence
        .search
        .decision
        .selectors
        .first()
        .cloned()
        .ok_or_else(|| {
            format!(
                "Live Corpus public search selector missing: case={}",
                case.case_id
            )
        })?;
    for query in [&evidence.source, &evidence.callable_skeleton] {
        if query.generation_digest != evidence.search.generation_digest
            || query.root_digest != evidence.search.source_root_digest
            || query.provider_id != case.provider_id
        {
            return Err(format!(
                "Live Corpus public route authority drift: case={} searchGeneration={} queryGeneration={} searchRoot={} queryRoot={}",
                case.case_id,
                evidence.search.generation_digest,
                query.generation_digest,
                evidence.search.source_root_digest,
                query.root_digest
            ));
        }
    }
    if evidence.zero_match.generation_digest != evidence.search.generation_digest
        || evidence.zero_match.source_root_digest != evidence.search.source_root_digest
    {
        return Err(format!(
            "Live Corpus public zero-match route crossed generation authority: case={}",
            case.case_id
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
    let search_read_work_counters = serde_json::to_value(&evidence.search.work_counters)
        .map_err(|error| format!("encode Live Corpus search work counters: {error}"))?;
    let exact_read_work_counters = serde_json::to_value(&evidence.source.work_counters)
        .map_err(|error| format!("encode Live Corpus exact work counters: {error}"))?;
    Ok(QualificationCaseReceipt {
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
        generation_digest: evidence.search.generation_digest,
        root_digest: evidence.search.source_root_digest,
        search_operation_id: evidence.search.operation_id,
        search_elapsed_micros: evidence.search.elapsed_micros,
        resident_sample_count,
        search_resident_read_latency_micros: evidence.search_resident_read_latency_micros,
        search_service_latency_micros: evidence.search_service_latency_micros,
        search_total_latency_micros: evidence.search_total_latency_micros,
        candidate_count: evidence.search.candidate_count,
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
        search_read_mode: "synchronous-mmap",
        search_read_work_counters,
        exact_read_mode: "synchronous-mmap",
        exact_read_work_counters,
        route: "public-typed-asp-client",
        search_terminal: "ready",
        query_terminal: "ready",
        callable_skeleton_terminal: "ready",
        zero_match_terminal: "ready",
        semantic_projection_schema_id,
        payload_schema_id,
        payload_digest,
        status: "qualified",
    })
}

fn qualification_result_string(result: &serde_json::Value, field: &str) -> Result<String, String> {
    result
        .get(field)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("Live Corpus callable projection omitted {field}"))
}

#[path = "runner_contract.rs"]
mod contract_validation;

use super::contract::ClientProtocolReceipt;
use super::contract::QualificationCase;
use super::contract::QualificationCaseReceipt;
use super::contract::QualificationPlan;
use super::contract::QualificationReceipt;
use contract_validation::parse_args;
use contract_validation::select_qualification_cases;
use contract_validation::validate_plan;

#[cfg(test)]
#[path = "../../../tests/unit/command/live_corpus_qualification.rs"]
mod tests;
