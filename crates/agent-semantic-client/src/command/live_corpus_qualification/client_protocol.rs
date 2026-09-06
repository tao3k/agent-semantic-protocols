// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Live Corpus qualification through the ordinary typed ASP Client application API.

use std::path::Path;
use std::time::Instant;

use agent_semantic_client::LanguageCommandClient;
use agent_semantic_client::LanguageCommandOperation;
use agent_semantic_client::LanguageCommandRequest;
use agent_semantic_client_protocol::AspClientExactQueryFailure;
use agent_semantic_client_protocol::AspClientExactQueryRequest;
use agent_semantic_client_protocol::AspClientExactQueryResponse;
use agent_semantic_client_protocol::AspClientSearchRequest;
use agent_semantic_client_protocol::ClientFrame;
use agent_semantic_client_protocol::ClientOutcome;
use agent_semantic_client_protocol::LIVE_CORPUS_CACHE_STATE_REQUEST_SCHEMA_ID;
use agent_semantic_client_protocol::LiveCorpusCacheStateRequest;
use agent_semantic_search::SearchPlaybookReceipt;

use super::contract::LatencyDistribution;
use super::contract::QualificationCase;

#[derive(Debug, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct PublicRouteFailure {
    reason_kind: String,
    message: String,
    #[serde(default)]
    details: Option<serde_json::Value>,
}

#[derive(Debug, PartialEq)]
pub(super) enum PublicRouteTerminal {
    Queued(AspClientExactQueryFailure),
    Building(AspClientExactQueryFailure),
    Ready(serde_json::Value),
    Failed(PublicRouteFailure),
    Cancelled,
}

impl PublicRouteTerminal {
    fn require_ready(self, route: &str) -> Result<serde_json::Value, String> {
        match self {
            Self::Ready(payload) => Ok(payload),
            Self::Queued(failure) => Err(format!(
                "Live Corpus public route remained Queued: route={route} reasonKind={} phase={}",
                failure.reason_kind, failure.phase
            )),
            Self::Building(failure) => Err(format!(
                "Live Corpus public route remained Building: route={route} reasonKind={} phase={}",
                failure.reason_kind, failure.phase
            )),
            Self::Failed(failure) => Err(format!(
                "Live Corpus public route failed: route={route} reasonKind={} message={}",
                failure.reason_kind, failure.message
            )),
            Self::Cancelled => Err(format!(
                "Live Corpus public route was cancelled: route={route}"
            )),
        }
    }
}

pub(super) fn typed_terminal(frame: ClientFrame) -> Result<PublicRouteTerminal, String> {
    match frame {
        ClientFrame::Response {
            outcome: ClientOutcome::Ready,
            result: Some(payload),
            error: None,
            ..
        } => Ok(PublicRouteTerminal::Ready(payload)),
        ClientFrame::Response {
            outcome: ClientOutcome::Cancelled,
            ..
        } => Ok(PublicRouteTerminal::Cancelled),
        ClientFrame::Response {
            outcome: ClientOutcome::Error | ClientOutcome::StaleGeneration,
            error: Some(error),
            ..
        } => {
            let failure = serde_json::from_value::<PublicRouteFailure>(error)
                .map_err(|error| format!("decode Live Corpus typed route failure: {error}"))?;
            if let Some(details) = failure.details.as_ref()
                && let Ok(generation_failure) =
                    serde_json::from_value::<AspClientExactQueryFailure>(details.clone())
            {
                return Ok(match generation_failure.reason_kind.as_str() {
                    "runtime-generation-queued" => PublicRouteTerminal::Queued(generation_failure),
                    "runtime-generation-building" => {
                        PublicRouteTerminal::Building(generation_failure)
                    }
                    _ => PublicRouteTerminal::Failed(failure),
                });
            }
            Ok(PublicRouteTerminal::Failed(failure))
        }
        other => Err(format!(
            "Live Corpus public route returned an invalid terminal frame: {other:?}"
        )),
    }
}

async fn dispatch_ready<C: LanguageCommandClient>(
    client: &C,
    project_root: &Path,
    language_id: &str,
    route: &str,
    operation: LanguageCommandOperation,
) -> Result<serde_json::Value, String> {
    let response = client
        .dispatch(LanguageCommandRequest {
            language_id: agent_semantic_client::LanguageId::new(language_id),
            operation,
            project_root: project_root.to_path_buf(),
            machine_readable: true,
        })
        .await?;
    typed_terminal(response.frame)?.require_ready(route)
}

