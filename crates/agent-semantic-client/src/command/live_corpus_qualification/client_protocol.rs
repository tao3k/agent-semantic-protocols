// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Live Corpus qualification through the ordinary typed ASP Client application API.

use std::path::Path;
use std::time::Instant;

use agent_semantic_client::LanguageCommandClient;
use agent_semantic_client::LanguageCommandOperation;
use agent_semantic_client::LanguageCommandRequest;
use agent_semantic_client_protocol::AspClientSearchPlaybookClauseAxis;
use agent_semantic_client_protocol::AspClientSearchPlaybookClauseRef;
use agent_semantic_client_protocol::AspClientWorkspaceSearchPlaybookRequest;
use agent_semantic_client_protocol::ClientFrame;
use agent_semantic_client_protocol::LIVE_CORPUS_CACHE_STATE_REQUEST_SCHEMA_ID;
use agent_semantic_client_protocol::LiveCorpusCacheStateRequest;

use super::contract::LatencyDistribution;
use super::contract::QualificationCase;
use super::protocol_model::ResidentSearchLatencyBudget;
use super::protocol_model::WorkspaceSearchQualificationReceipt;
use super::protocol_model::registered_producer_axis;
use super::protocol_model::typed_terminal;
use super::query_protocol::WorkspaceQueryQualificationReceipt;
use super::query_protocol::public_query;
#[cfg(test)]
pub(crate) use super::query_protocol::{
    render_workspace_query_scheme_source, workspace_query_qualification_request,
};
use super::search_receipt::workspace_search_qualification_receipt;

