// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Typed terminal receipts for Runtime-owned workspace generation admission.

use super::{
    WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID, WorkspaceGenerationAdmissionMode,
    WorkspaceGenerationAdmissionReceipt, WorkspaceGenerationAdmissionState,
    WorkspaceGenerationAdmissionTrigger, WorkspaceGenerationBuildCompletion,
    WorkspaceGenerationBuildFailure, WorkspaceGenerationCandidateIdentity,
    WorkspaceGenerationFailureStage,
};

pub(super) fn from_build_result(
    result: Result<WorkspaceGenerationBuildCompletion, WorkspaceGenerationBuildFailure>,
    workspace_identity: &str,
    expected: &WorkspaceGenerationCandidateIdentity,
    trigger: WorkspaceGenerationAdmissionTrigger,
    admission_mode: WorkspaceGenerationAdmissionMode,
    attempt: u64,
) -> WorkspaceGenerationAdmissionReceipt {
    match result {
        Ok(completion) if completion.candidate != *expected => binding_mismatch(
            workspace_identity,
            expected,
            completion.candidate,
            trigger,
            admission_mode,
            attempt,
        ),
        Ok(completion) => WorkspaceGenerationAdmissionReceipt {
            commit: Some(completion.commit),
            schema_id: WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: workspace_identity.to_owned(),
            trigger,
            admission_mode,
            build_owner: "runtime-server".to_owned(),
            cancellation_authority: "runtime-server".to_owned(),
            request_lifetime_independent: true,
            candidate_generation: completion.candidate.candidate_generation,
            policy_overlay_digest: completion.candidate.policy_overlay_digest,
            state: WorkspaceGenerationAdmissionState::Ready,
            accepted: false,
            attempt,
            failure_stage: None,
            error: None,
        },
        Err(error) => WorkspaceGenerationAdmissionReceipt {
            schema_id: WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: workspace_identity.to_owned(),
            trigger,
            admission_mode,
            build_owner: "runtime-server".to_owned(),
            cancellation_authority: "runtime-server".to_owned(),
            request_lifetime_independent: true,
            candidate_generation: expected.candidate_generation.clone(),
            policy_overlay_digest: expected.policy_overlay_digest.clone(),
            state: WorkspaceGenerationAdmissionState::Failed,
            accepted: false,
            attempt,
            commit: None,
            failure_stage: Some(error.stage),
            error: Some(error.message),
        },
    }
}

fn binding_mismatch(
    workspace_identity: &str,
    expected: &WorkspaceGenerationCandidateIdentity,
    observed: WorkspaceGenerationCandidateIdentity,
    trigger: WorkspaceGenerationAdmissionTrigger,
    admission_mode: WorkspaceGenerationAdmissionMode,
    attempt: u64,
) -> WorkspaceGenerationAdmissionReceipt {
    WorkspaceGenerationAdmissionReceipt {
        schema_id: WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        workspace_identity: workspace_identity.to_owned(),
        trigger,
        admission_mode,
        build_owner: "runtime-server".to_owned(),
        cancellation_authority: "runtime-server".to_owned(),
        request_lifetime_independent: true,
        candidate_generation: expected.candidate_generation.clone(),
        policy_overlay_digest: expected.policy_overlay_digest.clone(),
        state: WorkspaceGenerationAdmissionState::Failed,
        accepted: false,
        attempt,
        commit: None,
        failure_stage: Some(WorkspaceGenerationFailureStage::GenerationBuilder),
        error: Some(
            serde_json::json!({
                "schemaId": "agent.semantic-protocols.workspace-generation-admission-binding-mismatch",
                "schemaVersion": "1",
                "reasonKind": "workspace-generation-admission-binding-mismatch",
                "workspaceIdentity": workspace_identity,
                "expectedCandidateGeneration": expected.candidate_generation,
                "observedCandidateGeneration": observed.candidate_generation,
                "expectedPolicyOverlayDigest": expected.policy_overlay_digest,
                "observedPolicyOverlayDigest": observed.policy_overlay_digest,
            })
            .to_string(),
        ),
    }
}
