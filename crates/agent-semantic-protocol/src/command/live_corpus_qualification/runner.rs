//! Live Corpus qualification runner.

use std::path::{Path, PathBuf};

use super::contract::{
    ClientProtocolReceipt, QualificationCase, QualificationCaseReceipt, QualificationPlan,
    QualificationReceipt,
};
pub(super) use super::resident_metrics::{
    ResidentSearchOutcome, require_resident_sample_budget, require_zero_runtime_work,
    resident_latency_distribution,
};
use crate::command::live_corpus::{
    LiveCorpusQualification, live_corpus_git_repository_paths, live_corpus_lock_digest, load_lock,
    resolve_state_home, unique_resource,
};
use agent_semantic_client_core::LanguageId;
use agent_semantic_client_db::runtime_server_workspace::{
    ExactProjectionKind, RuntimeProjectionScope, WorkspaceRuntimeSelectorRead,
};
use agent_semantic_content_identity::{
    callable_skeleton_projection::{CALLABLE_SKELETON_PAYLOAD_SCHEMA_ID, CallableSkeletonPayload},
    semantic_projection::{SEMANTIC_PROJECTION_SCHEMA_ID, SemanticProjection},
};

const DEFAULT_PLAN_PATH: &str = "benchmarks/live-corpus-search-query-qualification.json";

#[derive(Debug)]
pub(super) struct QualifyArgs {
    plan_path: PathBuf,
    resource_id: Option<String>,
    json: bool,
}