#[derive(Debug)]
pub(super) struct PublicQualificationEvidence {
    pub(super) search: SearchPlaybookReceipt,
    pub(super) source: AspClientExactQueryResponse,
    pub(super) callable_skeleton: AspClientExactQueryResponse,
    pub(super) zero_match: SearchPlaybookReceipt,
    pub(super) merkle_proof: agent_semantic_client_db::runtime_merkle_owner_proof_qualification::RuntimeMerkleOwnerProofQualificationReceipt,
    pub(super) search_resident_read_latency_micros: LatencyDistribution,
    pub(super) search_service_latency_micros: LatencyDistribution,
    pub(super) search_total_latency_micros: LatencyDistribution,
    pub(super) exact_source_latency_micros: LatencyDistribution,
    pub(super) callable_skeleton_latency_micros: LatencyDistribution,
    pub(super) cold_build_search_query_latency_micros: LatencyDistribution,
    pub(super) cold_load_prepare_latency_micros: LatencyDistribution,
    pub(super) cold_load_search_query_latency_micros: LatencyDistribution,
    pub(super) warm_read_prepare_elapsed_micros: u64,
    pub(super) sequential_search_query_latency_micros: LatencyDistribution,
    pub(super) concurrent_search_query_latency_micros: LatencyDistribution,
}

