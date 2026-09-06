// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use super::model::{
    IncidentObservation, IncidentRecord, IncidentState, ReplayReceipt, TransitionError,
};

pub fn should_record(observation: &IncidentObservation) -> bool {
    !observation.succeeded
        || observation.blocked
        || observation.budget_exceeded
        || (observation.identity.requested_projection.requires_code()
            && !observation.code_projection_present)
}

pub fn observe(
    existing: Option<IncidentRecord>,
    observation: IncidentObservation,
) -> Result<Option<IncidentRecord>, TransitionError> {
    if !should_record(&observation) {
        return Ok(None);
    }

    let record = match existing {
        Some(mut record) => {
            if record.identity != observation.identity {
                return Err(TransitionError::IdentityMismatch);
            }
            record.state = IncidentState::Open;
            record.occurrence_count = record.occurrence_count.saturating_add(1);
            record.transition_sequence = record.transition_sequence.saturating_add(1);
            record.last_seen_unix_micros = observation.observed_at_unix_micros;
            record.code_projection_present = observation.code_projection_present;
            record.budget_micros = observation.budget_micros;
            record.elapsed_micros = observation.elapsed_micros;
            record.resources = observation.resources;
            record.repair_artifact_digest = None;
            record.verification_receipt = None;
            record
        }
        None => IncidentRecord {
            identity: observation.identity,
            state: IncidentState::Open,
            occurrence_count: 1,
            transition_sequence: 1,
            first_seen_unix_micros: observation.observed_at_unix_micros,
            last_seen_unix_micros: observation.observed_at_unix_micros,
            code_projection_present: observation.code_projection_present,
            budget_micros: observation.budget_micros,
            elapsed_micros: observation.elapsed_micros,
            repair_artifact_digest: None,
            verification_receipt: None,
            resources: observation.resources,
        },
    };
    Ok(Some(record))
}

pub fn begin_repair(
    record: &mut IncidentRecord,
    repair_artifact_digest: String,
) -> Result<(), TransitionError> {
    if record.state != IncidentState::Open {
        return Err(TransitionError::InvalidState);
    }
    record.state = IncidentState::Repairing;
    record.transition_sequence = record.transition_sequence.saturating_add(1);
    record.repair_artifact_digest = Some(repair_artifact_digest);
    Ok(())
}

pub fn request_verification(record: &mut IncidentRecord) -> Result<(), TransitionError> {
    if record.state != IncidentState::Repairing || record.repair_artifact_digest.is_none() {
        return Err(TransitionError::InvalidState);
    }
    record.state = IncidentState::VerificationPending;
    record.transition_sequence = record.transition_sequence.saturating_add(1);
    Ok(())
}

pub fn apply_replay(
    record: &mut IncidentRecord,
    receipt: ReplayReceipt,
) -> Result<(), TransitionError> {
    if record.state != IncidentState::VerificationPending {
        return Err(TransitionError::InvalidState);
    }
    let matches = receipt.canonical_request_digest == record.identity.canonical_request_digest
        && receipt.workspace_identity == record.identity.workspace_identity
        && Some(receipt.runtime_artifact_digest.as_str())
            == record.repair_artifact_digest.as_deref()
        && record.identity.generation_digest.as_deref() == Some(receipt.generation_digest.as_str())
        && receipt.code_projection_present
        && receipt.within_budget;
    record.verification_receipt = Some(receipt);
    record.state = if matches {
        IncidentState::Resolved
    } else {
        IncidentState::FailedVerification
    };
    record.transition_sequence = record.transition_sequence.saturating_add(1);
    if matches {
        Ok(())
    } else {
        Err(TransitionError::ReplayMismatch)
    }
}

pub fn reopen_failed_verification(record: &mut IncidentRecord) -> Result<(), TransitionError> {
    if record.state != IncidentState::FailedVerification {
        return Err(TransitionError::InvalidState);
    }
    record.state = IncidentState::Open;
    record.transition_sequence = record.transition_sequence.saturating_add(1);
    record.repair_artifact_digest = None;
    record.verification_receipt = None;
    Ok(())
}

pub fn supersede(record: &mut IncidentRecord) {
    record.state = IncidentState::Superseded;
    record.transition_sequence = record.transition_sequence.saturating_add(1);
    record.verification_receipt = None;
}

pub fn compact(record: &mut IncidentRecord) -> Result<(), TransitionError> {
    if record.state != IncidentState::Resolved {
        return Err(TransitionError::InvalidState);
    }
    let verified = record
        .verification_receipt
        .as_ref()
        .is_some_and(|receipt| receipt.code_projection_present && receipt.within_budget);
    if !verified {
        return Err(TransitionError::VerificationRequired);
    }
    record.state = IncidentState::Compacted;
    record.transition_sequence = record.transition_sequence.saturating_add(1);
    Ok(())
}
