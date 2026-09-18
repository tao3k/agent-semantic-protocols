// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Test-only Live Corpus operations on the shared Runtime ClientFrame session.

use super::AspClient;
use super::SessionKey;
use super::project_workspace_ids;
use super::session_registry;
use agent_semantic_client_protocol::ClientFrame;
use agent_semantic_client_protocol::LIVE_CORPUS_CACHE_STATE_METHOD;
use agent_semantic_client_protocol::LIVE_CORPUS_MERKLE_OWNER_READ_METHOD;
use agent_semantic_client_protocol::LiveCorpusCacheStateReceipt;
use agent_semantic_client_protocol::LiveCorpusCacheStateRequest;

impl AspClient {
    /// Prepare one explicit Live Corpus cache state through the Runtime-owned
    /// cache authority. Cold-load additionally evicts only this workspace's
    /// client session after the server confirms the exact generation/root.
    pub async fn prepare_live_corpus_cache_state(
        &self,
        request: LiveCorpusCacheStateRequest,
    ) -> Result<LiveCorpusCacheStateReceipt, String> {
        request.validate()?;
        let publication = self.runtime_handoff().await?;
        let (project_id, workspace_id) = project_workspace_ids(&self.project_root)?;
        let session_key = SessionKey::from_publication(&publication, project_id, workspace_id);
        let cache_state = request.cache_state.clone();
        let frame = self
            .dispatch_method(
                LIVE_CORPUS_CACHE_STATE_METHOD.to_owned(),
                serde_json::to_value(request)
                    .map_err(|error| format!("encode Live Corpus cache-state request: {error}"))?,
            )
            .await?;
        let ClientFrame::Response {
            outcome: agent_semantic_client_protocol::ClientOutcome::Ready,
            result: Some(payload),
            error: None,
            ..
        } = frame
        else {
            return Err(format!(
                "Live Corpus cache-state request did not return Ready: {frame:?}"
            ));
        };
        let mut receipt =
            serde_json::from_value::<LiveCorpusCacheStateReceipt>(payload.into_value())
                .map_err(|error| format!("decode Live Corpus cache-state receipt: {error}"))?;
        receipt.validate()?;
        if receipt.project_id != session_key.project_id
            || receipt.workspace_id != session_key.workspace_id
        {
            return Err(
                "Live Corpus cache-state receipt crossed its ProjectId/WorkspaceId binding"
                    .to_owned(),
            );
        }
        if matches!(cache_state.as_str(), "cold-load" | "released") {
            receipt.client_session_evicted =
                session_registry().lock().await.remove_key(&session_key);
            if !receipt.client_session_evicted {
                return Err(format!(
                    "Live Corpus {cache_state} did not evict its exact client session"
                ));
            }
        }
        receipt.validate()?;
        Ok(receipt)
    }

    /// Read one resident Merkle owner proof through the same ClientFrame
    /// session used by Live Corpus Search and Query qualification.
    pub async fn read_live_corpus_merkle_owner(
        &self,
        owner_paths: &[String],
        case_id: &str,
        resource_id: &str,
        language_id: &str,
        provider_id: &str,
    ) -> Result<agent_semantic_client_db::runtime_merkle_owner_proof_qualification::RuntimeMerkleOwnerProofQualificationReceipt, String>{
        let canonical_project_root =
            agent_semantic_client_core::state_core::ResolvedState::resolve_with_state_home(
                &self.project_root,
                &self.state_home,
            )?
            .workspace
            .root;
        let request = agent_semantic_client_db::runtime_merkle_owner_proof_qualification::RuntimeMerkleOwnerProofQualificationRequest {
            schema_id: agent_semantic_client_db::runtime_merkle_owner_proof_qualification::RUNTIME_MERKLE_OWNER_PROOF_QUALIFICATION_REQUEST_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            project_root: canonical_project_root.display().to_string(),
            owner_paths: owner_paths.to_vec(),
            case_id: case_id.to_owned(),
            resource_id: resource_id.to_owned(),
            language_id: language_id.to_owned(),
            provider_id: provider_id.to_owned(),
        };
        request.validate()?;
        let frame = self
            .dispatch_method(
                LIVE_CORPUS_MERKLE_OWNER_READ_METHOD.to_owned(),
                serde_json::to_value(request)
                    .map_err(|error| format!("encode Live Corpus Merkle owner request: {error}"))?,
            )
            .await?;
        let ClientFrame::Response {
            outcome: agent_semantic_client_protocol::ClientOutcome::Ready,
            result: Some(payload),
            error: None,
            ..
        } = frame
        else {
            return Err(format!(
                "Live Corpus Merkle owner request did not return Ready: {frame:?}"
            ));
        };
        serde_json::from_value(payload.into_value())
            .map_err(|error| format!("decode Live Corpus Merkle proof receipt: {error}"))
    }
}
