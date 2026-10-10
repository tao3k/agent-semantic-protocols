// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Lock-free preparation for the Runtime artifact activation transaction.

use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use crate::blake3_content_digest::Blake3ContentDigest;
use crate::runtime_artifact_activation::RuntimeArtifactActivationEvent;
use crate::runtime_artifact_activation::RuntimeArtifactCandidateIdentityReceipt;
use crate::runtime_artifact_activation::stage_pending_runtime_artifact_activation;
use crate::runtime_artifact_quiescence::PreparedRuntimeArtifactQuiescenceLease;
use crate::runtime_artifact_quiescence::stage_runtime_artifact_quiescence_lease;

pub(super) struct RuntimeArtifactActivationStagingRequest<'a> {
    pub state_home: &'a Path,
    pub activation_event_path: &'a Path,
    pub candidate_dir: &'a Path,
    pub artifact_path: &'a Path,
    pub stable_path: &'a Path,
    pub artifact_mode: &'a str,
    pub quiescence_operation: &'a str,
    pub activation_generation: u64,
    pub bundle_digest: &'a Blake3ContentDigest,
    pub artifact_digest: &'a Blake3ContentDigest,
    pub previous_artifact_digest: Option<Blake3ContentDigest>,
}

pub(super) struct StagedRuntimeArtifactActivation {
    pub event: RuntimeArtifactActivationEvent,
    pub pending_path: PathBuf,
    pub quiescence: PreparedRuntimeArtifactQuiescenceLease,
}

pub(super) fn stage_runtime_artifact_activation_transaction(
    request: RuntimeArtifactActivationStagingRequest<'_>,
) -> Result<StagedRuntimeArtifactActivation, String> {
    let quiescence = stage_runtime_artifact_quiescence_lease(
        request.state_home,
        request.quiescence_operation,
        request.artifact_digest,
    )?;
    let publication_nonce = quiescence.lease.lease_nonce.clone();
    let published_at_unix_millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("Runtime artifact publication clock failed: {error}"))?
        .as_millis();
    let event = RuntimeArtifactActivationEvent {
        schema_id: "agent.semantic-protocols.runtime-artifact-activation".to_owned(),
        schema_version: 1,
        activation_generation: request.activation_generation,
        bundle_digest: request.bundle_digest.clone(),
        artifact_digest: request.artifact_digest.clone(),
        artifact_path: request.artifact_path.to_path_buf(),
        candidate_slot_path: request.candidate_dir.to_path_buf(),
        previous_artifact_digest: request.previous_artifact_digest,
        artifact_mode: request.artifact_mode.to_owned(),
        published_at_unix_millis,
        publication_nonce: publication_nonce.clone(),
        candidate_identity: RuntimeArtifactCandidateIdentityReceipt {
            artifact_digest: request.artifact_digest.clone(),
            artifact_path: request.artifact_path.to_path_buf(),
            stable_path: request.stable_path.to_path_buf(),
            artifact_mode: request.artifact_mode.to_owned(),
            publication_nonce,
        },
    };
    let event_bytes = serde_json::to_vec_pretty(&event)
        .map_err(|error| format!("encode Runtime artifact activation event: {error}"))?;
    std::fs::write(request.candidate_dir.join("activation.json"), &event_bytes)
        .map_err(|error| format!("stage Runtime artifact activation event: {error}"))?;
    let pending_path = stage_pending_runtime_artifact_activation(
        request.activation_event_path,
        &event_bytes,
        &event.publication_nonce,
    )?;
    Ok(StagedRuntimeArtifactActivation {
        event,
        pending_path,
        quiescence,
    })
}