fn publish_qualification_receipt(state_home: &Path, encoded: &str) -> Result<PathBuf, String> {
    let receipt_path = state_home
        .join("runtime")
        .join("live-corpus")
        .join("search-query-qualification.json");
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

pub(crate) async fn run(args: &[String]) -> Result<(), String> {
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
    let endpoint = agent_semantic_client_db::read_runtime_server_endpoint(&state_home)?
        .ok_or_else(|| "Live Corpus qualification requires a healthy Runtime Server".to_owned())?;

    let resident_sample_count = plan.resident_sample_count;
    let cases = select_qualification_cases(plan.cases, args.resource_id.as_deref())?;
    let mut receipts = Vec::with_capacity(cases.len());
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
        let qualified = qualify_case(
            &endpoint,
            &checkout_path,
            case,
            resident_sample_count,
            qualification.head_revision,
            qualification.git_tree,
        )
        .await?;
        if qualified.root_digest != qualification.source_merkle_root {
            return Err(format!(
                "Live Corpus resident generation root does not match immutable artifact: case={} artifactRoot={} residentRoot={}",
                qualified.case_id, qualification.source_merkle_root, qualified.root_digest
            ));
        }
        receipts.push(qualified);
    }
    let receipt = QualificationReceipt {
        schema_id: "agent.semantic-protocols.live-corpus-search-query-qualification-receipt",
        schema_version: "1",
        plan_digest: live_corpus_lock_digest(&plan_bytes),
        lock_digest: live_corpus_lock_digest(&lock_bytes),
        client_protocol: ClientProtocolReceipt {
            protocol_id: "agent.semantic-protocols.client",
            protocol_version: "1",
            transport: "http-json",
            phases: [
                "initialize",
                "catalog",
                "request",
                "cancel",
                "cancelled",
                "shutdown",
            ],
            session_policy: plan.client_protocol.session_policy.clone(),
            ready_effects: plan.client_protocol.ready_effects.clone(),
            forbidden_ready_effects: plan.client_protocol.forbidden_ready_effects.clone(),
            non_ready_dispatch_count: plan.client_protocol.non_ready_dispatch_count,
            residual_task_count: plan.client_protocol.residual_task_count,
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
            qualified_case_count: receipts.len(),
            maximum_resident_micros: plan.client_protocol.maximum_resident_micros,
            p50_maximum_micros: plan.client_protocol.p50_maximum_micros,
            p99_maximum_micros: plan.client_protocol.p99_maximum_micros,
            max_maximum_micros: plan.client_protocol.max_maximum_micros,
        },
        qualified_case_count: receipts.len(),
        cases: receipts,
        status: "qualified",
    };
    let encoded = serde_json::to_string(&receipt)
        .map_err(|error| format!("encode Live Corpus qualification receipt: {error}"))?;
    let receipt_path = publish_qualification_receipt(&state_home, &encoded)?;
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

async fn qualify_case(
    endpoint: &agent_semantic_client_db::RuntimeServerEndpoint,
    project_root: &Path,
    case: QualificationCase,
    resident_sample_count: usize,
    revision: String,
    git_tree: String,
) -> Result<QualificationCaseReceipt, String> {
    let workspace_identity =
        agent_semantic_client_core::state_core::ResolvedState::resolve(project_root)?
            .workspace
            .workspace_id
            .to_string();
    let session = agent_semantic_client_db::WorkspaceDbIpcSession::for_runtime_server_client(
        endpoint,
        workspace_identity.clone(),
        project_root.to_path_buf(),
    );
    let required_generation = tokio::time::timeout(
        std::time::Duration::from_millis(800),
        session.restore_runtime_generation_from_pointer(),
    )
    .await
    .map_err(|_| {
        format!(
            "Live Corpus generation pointer restore exceeded 800ms: case={}",
            case.case_id
        )
    })??;
    let resident_generation_digest = required_generation.generation_digest.clone();
    let resident_root_digest = required_generation.source_root_digest.clone();
    super::client_protocol::qualify_client_protocol_case(
        &endpoint.client_http_endpoint,
        &workspace_identity,
        project_root,
        &resident_generation_digest,
        &case,
    )
    .await?;

    let language_id = LanguageId::from(case.language_id.as_str());
    let cold_prime = resident_search(
        &session,
        &resident_root_digest,
        &language_id,
        &case.search.method,
        &case.search.view,
        &case.search.terms,
        format!("live-corpus-search-{}-cold-prime", case.case_id),
    )
    .await?;
    require_zero_runtime_work(
        &case.case_id,
        "search-cold-prime",
        0,
        &cold_prime.work_counters,
    )?;
    let search = resident_search(
        &session,
        &resident_root_digest,
        &language_id,
        &case.search.method,
        &case.search.view,
        &case.search.terms,
        format!("live-corpus-search-{}-0", case.case_id),
    )
    .await?;
    let search_operation_id = search.operation_id.clone();
    if search.elapsed_micros > case.search.maximum_resident_micros {
        return Err(format!(
            "Live Corpus resident search exceeded budget: case={} residentReadMicros={} serviceMicros={} elapsedMicros={} budgetMicros={}",
            case.case_id,
            search.resident_read_elapsed_micros,
            search.service_elapsed_micros,
            search.elapsed_micros,
            case.search.maximum_resident_micros
        ));
    }
    let mut search_resident_read_latency_samples = Vec::with_capacity(resident_sample_count);
    search_resident_read_latency_samples.push(search.resident_read_elapsed_micros);
    let mut search_service_latency_samples = Vec::with_capacity(resident_sample_count);
    search_service_latency_samples.push(search.service_elapsed_micros);
    let mut search_total_latency_samples = Vec::with_capacity(resident_sample_count);
    search_total_latency_samples.push(search.elapsed_micros);
    require_zero_runtime_work(&case.case_id, "search", 0, &search.work_counters)?;
    if search.candidate_count < case.search.minimum_candidates {
        return Err(format!(
            "Live Corpus resident search returned too few candidates: case={} candidates={} minimum={}",
            case.case_id, search.candidate_count, case.search.minimum_candidates
        ));
    }
    let selector = search.selectors.first().cloned().ok_or_else(|| {
        format!(
            "Live Corpus resident search returned no parser-owned selector: case={}",
            case.case_id
        )
    })?;
    let query_result = session
        .read_runtime_exact_projection(
            language_id.clone(),
            ExactProjectionKind::Source,
            RuntimeProjectionScope::Production,
            selector.clone(),
        )
        .await?;
    let query_evidence = query_result.evidence.clone();
    let query = query_result.value;
    let query_elapsed_micros = query_evidence.elapsed_micros;
    if query_elapsed_micros > case.query.maximum_resident_micros {
        return Err(format!(
            "Live Corpus exact query exceeded budget: case={} elapsedMicros={} budgetMicros={}",
            case.case_id, query_elapsed_micros, case.query.maximum_resident_micros
        ));
    }
    let mut exact_source_latency_samples = Vec::with_capacity(resident_sample_count);
    exact_source_latency_samples.push(query_elapsed_micros);
    require_zero_runtime_work(
        &case.case_id,
        "exact-source",
        0,
        &query_evidence.work_counters,
    )?;
    let (generation_digest, root_digest, resolved_selector) = match query {
        WorkspaceRuntimeSelectorRead::Projection {
            generation_digest,
            root_digest,
            resolved_selector,
            ..
        } => (generation_digest, root_digest, resolved_selector),
        other => {
            return Err(format!(
                "Live Corpus exact query did not return a resident projection: case={} state={other:?}",
                case.case_id
            ));
        }
    };
    if resolved_selector != selector {
        return Err(format!(
            "Live Corpus source query resolved selector drifted: case={} requested={} resolved={}",
            case.case_id, selector, resolved_selector
        ));
    }
    if generation_digest != resident_generation_digest || root_digest != resident_root_digest {
        return Err(format!(
            "Live Corpus resident generation authority drifted during query: case={} reasonKind=stale-generation residentGeneration={} queryGeneration={} residentRoot={} queryRoot={} candidates=[]",
            case.case_id,
            resident_generation_digest,
            generation_digest,
            resident_root_digest,
            root_digest
        ));
    }
    let projection_result = session
        .read_runtime_exact_projection(
            language_id.clone(),
            ExactProjectionKind::CallableSkeleton,
            RuntimeProjectionScope::Production,
            selector.clone(),
        )
        .await?;
    let projection_evidence = projection_result.evidence.clone();
    let projection_read = projection_result.value;
    let projection_elapsed_micros = projection_evidence.elapsed_micros;
    if projection_elapsed_micros > case.query.maximum_resident_micros {
        return Err(format!(
            "Live Corpus callable-skeleton query exceeded budget: case={} elapsedMicros={} budgetMicros={}",
            case.case_id, projection_elapsed_micros, case.query.maximum_resident_micros
        ));
    }
    let mut callable_skeleton_latency_samples = Vec::with_capacity(resident_sample_count);
    callable_skeleton_latency_samples.push(projection_elapsed_micros);
    require_zero_runtime_work(
        &case.case_id,
        "callable-skeleton",
        0,
        &projection_evidence.work_counters,
    )?;
    let projection = match projection_read {
        WorkspaceRuntimeSelectorRead::Projection {
            generation_digest: projection_generation,
            root_digest: projection_root,
            resolved_selector: projection_selector,
            bytes,
        } => {
            if projection_generation != generation_digest
                || projection_root != root_digest
                || projection_selector != selector
            {
                return Err(format!(
                    "Live Corpus callable-skeleton authority mismatch: case={}",
                    case.case_id
                ));
            }
            serde_json::from_slice::<SemanticProjection<CallableSkeletonPayload>>(&bytes).map_err(
                |error| {
                    format!(
                        "Live Corpus callable-skeleton decode failed: case={} error={error}",
                        case.case_id
                    )
                },
            )?
        }
        other => {
            return Err(format!(
                "Live Corpus callable-skeleton query did not return a resident projection: case={} state={other:?}",
                case.case_id
            ));
        }
    };
    projection.validate().map_err(|error| {
        format!(
            "Live Corpus semantic projection validation failed: case={} error={error}",
            case.case_id
        )
    })?;
    projection.payload.validate().map_err(|error| {
        format!(
            "Live Corpus callable-skeleton payload validation failed: case={} error={error}",
            case.case_id
        )
    })?;
    projection
        .payload
        .validate_scope(&projection.root_selector)
        .map_err(|error| {
            format!(
                "Live Corpus callable-skeleton scope validation failed: case={} error={error}",
                case.case_id
            )
        })?;
    if projection.schema_id != SEMANTIC_PROJECTION_SCHEMA_ID
        || projection.payload_schema_id != CALLABLE_SKELETON_PAYLOAD_SCHEMA_ID
        || projection.projection_kind != "callable-skeleton"
        || projection.language_id != case.language_id
        || projection.provider_id != case.provider_id
        || projection.root_selector != selector
    {
        return Err(format!(
            "Live Corpus projection identity mismatch: case={} schemaId={} payloadSchemaId={} projectionKind={} languageId={} providerId={} rootSelector={}",
            case.case_id,
            projection.schema_id,
            projection.payload_schema_id,
            projection.projection_kind,
            projection.language_id,
            projection.provider_id,
            projection.root_selector,
        ));
    }
    for sample_index in 1..resident_sample_count {
        let search_sample = resident_search(
            &session,
            &resident_root_digest,
            &language_id,
            &case.search.method,
            &case.search.view,
            &case.search.terms,
            format!("live-corpus-search-{}-{sample_index}", case.case_id),
        )
        .await?;
        require_resident_sample_budget(
            &case.case_id,
            "search-total",
            sample_index,
            search_sample.elapsed_micros,
            case.search.maximum_resident_micros,
        )?;
        require_zero_runtime_work(
            &case.case_id,
            "search",
            sample_index,
            &search_sample.work_counters,
        )?;
        if search_sample.selectors.first() != Some(&selector) {
            return Err(format!(
                "Live Corpus resident search selector drift: case={} sampleIndex={sample_index}",
                case.case_id
            ));
        }
        search_resident_read_latency_samples.push(search_sample.resident_read_elapsed_micros);
        search_service_latency_samples.push(search_sample.service_elapsed_micros);
        search_total_latency_samples.push(search_sample.elapsed_micros);

        let exact_sample = session
            .read_runtime_exact_projection(
                language_id.clone(),
                ExactProjectionKind::Source,
                RuntimeProjectionScope::Production,
                selector.clone(),
            )
            .await?;
        require_resident_sample_budget(
            &case.case_id,
            "exact-source",
            sample_index,
            exact_sample.evidence.elapsed_micros,
            case.query.maximum_resident_micros,
        )?;
        require_zero_runtime_work(
            &case.case_id,
            "exact-source",
            sample_index,
            &exact_sample.evidence.work_counters,
        )?;
        let exact_elapsed_micros = exact_sample.evidence.elapsed_micros;
        match exact_sample.value {
            WorkspaceRuntimeSelectorRead::Projection {
                generation_digest: sample_generation,
                root_digest: sample_root,
                resolved_selector: sample_selector,
                ..
            } if sample_generation == generation_digest
                && sample_root == root_digest
                && sample_selector == selector => {}
            other => {
                return Err(format!(
                    "Live Corpus exact-source authority drift: case={} sampleIndex={sample_index} state={other:?}",
                    case.case_id
                ));
            }
        }
        exact_source_latency_samples.push(exact_elapsed_micros);

        let callable_sample = session
            .read_runtime_exact_projection(
                language_id.clone(),
                ExactProjectionKind::CallableSkeleton,
                RuntimeProjectionScope::Production,
                selector.clone(),
            )
            .await?;
        require_resident_sample_budget(
            &case.case_id,
            "callable-skeleton",
            sample_index,
            callable_sample.evidence.elapsed_micros,
            case.query.maximum_resident_micros,
        )?;
        require_zero_runtime_work(
            &case.case_id,
            "callable-skeleton",
            sample_index,
            &callable_sample.evidence.work_counters,
        )?;
        let callable_elapsed_micros = callable_sample.evidence.elapsed_micros;
        match callable_sample.value {
            WorkspaceRuntimeSelectorRead::Projection {
                generation_digest: sample_generation,
                root_digest: sample_root,
                resolved_selector: sample_selector,
                ..
            } if sample_generation == generation_digest
                && sample_root == root_digest
                && sample_selector == selector => {}
            other => {
                return Err(format!(
                    "Live Corpus callable-skeleton authority drift: case={} sampleIndex={sample_index} state={other:?}",
                    case.case_id
                ));
            }
        }
        callable_skeleton_latency_samples.push(callable_elapsed_micros);
    }

    let search_resident_read_latency_micros =
        resident_latency_distribution(search_resident_read_latency_samples)?;
    let search_service_latency_micros =
        resident_latency_distribution(search_service_latency_samples)?;
    let search_total_latency_micros = resident_latency_distribution(search_total_latency_samples)?;
    let exact_source_latency_micros = resident_latency_distribution(exact_source_latency_samples)?;
    let callable_skeleton_latency_micros =
        resident_latency_distribution(callable_skeleton_latency_samples)?;

    let merkle_owner_path = search.owner_paths.first().ok_or_else(|| {
        format!(
            "runtime-resident-search-owner-path-missing: case={}",
            case.case_id
        )
    })?;
    let merkle_read_result = session.read_runtime_merkle_owner(merkle_owner_path).await?;
    let merkle_telemetry_digest = merkle_read_result.evidence.telemetry_digest.clone();
    let merkle_read = merkle_read_result.value;
    let merkle_receipt = agent_semantic_client_db::runtime_merkle_owner_proof_qualification::RuntimeMerkleOwnerProofQualificationReceipt::qualified(
        agent_semantic_client_db::runtime_merkle_owner_proof_qualification::RuntimeMerkleOwnerProofEvidenceLayer::LiveCorpus,
        case.case_id.clone(),
        Some(case.resource_id.clone()),
        case.language_id.clone(),
        case.provider_id.clone(),
        selector.clone(),
        0,
        Default::default(),
        merkle_read,
    )?;
    if merkle_receipt.generation_digest.as_deref()
        != Some(required_generation.generation_digest.as_str())
        || merkle_receipt.owner_path.as_deref() != Some(merkle_owner_path.as_str())
        || merkle_receipt.proof_step_count == Some(0)
    {
        return Err(format!(
            "runtime-resident-merkle-authority-mismatch: case={} ownerPath={merkle_owner_path}",
            case.case_id
        ));
    }
    let merkle_source_blob_digest = merkle_receipt.owner_content_digest.ok_or_else(|| {
        format!(
            "runtime-resident-merkle-content-digest-missing: case={}",
            case.case_id
        )
    })?;
    let merkle_owner_subtree_digest = merkle_receipt.owner_subtree_digest.ok_or_else(|| {
        format!(
            "runtime-resident-merkle-subtree-digest-missing: case={}",
            case.case_id
        )
    })?;
    let merkle_proof_digest = merkle_receipt.proof_digest.ok_or_else(|| {
        format!(
            "runtime-resident-merkle-proof-digest-missing: case={}",
            case.case_id
        )
    })?;
    let merkle_proof_step_count = merkle_receipt.proof_step_count.ok_or_else(|| {
        format!(
            "runtime-resident-merkle-proof-steps-missing: case={}",
            case.case_id
        )
    })?;
    let query_operation_id = format!("live-corpus-query-{}", case.case_id);

    let zero_match_operation_id = format!("live-corpus-zero-match-{}", case.case_id);
    let zero_match = resident_search(
        &session,
        &resident_root_digest,
        &language_id,
        "lexical",
        "seeds",
        &case.zero_match_terms,
        zero_match_operation_id.clone(),
    )
    .await?;
    if zero_match.candidate_count != 0 {
        return Err(format!(
            "Live Corpus zero-match query returned candidates: case={} candidates={}",
            case.case_id, zero_match.candidate_count
        ));
    }
    if zero_match.elapsed_micros > case.search.maximum_resident_micros {
        return Err(format!(
            "runtime-resident-zero-match-budget-exceeded: case={} elapsedMicros={} budgetMicros={}",
            case.case_id, zero_match.elapsed_micros, case.search.maximum_resident_micros
        ));
    }
    Ok(QualificationCaseReceipt {
        case_id: case.case_id,
        resource_id: case.resource_id,
        scenario_id: case.scenario_id,
        language_id: case.language_id,
        provider_id: case.provider_id,
        revision,
        git_tree,
        generation_digest,
        root_digest,
        search_operation_id,
        search_elapsed_micros: search.elapsed_micros,
        resident_sample_count,
        search_resident_read_latency_micros,
        search_service_latency_micros,
        search_total_latency_micros,
        candidate_count: search.candidate_count,
        selector: selector.clone(),
        query_operation_id,
        query_elapsed_micros,
        exact_source_latency_micros,
        callable_skeleton_latency_micros,
        merkle_owner_path: merkle_owner_path.clone(),
        merkle_source_blob_digest,
        merkle_owner_subtree_digest,
        merkle_proof_digest,
        merkle_proof_step_count,
        zero_match_operation_id,
        source_exact_telemetry_digest: query_evidence.telemetry_digest.clone(),
        source_index_telemetry_digest: query_evidence.telemetry_digest.clone(),
        callable_skeleton_telemetry_digest: projection_evidence.telemetry_digest.clone(),
        merkle_telemetry_digest,
        runtime_ecosystem: "tokio",
        search_read_mode: "synchronous-mmap",
        search_read_work_counters:
            agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadWorkCounters {
                database_read_count: search.work_counters.database_opens,
                filesystem_read_count: search.work_counters.filesystem_reads,
                provider_process_count: search.work_counters.provider_spawns,
                scheduler_task_count: 0,
                socket_operation_count: search.work_counters.control_socket_roundtrips,
            },
        exact_read_mode: "synchronous-mmap",
        exact_read_work_counters:
            agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadWorkCounters {
                database_read_count: projection_evidence.work_counters.database_opens,
                filesystem_read_count: projection_evidence.work_counters.filesystem_reads,
                provider_process_count: projection_evidence.work_counters.provider_spawns,
                scheduler_task_count: 0,
                socket_operation_count: projection_evidence.work_counters.control_socket_roundtrips,
            },
        status: "qualified",
        semantic_projection_schema_id: projection.schema_id,
        payload_schema_id: projection.payload_schema_id,
        payload_digest: projection.payload_digest,
    })
}

async fn resident_search(
    session: &agent_semantic_client_db::WorkspaceDbIpcSession,
    resident_root: &str,
    language_id: &agent_semantic_client_core::LanguageId,
    method: &str,
    view: &str,
    terms: &[String],
    operation_id: String,
) -> Result<ResidentSearchOutcome, String> {
    if method != "lexical" || view != "seeds" {
        return Err(format!(
            "runtime-resident-search-plan-unsupported: method={method} view={view}"
        ));
    }
    let mut args = vec!["search".to_owned(), "lexical".to_owned()];
    for term in terms {
        args.push("--query".to_owned());
        args.push(term.clone());
    }
    args.push("--view".to_owned());
    args.push(view.to_owned());
    let receipt = session
        .provider_search(operation_id, language_id.clone(), args)
        .await?;
    let observed_root = receipt
        .root_digest
        .as_deref()
        .ok_or_else(|| "runtime-resident-search-source-snapshot-missing".to_owned())?
        .to_string();
    if observed_root != resident_root {
        return Err(format!(
            "runtime-resident-search-root-mismatch: expected={resident_root} observed={observed_root}"
        ));
    }
    Ok(ResidentSearchOutcome {
        operation_id: receipt.operation_id,
        resident_read_elapsed_micros: receipt.resident_read_elapsed_micros,
        service_elapsed_micros: receipt.service_elapsed_micros,
        elapsed_micros: receipt.elapsed_micros,
        candidate_count: receipt.candidate_count,
        selectors: receipt.selectors,
        owner_paths: receipt.owner_paths,
        work_counters: receipt.work_counters,
    })
}

fn validate_plan(plan: &QualificationPlan) -> Result<(), String> {
    if plan.schema_id != "agent.semantic-protocols.live-corpus-search-query-qualification-plan"
        || plan.schema_version != "1"
    {
        return Err("unsupported Live Corpus qualification plan schema".to_owned());
    }
    if plan.cases.is_empty() || plan.required_languages.is_empty() {
        return Err("Live Corpus qualification plan is empty".to_owned());
    }
    if plan.resident_sample_count < 128 {
        return Err(format!(
            "Live Corpus qualification requires at least 128 resident samples: actual={}",
            plan.resident_sample_count
        ));
    }
    if plan.client_protocol.protocol_id != "agent.semantic-protocols.client"
        || plan.client_protocol.protocol_version != "1"
        || plan.client_protocol.transport != "http-json"
        || plan.client_protocol.phases
            != [
                "initialize",
                "catalog",
                "request",
                "cancel",
                "cancelled",
                "shutdown",
            ]
        || plan.client_protocol.session_policy != "one-initialize-per-session"
        || plan.client_protocol.ready_effects != ["mpsc", "oneshot", "cancel", "response"]
        || plan.client_protocol.forbidden_ready_effects
            != [
                "process",
                "filesystem",
                "dbWrite",
                "generationMutation",
                "providerActivation",
                "controlPoll",
            ]
        || plan.client_protocol.non_ready_dispatch_count != 0
        || plan.client_protocol.residual_task_count != 0
        || plan.client_protocol.applies_to_case_count != 17
        || plan.client_protocol.maximum_resident_micros > 1_000
        || plan.client_protocol.p50_maximum_micros != 250
        || plan.client_protocol.p99_maximum_micros != 700
        || plan.client_protocol.max_maximum_micros != 1_000
    {
        return Err("Live Corpus qualification client protocol contract is invalid".to_owned());
    }
    for case in &plan.cases {
        if case.query.selector_strategy != "first-ranked-parser-owned"
            || case.query.owner_view != "items"
            || case.query.projection_scope != "live-corpus"
            || case.search.maximum_resident_micros > 1_000
            || case.query.maximum_resident_micros > 1_000
            || case.required_telemetry_events
                != [
                    "runtime_resident_search_terminal",
                    "runtime_exact_projection_terminal",
                ]
        {
            return Err(format!(
                "Live Corpus qualification case violates the resident contract: case={}",
                case.case_id
            ));
        }
    }
    Ok(())
}

fn parse_args(args: &[String]) -> Result<QualifyArgs, String> {
    let mut plan_path = PathBuf::from(DEFAULT_PLAN_PATH);
    let mut resource_id = None;
    let mut json = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--plan" => {
                index += 1;
                plan_path = PathBuf::from(args.get(index).ok_or_else(|| {
                    "live-corpus qualify requires a path after --plan".to_owned()
                })?);
            }
            "--json" => json = true,
            "--resource" => {
                index += 1;
                let value = args.get(index).ok_or_else(|| {
                    "live-corpus qualify requires a resource id after --resource".to_owned()
                })?;
                if value.trim().is_empty() {
                    return Err("live-corpus qualify resource id must be non-empty text".to_owned());
                }
                if resource_id.replace(value.clone()).is_some() {
                    return Err(
                        "live-corpus qualify accepts exactly one --resource option".to_owned()
                    );
                }
            }
            option => return Err(format!("unknown live-corpus qualify option: {option}")),
        }
        index += 1;
    }
    Ok(QualifyArgs {
        plan_path,
        resource_id,
        json,
    })
}

fn select_qualification_cases(
    cases: Vec<QualificationCase>,
    resource_id: Option<&str>,
) -> Result<Vec<QualificationCase>, String> {
    let Some(resource_id) = resource_id else {
        return Ok(cases);
    };
    let selected = cases
        .into_iter()
        .filter(|case| case.resource_id == resource_id)
        .collect::<Vec<_>>();
    match selected.len() {
        0 => Err(format!(
            "Live Corpus qualification resource was not found in plan: resource={resource_id}"
        )),
        1 => Ok(selected),
        count => Err(format!(
            "Live Corpus qualification resource is duplicated in plan: resource={resource_id} count={count}"
        )),
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/command/live_corpus_qualification.rs"]
mod tests;