pub(super) async fn qualify_public_client_case<C>(
    client: &C,
    project_root: &Path,
    case: &QualificationCase,
    resident_sample_count: usize,
    sequential_sample_count: usize,
    concurrent_sample_count: usize,
    cold_load_sample_count: usize,
    cache_client: &agent_semantic_client::AspClient,
    artifact_digest: &str,
) -> Result<PublicQualificationEvidence, String>
where
    C: LanguageCommandClient + Clone + Send + Sync + 'static,
{
    let cold_build_started = Instant::now();
    let search = search_receipt(
        client,
        project_root,
        case.language_id.as_str(),
        &case.search.method,
        &case.search.terms,
    )
    .await?;
    if search.decision.owner_paths.len() < case.search.minimum_candidates {
        return Err(format!(
            "Live Corpus public search returned too few candidates: case={} candidates={} minimum={}",
            case.case_id,
            search.decision.owner_paths.len(),
            case.search.minimum_candidates
        ));
    }
    if search.metrics.stage_elapsed_micros.indexed_lexical > case.search.maximum_resident_micros {
        return Err(format!(
            "Live Corpus public search exceeded resident budget: case={} elapsedMicros={} maximumMicros={}",
            case.case_id,
            search.metrics.stage_elapsed_micros.indexed_lexical,
            case.search.maximum_resident_micros
        ));
    }
    let selector = search.decision.selectors.first().ok_or_else(|| {
        format!(
            "Live Corpus public search returned no parser-owned selector: case={}",
            case.case_id
        )
    })?;
    let canonical_selector =
        agent_semantic_content_identity::CanonicalItemSelector::parse_root_or_exact_descendant(
            selector.clone(),
        )?;
    let owner_path = canonical_selector.owner_path()?;
    let merkle_started = Instant::now();
    let merkle_read =
        agent_semantic_client_db::workspace_db_ipc::read_runtime_merkle_owner_via_runtime_server(
            project_root,
            owner_path,
        )
        .await?;
    let merkle_elapsed_micros = merkle_started
        .elapsed()
        .as_micros()
        .min(u128::from(u64::MAX)) as u64;
    let merkle_proof = agent_semantic_client_db::runtime_merkle_owner_proof_qualification::RuntimeMerkleOwnerProofQualificationReceipt::qualified(
        agent_semantic_client_db::runtime_merkle_owner_proof_qualification::RuntimeMerkleOwnerProofEvidenceLayer::LiveCorpus,
        case.case_id.clone(),
        Some(case.resource_id.clone()),
        case.language_id.clone(),
        case.provider_id.clone(),
        selector.clone(),
        merkle_elapsed_micros,
        agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadWorkCounters::default(),
        merkle_read,
    )?;
    let source = public_query(
        client,
        project_root,
        case.language_id.as_str(),
        selector,
        "source",
    )
    .await?;
    let callable_skeleton = public_query(
        client,
        project_root,
        case.language_id.as_str(),
        selector,
        "callable-skeleton",
    )
    .await?;
    let cold_build_search_query_elapsed = cold_build_started
        .elapsed()
        .as_micros()
        .min(u128::from(u64::MAX)) as u64;
    for (projection, response) in [
        ("source", &source),
        ("callable-skeleton", &callable_skeleton),
    ] {
        if response.resident_read_elapsed_micros > case.query.maximum_resident_micros {
            return Err(format!(
                "Live Corpus public query exceeded resident budget: case={} projection={projection} elapsedMicros={} maximumMicros={}",
                case.case_id,
                response.resident_read_elapsed_micros,
                case.query.maximum_resident_micros
            ));
        }
    }
    let observed_terminal_events = std::collections::BTreeSet::from([
        "runtime_resident_search_terminal",
        "runtime_exact_projection_terminal",
    ]);
    if let Some(missing) = case
        .required_telemetry_events
        .iter()
        .find(|event| !observed_terminal_events.contains(event.as_str()))
    {
        return Err(format!(
            "Live Corpus public route did not observe required typed terminal event: case={} event={missing}",
            case.case_id
        ));
    }
    let zero_match = search_receipt(
        client,
        project_root,
        case.language_id.as_str(),
        "lexical",
        &case.zero_match_terms,
    )
    .await?;
    if !zero_match.decision.owner_paths.is_empty() {
        return Err(format!(
            "Live Corpus public zero-match search returned candidates: case={} candidates={}",
            case.case_id,
            zero_match.decision.owner_paths.len()
        ));
    }
    let warm_read_prepare = cache_client
        .prepare_live_corpus_cache_state(cache_state_request(
            case,
            artifact_digest,
            "warm-read",
            "reuse-exact-resident-generation",
            "none",
            Some(search.generation_digest.clone()),
            Some(search.source_root_digest.clone()),
            0,
        ))
        .await?;
    if warm_read_prepare.resident_generation_evicted
        || warm_read_prepare.client_session_evicted
        || warm_read_prepare.generation_digest.as_deref() != Some(search.generation_digest.as_str())
        || warm_read_prepare.root_digest.as_deref() != Some(search.source_root_digest.as_str())
    {
        return Err(format!(
            "Live Corpus warm-read preparation drifted or mutated cache state: case={}",
            case.case_id
        ));
    }
    let mut search_resident_read_samples = Vec::with_capacity(resident_sample_count);
    let mut search_service_samples = Vec::with_capacity(resident_sample_count);
    let mut search_total_samples = Vec::with_capacity(resident_sample_count);
    let mut exact_source_samples = Vec::with_capacity(resident_sample_count);
    let mut callable_skeleton_samples = Vec::with_capacity(resident_sample_count);
    for sample_index in 0..resident_sample_count {
        let sampled_search = search_receipt(
            client,
            project_root,
            case.language_id.as_str(),
            &case.search.method,
            &case.search.terms,
        )
        .await?;
        validate_sampled_search(case, &search, &sampled_search, sample_index)?;
        let sampled_source = public_query(
            client,
            project_root,
            case.language_id.as_str(),
            selector,
            "source",
        )
        .await?;
        let sampled_skeleton = public_query(
            client,
            project_root,
            case.language_id.as_str(),
            selector,
            "callable-skeleton",
        )
        .await?;
        validate_sampled_query(case, &source, &sampled_source, "source", sample_index)?;
        validate_sampled_query(
            case,
            &callable_skeleton,
            &sampled_skeleton,
            "callable-skeleton",
            sample_index,
        )?;
        search_resident_read_samples
            .push(sampled_search.metrics.stage_elapsed_micros.indexed_lexical);
        search_service_samples.push(sampled_search.service_elapsed_micros);
        search_total_samples.push(sampled_search.elapsed_micros);
        exact_source_samples.push(sampled_source.elapsed_micros);
        callable_skeleton_samples.push(sampled_skeleton.elapsed_micros);
    }
    let mut sequential_samples = Vec::with_capacity(sequential_sample_count);
    for sample_index in 0..sequential_sample_count {
        sequential_samples.push(
            run_positive_search_query_sample(
                client,
                project_root,
                case,
                &search,
                &source,
                &callable_skeleton,
                selector,
                sample_index,
            )
            .await?,
        );
    }
    let mut concurrent = tokio::task::JoinSet::new();
    for sample_index in 0..concurrent_sample_count {
        let client = client.clone();
        let project_root = project_root.to_path_buf();
        let case = case.clone();
        let search = search.clone();
        let source = source.clone();
        let callable_skeleton = callable_skeleton.clone();
        let selector = selector.clone();
        concurrent.spawn(async move {
            run_positive_search_query_sample(
                &client,
                &project_root,
                &case,
                &search,
                &source,
                &callable_skeleton,
                &selector,
                sample_index,
            )
            .await
        });
    }
    let mut concurrent_samples = Vec::with_capacity(concurrent_sample_count);
    while let Some(sample) = concurrent.join_next().await {
        concurrent_samples
            .push(sample.map_err(|error| {
                format!("Live Corpus concurrent sample task failed: {error}")
            })??);
    }
    if concurrent_samples.len() != concurrent_sample_count {
        return Err(format!(
            "Live Corpus concurrent workload did not execute every sample: expected={concurrent_sample_count} actual={}",
            concurrent_samples.len()
        ));
    }
    let mut cold_load_prepare_samples = Vec::with_capacity(cold_load_sample_count);
    let mut cold_load_search_query_samples = Vec::with_capacity(cold_load_sample_count);
    for sample_index in 0..cold_load_sample_count {
        let cache_receipt = cache_client
            .prepare_live_corpus_cache_state(cache_state_request(
                case,
                artifact_digest,
                "cold-load",
                "evict-resident-generation-only",
                "benchmark-workspace-generation",
                Some(search.generation_digest.clone()),
                Some(search.source_root_digest.clone()),
                sample_index,
            ))
            .await?;
        if !cache_receipt.resident_generation_evicted
            || !cache_receipt.client_session_evicted
            || cache_receipt.generation_digest.as_deref() != Some(search.generation_digest.as_str())
            || cache_receipt.root_digest.as_deref() != Some(search.source_root_digest.as_str())
        {
            return Err(format!(
                "Live Corpus cold-load preparation did not preserve exact content identity: case={} sample={sample_index}",
                case.case_id
            ));
        }
        cold_load_prepare_samples.push(cache_receipt.elapsed_micros);
        cold_load_search_query_samples.push(
            run_positive_search_query_sample(
                client,
                project_root,
                case,
                &search,
                &source,
                &callable_skeleton,
                selector,
                sample_index,
            )
            .await?,
        );
    }
    Ok(PublicQualificationEvidence {
        search,
        source,
        callable_skeleton,
        zero_match,
        merkle_proof,
        search_resident_read_latency_micros: distribution_with_budget(
            "search-resident-read",
            case,
            search_resident_read_samples,
            case.search.maximum_resident_micros,
        )?,
        search_service_latency_micros: distribution_with_budget(
            "search-service",
            case,
            search_service_samples,
            case.search.maximum_resident_micros,
        )?,
        search_total_latency_micros: distribution_with_budget(
            "search-total",
            case,
            search_total_samples,
            case.search.maximum_resident_micros,
        )?,
        exact_source_latency_micros: distribution_with_budget(
            "exact-source",
            case,
            exact_source_samples,
            case.query.maximum_resident_micros,
        )?,
        callable_skeleton_latency_micros: distribution_with_budget(
            "callable-skeleton",
            case,
            callable_skeleton_samples,
            case.query.maximum_resident_micros,
        )?,
        cold_build_search_query_latency_micros: LatencyDistribution::from_samples(vec![
            cold_build_search_query_elapsed,
        ])?,
        cold_load_prepare_latency_micros: LatencyDistribution::from_samples(
            cold_load_prepare_samples,
        )?,
        cold_load_search_query_latency_micros: LatencyDistribution::from_samples(
            cold_load_search_query_samples,
        )?,
        warm_read_prepare_elapsed_micros: warm_read_prepare.elapsed_micros,
        sequential_search_query_latency_micros: LatencyDistribution::from_samples(
            sequential_samples,
        )?,
        concurrent_search_query_latency_micros: LatencyDistribution::from_samples(
            concurrent_samples,
        )?,
    })
}