#[derive(Debug)]
pub(super) struct PublicQualificationEvidence {
    pub(super) search: WorkspaceSearchQualificationReceipt,
    pub(super) source: WorkspaceQueryQualificationReceipt,
    pub(super) callable_skeleton: WorkspaceQueryQualificationReceipt,
    pub(super) zero_match: WorkspaceSearchQualificationReceipt,
    pub(super) merkle_proof: agent_semantic_client_db::runtime_merkle_owner_proof_qualification::RuntimeMerkleOwnerProofQualificationReceipt,
    pub(super) selected_selector: String,
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

#[expect(
    clippy::too_many_arguments,
    reason = "qualification binds corpus identity, expected result, budget, and client evidence explicitly"
)]
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
    resident_search_budget: ResidentSearchLatencyBudget,
) -> Result<PublicQualificationEvidence, String>
where
    C: LanguageCommandClient + Clone + Send + Sync + 'static,
{
    let cold_build_started = Instant::now();
    let search = search_receipt(
        client,
        project_root,
        case.language_id.as_str(),
        &case.search,
    )
    .await?;
    if search.owner_paths.len() < case.minimum_candidates {
        return Err(format!(
            "Live Corpus public search returned too few candidates: case={} candidates={} minimum={}",
            case.case_id,
            search.owner_paths.len(),
            case.minimum_candidates
        ));
    }
    let merkle_proof = cache_client
        .read_live_corpus_merkle_owner(
            &search.owner_paths,
            &case.case_id,
            &case.resource_id,
            &case.language_id,
            &case.provider_id,
        )
        .await?;
    let selector = merkle_proof.structural_selector.clone().ok_or_else(|| {
        format!(
            "Live Corpus Merkle proof has no callable selector: case={}",
            case.case_id
        )
    })?;
    let resident_generation_digest = merkle_proof.generation_digest.clone().ok_or_else(|| {
        format!(
            "Live Corpus Merkle proof has no Runtime generation: case={}",
            case.case_id
        )
    })?;
    let source = public_query(
        client,
        project_root,
        case.language_id.as_str(),
        &selector,
        "source",
        &case.source_query,
    )
    .await?;
    let callable_skeleton = public_query(
        client,
        project_root,
        case.language_id.as_str(),
        &selector,
        "callable-skeleton",
        &case.callable_skeleton_query,
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
        if response.request_profile == "resident-hit"
            && response.elapsed_micros > case.maximum_resident_query_micros
        {
            return Err(format!(
                "Live Corpus public query exceeded resident budget: case={} projection={projection} elapsedMicros={} maximumMicros={}",
                case.case_id, response.elapsed_micros, case.maximum_resident_query_micros
            ));
        }
    }
    let observed_terminal_events = std::collections::BTreeSet::from([
        "runtime_resident_search_terminal",
        "runtime_query_playbook_terminal",
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
        &case.zero_match_search,
    )
    .await?;
    if !zero_match.owner_paths.is_empty() {
        return Err(format!(
            "Live Corpus public zero-match search returned candidates: case={} candidates={}",
            case.case_id,
            zero_match.owner_paths.len()
        ));
    }
    let warm_read_prepare = cache_client
        .prepare_live_corpus_cache_state(cache_state_request(
            case,
            artifact_digest,
            "warm-read",
            "reuse-exact-resident-generation",
            "none",
            Some(resident_generation_digest.clone()),
            Some(source.root_digest.clone()),
            0,
        ))
        .await?;
    if warm_read_prepare.resident_generation_evicted
        || warm_read_prepare.client_session_evicted
        || warm_read_prepare.generation_digest.as_deref()
            != Some(resident_generation_digest.as_str())
        || warm_read_prepare.root_digest.as_deref() != Some(source.root_digest.as_str())
    {
        return Err(format!(
            "Live Corpus warm-read preparation drifted or mutated cache state: case={}",
            case.case_id
        ));
    }
    let mut search_total_samples = Vec::with_capacity(resident_sample_count);
    let mut exact_source_samples = Vec::with_capacity(resident_sample_count);
    let mut callable_skeleton_samples = Vec::with_capacity(resident_sample_count);
    for sample_index in 0..resident_sample_count {
        let sampled_search = search_receipt(
            client,
            project_root,
            case.language_id.as_str(),
            &case.search,
        )
        .await?;
        validate_sampled_search(case, &search, &sampled_search, sample_index)?;
        let sampled_source = public_query(
            client,
            project_root,
            case.language_id.as_str(),
            &selector,
            "source",
            &case.source_query,
        )
        .await?;
        let sampled_skeleton = public_query(
            client,
            project_root,
            case.language_id.as_str(),
            &selector,
            "callable-skeleton",
            &case.callable_skeleton_query,
        )
        .await?;
        validate_sampled_query(
            case,
            &source,
            &sampled_source,
            "source",
            "resident-hit",
            sample_index,
        )?;
        validate_sampled_query(
            case,
            &callable_skeleton,
            &sampled_skeleton,
            "callable-skeleton",
            "resident-hit",
            sample_index,
        )?;
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
                &selector,
                "resident-hit",
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
                "resident-hit",
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
                Some(resident_generation_digest.clone()),
                Some(source.root_digest.clone()),
                sample_index,
            ))
            .await?;
        if !cache_receipt.resident_generation_evicted
            || !cache_receipt.client_session_evicted
            || cache_receipt.generation_digest.as_deref()
                != Some(resident_generation_digest.as_str())
            || cache_receipt.root_digest.as_deref() != Some(source.root_digest.as_str())
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
                &selector,
                "materialized",
                sample_index,
            )
            .await?,
        );
    }
    let search_total_latency_micros = distribution_with_percentile_budget(
        "search-total",
        case,
        search_total_samples,
        resident_search_budget,
    )
    .map_err(|error| {
        format!(
            "{error} packetBytes={} nodeCount={} edgeCount={} frontierCount={} coverageCertificateCount={} baselineDecodeMicros={}",
            search.packet_bytes,
            search.node_count,
            search.edge_count,
            search.frontier_count,
            search.coverage_certificate_count,
            search.response_decode_elapsed_micros,
        )
    })?;
    Ok(PublicQualificationEvidence {
        search,
        source,
        callable_skeleton,
        zero_match,
        merkle_proof,
        selected_selector: selector,
        search_total_latency_micros,
        exact_source_latency_micros: distribution_with_budget(
            "exact-source",
            case,
            exact_source_samples,
            case.maximum_resident_query_micros,
        )?,
        callable_skeleton_latency_micros: distribution_with_budget(
            "callable-skeleton",
            case,
            callable_skeleton_samples,
            case.maximum_resident_query_micros,
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
    baseline_search: &WorkspaceSearchQualificationReceipt,
    baseline_source: &WorkspaceQueryQualificationReceipt,
    baseline_skeleton: &WorkspaceQueryQualificationReceipt,
    selector: &str,
    expected_query_profile: &str,
    sample_index: usize,
) -> Result<u64, String> {
    let started = Instant::now();
    let sampled_search = search_receipt(
        client,
        project_root,
        case.language_id.as_str(),
        &case.search,
    )
    .await?;
    let sampled_source = public_query(
        client,
        project_root,
        case.language_id.as_str(),
        selector,
        "source",
        &case.source_query,
    )
    .await?;
    let sampled_skeleton = public_query(
        client,
        project_root,
        case.language_id.as_str(),
        selector,
        "callable-skeleton",
        &case.callable_skeleton_query,
    )
    .await?;
    validate_sampled_search(case, baseline_search, &sampled_search, sample_index)?;
    validate_sampled_query(
        case,
        baseline_source,
        &sampled_source,
        "source",
        expected_query_profile,
        sample_index,
    )?;
    validate_sampled_query(
        case,
        baseline_skeleton,
        &sampled_skeleton,
        "callable-skeleton",
        expected_query_profile,
        sample_index,
    )?;
    Ok(started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64)
}

fn validate_sampled_search(
    case: &QualificationCase,
    baseline: &WorkspaceSearchQualificationReceipt,
    sample: &WorkspaceSearchQualificationReceipt,
    sample_index: usize,
) -> Result<(), String> {
    if sample.source_generation_digest != baseline.source_generation_digest
        || sample.provider_catalog_digest != baseline.provider_catalog_digest
        || sample.topology_generation_digest != baseline.topology_generation_digest
        || sample.selectors != baseline.selectors
        || sample.owner_paths != baseline.owner_paths
        || sample.packet_bytes != baseline.packet_bytes
        || sample.node_count != baseline.node_count
        || sample.edge_count != baseline.edge_count
        || sample.frontier_count != baseline.frontier_count
        || sample.coverage_certificate_count != baseline.coverage_certificate_count
    {
        let drift = [
            (
                "sourceGenerationDigest",
                sample.source_generation_digest != baseline.source_generation_digest,
            ),
            (
                "providerCatalogDigest",
                sample.provider_catalog_digest != baseline.provider_catalog_digest,
            ),
            (
                "topologyGenerationDigest",
                sample.topology_generation_digest != baseline.topology_generation_digest,
            ),
            ("selectors", sample.selectors != baseline.selectors),
            ("ownerPaths", sample.owner_paths != baseline.owner_paths),
            ("packetBytes", sample.packet_bytes != baseline.packet_bytes),
            ("nodeCount", sample.node_count != baseline.node_count),
            ("edgeCount", sample.edge_count != baseline.edge_count),
            (
                "frontierCount",
                sample.frontier_count != baseline.frontier_count,
            ),
            (
                "coverageCertificateCount",
                sample.coverage_certificate_count != baseline.coverage_certificate_count,
            ),
        ]
        .into_iter()
        .filter_map(|(field, drifted)| drifted.then_some(field))
        .collect::<Vec<_>>()
        .join(",");
        let baseline_selector_set = baseline
            .selectors
            .iter()
            .collect::<std::collections::BTreeSet<_>>();
        let sample_selector_set = sample
            .selectors
            .iter()
            .collect::<std::collections::BTreeSet<_>>();
        let missing_selectors = baseline_selector_set
            .difference(&sample_selector_set)
            .take(3)
            .copied()
            .collect::<Vec<_>>();
        let added_selectors = sample_selector_set
            .difference(&baseline_selector_set)
            .take(3)
            .copied()
            .collect::<Vec<_>>();
        return Err(format!(
            "Live Corpus resident search sample identity drift: case={} sample={sample_index} fields={drift} baselineSelectors={} sampleSelectors={} baselineOwners={} sampleOwners={} missingSelectors={missing_selectors:?} addedSelectors={added_selectors:?}",
            case.case_id,
            baseline.selectors.len(),
            sample.selectors.len(),
            baseline.owner_paths.len(),
            sample.owner_paths.len(),
        ));
    }
    Ok(())
}

fn validate_sampled_query(
    case: &QualificationCase,
    baseline: &WorkspaceQueryQualificationReceipt,
    sample: &WorkspaceQueryQualificationReceipt,
    projection: &str,
    expected_request_profile: &str,
    sample_index: usize,
) -> Result<(), String> {
    if sample.request_profile != expected_request_profile
        || sample.language_id != baseline.language_id
        || sample.provider_id != baseline.provider_id
        || sample.generation_digest != baseline.generation_digest
        || sample.root_digest != baseline.root_digest
        || sample.selector != baseline.selector
        || sample.projection != baseline.projection
        || sample.bytes != baseline.bytes
        || sample.result != baseline.result
    {
        return Err(format!(
            "Live Corpus Query Playbook sample identity drift: case={} projection={projection} sample={sample_index}",
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

fn distribution_with_percentile_budget(
    operation: &str,
    case: &QualificationCase,
    samples: Vec<u64>,
    budget: ResidentSearchLatencyBudget,
) -> Result<LatencyDistribution, String> {
    let distribution = LatencyDistribution::from_samples(samples)?;
    if distribution.p50_micros > budget.p50_micros
        || distribution.p99_micros > budget.p99_micros
        || distribution.max_micros > budget.max_micros
    {
        return Err(format!(
            "Live Corpus resident distribution exceeded percentile budget: case={} operation={operation} p50Micros={} p50MaximumMicros={} p99Micros={} p99MaximumMicros={} maxMicros={} maxMaximumMicros={}",
            case.case_id,
            distribution.p50_micros,
            budget.p50_micros,
            distribution.p99_micros,
            budget.p99_micros,
            distribution.max_micros,
            budget.max_micros,
        ));
    }
    Ok(distribution)
}

async fn search_receipt<C: LanguageCommandClient>(
    client: &C,
    project_root: &Path,
    language_id: &str,
    scheme_source: &str,
) -> Result<WorkspaceSearchQualificationReceipt, String> {
    let request = workspace_search_qualification_request_with_client(
        client,
        project_root,
        language_id,
        scheme_source,
    )
    .await?;
    let started = Instant::now();
    let response = client
        .dispatch(LanguageCommandRequest {
            language_id: agent_semantic_client::LanguageId::new(language_id),
            operation: LanguageCommandOperation::WorkspaceSearchPlaybook(request),
            project_root: project_root.to_path_buf(),
            machine_readable: true,
        })
        .await?;
    let operation_id = match &response.frame {
        ClientFrame::Response { request_id, .. } => request_id.as_str().to_owned(),
        frame => {
            return Err(format!(
                "Live Corpus Search Playbook returned a non-response frame: {frame:?}"
            ));
        }
    };
    let decode_started = Instant::now();
    let payload = typed_terminal(response.frame)?.require_ready("workspace.search.playbook")?;
    let settlement = agent_semantic_search_projection::SearchTopologySettlement::admit(payload)
        .map_err(|error| format!("decode Live Corpus Search settlement: {error}"))?;
    let response_decode_elapsed_micros = decode_started
        .elapsed()
        .as_micros()
        .min(u128::from(u64::MAX)) as u64;
    workspace_search_qualification_receipt(
        settlement.as_json(),
        operation_id,
        started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64,
        response_decode_elapsed_micros,
    )
}

pub(crate) async fn search_receipt_for_literal<C: LanguageCommandClient>(
    client: &C,
    project_root: &Path,
    language_id: &str,
    literal: &str,
) -> Result<WorkspaceSearchQualificationReceipt, String> {
    if literal.is_empty() {
        return Err("Live Corpus Search literal must not be empty".to_owned());
    }
    let axis = registered_producer_axis(language_id)?;
    let literal = serde_json::to_string(literal)
        .map_err(|error| format!("encode Search Scheme literal: {error}"))?;
    let source =
        format!("(search (producers ({axis} {language_id})) (rg \"-n\" \"-F\" {literal}))");
    search_receipt(client, project_root, language_id, &source).await
}

pub(crate) async fn search_receipt_for_scheme<C: LanguageCommandClient>(
    client: &C,
    project_root: &Path,
    language_id: &str,
    scheme_source: &str,
) -> Result<WorkspaceSearchQualificationReceipt, String> {
    search_receipt(client, project_root, language_id, scheme_source).await
}

async fn workspace_search_qualification_request_with_client<C: LanguageCommandClient>(
    client: &C,
    project_root: &Path,
    producer_id: &str,
    scheme_source: &str,
) -> Result<AspClientWorkspaceSearchPlaybookRequest, String> {
    let parsed = parse_workspace_search_qualification(producer_id, scheme_source)?;
    let mut compiled = Vec::with_capacity(parsed.syntax.len());
    for block in &parsed.syntax {
        let [query_source] = block.argv.as_slice() else {
            return Err(
                "Live Corpus syntax requires exactly one enhanced Tree-sitter Query".to_owned(),
            );
        };
        let response = client
            .dispatch(LanguageCommandRequest {
                language_id: agent_semantic_client::LanguageId::new(&block.producer),
                operation: LanguageCommandOperation::WorkspaceSyntaxPlanContext(
                    agent_semantic_client_protocol::AspClientWorkspaceSyntaxPlanContextRequest {
                        schema_id: "agent.semantic-protocols.asp-client-workspace-syntax-plan-context-request".to_owned(),
                        schema_version: "1".to_owned(),
                        producer: block.producer.clone(),
                    },
                ),
                project_root: project_root.to_path_buf(),
                machine_readable: true,
            })
            .await?;
        let payload =
            typed_terminal(response.frame)?.require_ready("workspace.syntax.plan-context")?;
        let context: agent_semantic_client_protocol::AspClientWorkspaceSyntaxPlanContextResponse =
            serde_json::from_value(payload.clone())
                .map_err(|error| format!("decode Live Corpus syntax plan context: {error}"))?;
        context.validate()?;
        let plan = agent_semantic_tree_sitter::compile_resident_syntax_plan(
            query_source,
            &context.generation_digest,
            &context.capability,
        )?;
        compiled.push(
            agent_semantic_client_protocol::AspClientSearchPlaybookSyntaxBlock {
                producer: block.producer.clone(),
                plan,
            },
        );
    }
    workspace_search_request_from_parsed(parsed, (!compiled.is_empty()).then_some(compiled))
}

fn parse_workspace_search_qualification(
    producer_id: &str,
    scheme_source: &str,
) -> Result<agent_semantic_search::ProgressiveSearchPlaybookRequest, String> {
    if scheme_source.trim().is_empty() {
        return Err("Live Corpus Search requires a complete Scheme expression".to_owned());
    }
    let expected_axis = registered_producer_axis(producer_id)?;
    let parsed = agent_semantic_search::parse_progressive_search_playbook_args(&[
        "search".to_owned(),
        "playbook".to_owned(),
        scheme_source.to_owned(),
    ])
    .map_err(|error| format!("lower Live Corpus Search Scheme: {error}"))?;
    if parsed.language.as_deref() != (expected_axis == "language").then_some(producer_id)
        || parsed.documents.as_deref() != (expected_axis == "documents").then_some(producer_id)
    {
        return Err(format!(
            "Live Corpus Search Scheme producer drift: expectedAxis={expected_axis} expectedProducer={producer_id} language={:?} documents={:?}",
            parsed.language, parsed.documents
        ));
    }
    Ok(parsed)
}

fn workspace_search_request_from_parsed(
    parsed: agent_semantic_search::ProgressiveSearchPlaybookRequest,
    syntax: Option<Vec<agent_semantic_client_protocol::AspClientSearchPlaybookSyntaxBlock>>,
) -> Result<AspClientWorkspaceSearchPlaybookRequest, String> {
    let rg = (!parsed.rg.is_empty()).then_some(parsed.rg);
    let tantivy = (!parsed.tantivy.is_empty()).then_some(parsed.tantivy);
    Ok(AspClientWorkspaceSearchPlaybookRequest {
        schema_id: "agent.semantic-protocols.asp-client-workspace-search-playbook-request"
            .to_owned(),
        schema_version: "1".to_owned(),
        language: parsed.language,
        documents: parsed.documents,
        workspace: parsed.workspace,
        rg,
        tantivy,
        syntax,
        native_syntax: (!parsed.native_syntax.is_empty()).then_some(parsed.native_syntax),
        graph: (!parsed.graph.is_empty()).then(|| {
            parsed
                .graph
                .into_iter()
                .map(
                    |block| agent_semantic_client_protocol::AspClientSearchPlaybookGraphBlock {
                        language: block.language,
                        argv: block.argv,
                    },
                )
                .collect()
        }),
        composition: crate::command::root_language_facade::lower_protocol_composition(
            parsed.normalized_composition,
        ),
        clause_order: parsed
            .clause_order
            .into_iter()
            .map(|clause| AspClientSearchPlaybookClauseRef {
                axis: match clause.axis {
                    agent_semantic_search::SearchPlaybookClauseAxis::Rg => {
                        AspClientSearchPlaybookClauseAxis::Rg
                    }
                    agent_semantic_search::SearchPlaybookClauseAxis::Tantivy => {
                        AspClientSearchPlaybookClauseAxis::Tantivy
                    }
                    agent_semantic_search::SearchPlaybookClauseAxis::Syntax => {
                        AspClientSearchPlaybookClauseAxis::Syntax
                    }
                    agent_semantic_search::SearchPlaybookClauseAxis::NativeSyntax => {
                        AspClientSearchPlaybookClauseAxis::NativeSyntax
                    }
                    agent_semantic_search::SearchPlaybookClauseAxis::Graph => {
                        AspClientSearchPlaybookClauseAxis::Graph
                    }
                },
                block_index: clause.block_index,
            })
            .collect(),
    })
}
