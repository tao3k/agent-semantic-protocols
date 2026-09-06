// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use super::{
    IncidentIdentity, IncidentObservation, IncidentRecord, IncidentSurface, IncidentTelemetryEvent,
    RequestedProjection, ResourceObservation, TransitionError, observe,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchIncidentTerminalOutcome {
    Succeeded {
        code_projection_present: bool,
    },
    Failed {
        reason_kind: String,
    },
    Blocked {
        reason_kind: String,
    },
    BudgetExceeded {
        reason_kind: String,
        code_projection_present: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchIncidentTerminalContext {
    pub workspace_identity: String,
    pub language_id: String,
    pub surface: IncidentSurface,
    pub canonical_request_digest: String,
    pub requested_projection: RequestedProjection,
    pub stage: String,
    pub runtime_artifact_digest: Option<String>,
    pub provider_contract_digest: Option<String>,
    pub generation_digest: Option<String>,
    pub budget_micros: Option<u64>,
    pub elapsed_micros: Option<u64>,
    pub observed_at_unix_micros: u64,
    pub resources: ResourceObservation,
}

pub fn observe_terminal(
    existing: Option<IncidentRecord>,
    context: SearchIncidentTerminalContext,
    outcome: SearchIncidentTerminalOutcome,
) -> Result<Option<(IncidentRecord, IncidentTelemetryEvent)>, TransitionError> {
    let repeated = existing.is_some();
    let observation = terminal_observation(context, outcome);
    let Some(record) = observe(existing, observation.clone())? else {
        return Ok(None);
    };
    let transition = if repeated {
        "repeated-failure"
    } else {
        "failure-observed"
    };
    let event = IncidentTelemetryEvent::from_record(observation, &record, transition)?;
    Ok(Some((record, event)))
}

fn terminal_observation(
    context: SearchIncidentTerminalContext,
    outcome: SearchIncidentTerminalOutcome,
) -> IncidentObservation {
    let (succeeded, blocked, budget_exceeded, code_projection_present, reason_kind) = match outcome
    {
        SearchIncidentTerminalOutcome::Succeeded {
            code_projection_present,
        } => {
            let reason = if context.requested_projection.requires_code() && !code_projection_present
            {
                "requested-code-projection-missing"
            } else {
                "none"
            };
            (
                true,
                false,
                false,
                code_projection_present,
                reason.to_owned(),
            )
        }
        SearchIncidentTerminalOutcome::Failed { reason_kind } => {
            (false, false, false, false, reason_kind)
        }
        SearchIncidentTerminalOutcome::Blocked { reason_kind } => {
            (false, true, false, false, reason_kind)
        }
        SearchIncidentTerminalOutcome::BudgetExceeded {
            reason_kind,
            code_projection_present,
        } => (false, false, true, code_projection_present, reason_kind),
    };
    let incident_id = incident_id(&context, &reason_kind);
    IncidentObservation {
        identity: IncidentIdentity {
            incident_id,
            workspace_identity: context.workspace_identity,
            language_id: context.language_id,
            surface: context.surface,
            canonical_request_digest: context.canonical_request_digest,
            requested_projection: context.requested_projection,
            stage: context.stage,
            reason_kind,
            runtime_artifact_digest: context.runtime_artifact_digest,
            provider_contract_digest: context.provider_contract_digest,
            generation_digest: context.generation_digest,
        },
        succeeded,
        blocked,
        budget_exceeded,
        code_projection_present,
        budget_micros: context.budget_micros,
        elapsed_micros: context.elapsed_micros,
        observed_at_unix_micros: context.observed_at_unix_micros,
        resources: context.resources,
    }
}

fn incident_id(context: &SearchIncidentTerminalContext, reason_kind: &str) -> String {
    let identity = serde_json::to_vec(&(
        &context.workspace_identity,
        &context.language_id,
        context.surface,
        &context.canonical_request_digest,
        context.requested_projection,
        &context.stage,
        reason_kind,
        &context.runtime_artifact_digest,
        &context.provider_contract_digest,
        &context.generation_digest,
    ))
    .expect("search incident identity fields must serialize");
    format!("blake3-256:{}", blake3::hash(&identity).to_hex())
}