#[allow(clippy::too_many_arguments)]
fn cache_state_request(
    case: &QualificationCase,
    artifact_digest: &str,
    cache_state: &str,
    prepare_action: &str,
    mutation_scope: &str,
    expected_generation_digest: Option<String>,
    expected_root_digest: Option<String>,
    sample_index: usize,
) -> LiveCorpusCacheStateRequest {
    LiveCorpusCacheStateRequest {
        schema_id: LIVE_CORPUS_CACHE_STATE_REQUEST_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        operation_id: format!("live-corpus-{}-{cache_state}-{sample_index}", case.case_id),
        resource_id: case.resource_id.clone(),
        language_id: case.language_id.clone(),
        provider_id: case.provider_id.clone(),
        artifact_digest: artifact_digest.to_owned(),
        cache_state: cache_state.to_owned(),
        prepare_action: prepare_action.to_owned(),
        mutation_scope: mutation_scope.to_owned(),
        expected_generation_digest,
        expected_root_digest,
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_positive_search_query_sample<C: LanguageCommandClient>(
    client: &C,
    project_root: &Path,
    case: &QualificationCase,
    baseline_search: &SearchPlaybookReceipt,
    baseline_source: &AspClientExactQueryResponse,
    baseline_skeleton: &AspClientExactQueryResponse,
    selector: &str,
    sample_index: usize,
) -> Result<u64, String> {
    let started = Instant::now();
    let sampled_search = search_receipt(
        client,
        project_root,
        case.language_id.as_str(),
        &case.search.method,
        &case.search.terms,
    )
    .await?;
    let sampled_source = public_query(
        client,
        project_root,
        case.language_id.as_str(),
        selector,
        "source",
    )
    .await?;
    let sampled_skeleton = public_query(
        client,
        project_root,
        case.language_id.as_str(),
        selector,
        "callable-skeleton",
    )
    .await?;
    validate_sampled_search(case, baseline_search, &sampled_search, sample_index)?;
    validate_sampled_query(
        case,
        baseline_source,
        &sampled_source,
        "source",
        sample_index,
    )?;
    validate_sampled_query(
        case,
        baseline_skeleton,
        &sampled_skeleton,
        "callable-skeleton",
        sample_index,
    )?;
    Ok(started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64)
}

fn validate_sampled_search(
    case: &QualificationCase,
    baseline: &SearchPlaybookReceipt,
    sample: &SearchPlaybookReceipt,
    sample_index: usize,
) -> Result<(), String> {
    if sample.generation_digest != baseline.generation_digest
        || sample.source_root_digest != baseline.source_root_digest
        || sample.provider_digest != baseline.provider_digest
        || sample.index_artifact_digest != baseline.index_artifact_digest
        || sample.decision.selectors != baseline.decision.selectors
        || sample.decision.owner_paths != baseline.decision.owner_paths
    {
        return Err(format!(
            "Live Corpus resident search sample identity drift: case={} sample={sample_index}",
            case.case_id
        ));
    }
    Ok(())
}

fn validate_sampled_query(
    case: &QualificationCase,
    baseline: &AspClientExactQueryResponse,
    sample: &AspClientExactQueryResponse,
    projection: &str,
    sample_index: usize,
) -> Result<(), String> {
    if sample.language_id != baseline.language_id
        || sample.provider_id != baseline.provider_id
        || sample.generation_digest != baseline.generation_digest
        || sample.root_digest != baseline.root_digest
        || sample.result != baseline.result
    {
        return Err(format!(
            "Live Corpus exact-query sample identity drift: case={} projection={projection} sample={sample_index}",
            case.case_id
        ));
    }
    Ok(())
}

fn distribution_with_budget(
    operation: &str,
    case: &QualificationCase,
    samples: Vec<u64>,
    maximum_micros: u64,
) -> Result<LatencyDistribution, String> {
    let distribution = LatencyDistribution::from_samples(samples)?;
    if distribution.max_micros > maximum_micros {
        return Err(format!(
            "Live Corpus resident distribution exceeded budget: case={} operation={operation} maxMicros={} maximumMicros={maximum_micros}",
            case.case_id, distribution.max_micros
        ));
    }
    Ok(distribution)
}

async fn search_receipt<C: LanguageCommandClient>(
    client: &C,
    project_root: &Path,
    language_id: &str,
    operation: &str,
    terms: &[String],
) -> Result<SearchPlaybookReceipt, String> {
    let payload = dispatch_ready(
        client,
        project_root,
        language_id,
        "search",
        LanguageCommandOperation::Search(AspClientSearchRequest::playbook(
            operation,
            terms.join(" "),
        )),
    )
    .await?;
    let receipt = serde_json::from_value::<SearchPlaybookReceipt>(payload)
        .map_err(|error| format!("decode Live Corpus public search payload: {error}"))?;
    receipt.validate()?;
    if receipt.language_id != language_id {
        return Err(format!(
            "Live Corpus public search language drift: expected={language_id} actual={}",
            receipt.language_id
        ));
    }
    Ok(receipt)
}

async fn public_query<C: LanguageCommandClient>(
    client: &C,
    project_root: &Path,
    language_id: &str,
    selector: &str,
    projection: &str,
) -> Result<AspClientExactQueryResponse, String> {
    let payload = dispatch_ready(
        client,
        project_root,
        language_id,
        "query",
        LanguageCommandOperation::ExactQuery(AspClientExactQueryRequest {
            schema_id: "agent.semantic-protocols.asp-client-exact-query-request".to_owned(),
            schema_version: "1".to_owned(),
            selector: selector.to_owned(),
            projection: projection.to_owned(),
        }),
    )
    .await?;
    let response = serde_json::from_value::<AspClientExactQueryResponse>(payload)
        .map_err(|error| format!("decode Live Corpus public {projection} payload: {error}"))?;
    response.validate()?;
    if response.language_id != language_id {
        return Err(format!(
            "Live Corpus public query language drift: expected={language_id} actual={}",
            response.language_id
        ));
    }
    Ok(response)
}
