// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Query Playbook client timing and Runtime Search telemetry projection.

use std::sync::Arc;

use agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBusSender;
use agent_semantic_client_server::AspClientDispatchRequest;

use super::{AspClientWorkspaceQueryPlaybookRequest, RUNTIME_CLIENT_DISPATCH_BUDGET};

#[expect(
    clippy::too_many_arguments,
    reason = "settled timing records preserve each V1 phase measurement explicitly"
)]
pub(crate) fn record_settled_client_timing_observations(
    publication: &agent_semantic_content_identity::runtime_workspace_execution_publication::RuntimeWorkspaceExecutionPublication,
    witness: &agent_semantic_client_protocol::RuntimeSearchClientTimingWitness,
    session_id: &str,
    request_id: &str,
    language_ids: Vec<String>,
    provider_ids: Vec<String>,
    projection: Option<&str>,
    telemetry_sender: &RuntimeTelemetryBusSender,
) -> Result<agent_semantic_runtime_observability::RuntimeSearchTelemetryTrace, String> {
    let identity =
        agent_semantic_runtime_observability::RuntimeSearchTelemetryIdentity::settle_client_timing(
            publication,
            witness,
            session_id,
            request_id,
            language_ids,
            provider_ids,
        )
        .map_err(|error| error.reason_kind().to_owned())?;
    let trace = agent_semantic_runtime_observability::RuntimeSearchTelemetryTrace::new(identity);
    let budget_micros = runtime_search_trace_budget_micros();
    for phase in &witness.phases {
        let mut observation = trace
            .record_client_phase(&phase.name, phase.elapsed_micros, budget_micros)
            .map_err(|error| error.reason_kind().to_owned())?;
        observation.requested_projection = projection.map(str::to_owned);
        let _ = telemetry_sender.try_record_performance(observation);
    }
    Ok(trace)
}

pub(super) fn record_query_client_timing_observations(
    request: &AspClientDispatchRequest,
    params: &AspClientWorkspaceQueryPlaybookRequest,
    generation: Arc<crate::RuntimeQueryGeneration>,
    active_provider_targets: &[(String, String)],
    telemetry_sender: &RuntimeTelemetryBusSender,
) {
    let Some(witness) = request.client_timing_witness.clone() else {
        return;
    };
    let provider_by_language = active_provider_targets
        .iter()
        .map(|(language, provider)| (language.as_str(), provider.as_str()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut languages = std::collections::BTreeSet::new();
    let mut providers = std::collections::BTreeSet::new();
    for selector in &params.selectors {
        let Some((language, _)) = selector.split_once("://") else {
            return;
        };
        let Some(provider) = provider_by_language.get(language).copied() else {
            return;
        };
        languages.insert(language.to_owned());
        providers.insert(provider.to_owned());
    }
    let Some(publication) = generation.execution_publication() else {
        return;
    };
    let _ = record_settled_client_timing_observations(
        publication,
        &witness,
        request.session_id.as_str(),
        request.request_id.as_str(),
        languages.into_iter().collect(),
        providers.into_iter().collect(),
        Some(&params.projection),
        telemetry_sender,
    );
}

pub(crate) fn emit_runtime_search_trace_observation(
    telemetry_sender: &RuntimeTelemetryBusSender,
    observation: Result<
        agent_semantic_runtime_observability::RuntimePerformanceObservation,
        agent_semantic_runtime_observability::RuntimeSearchTelemetryError,
    >,
) {
    if let Ok(observation) = observation {
        let _ = telemetry_sender.try_record_performance(observation);
    }
}

pub(crate) fn runtime_search_trace_budget_micros() -> u64 {
    RUNTIME_CLIENT_DISPATCH_BUDGET
        .as_micros()
        .min(u128::from(u64::MAX)) as u64
}
