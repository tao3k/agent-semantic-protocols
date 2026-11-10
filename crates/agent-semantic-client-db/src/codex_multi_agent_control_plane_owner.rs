// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::HashMap;
use std::sync::Arc;

use agent_semantic_context_product::agent_session_lifecycle::WorkspaceServerProjection;
use agent_semantic_context_product::codex_multi_agent_v2_control_plane::{
    CODEX_MULTI_AGENT_V2_CONTROL_PLANE_SCHEMA_ID,
    CODEX_MULTI_AGENT_V2_CONTROL_PLANE_SCHEMA_VERSION, CodexMultiAgentV2ControlPlaneProjection,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

pub const CODEX_CONTROL_PLANE_PUBLICATION_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.codex-multi-agent-v2-control-plane-publication-receipt";
pub const CODEX_CONTROL_PLANE_PUBLICATION_RECEIPT_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexControlPlanePublicationReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_identity: String,
    pub root_session_id: String,
    pub generation: u64,
    pub source_digest: Option<String>,
    pub idempotent: bool,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct CodexControlPlaneKey {
    workspace_identity: String,
    root_session_id: String,
}

#[derive(Default)]
pub struct CodexMultiAgentControlPlaneOwner {
    snapshots: RwLock<HashMap<CodexControlPlaneKey, Arc<CodexMultiAgentV2ControlPlaneProjection>>>,
}

impl CodexMultiAgentControlPlaneOwner {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn publish(
        &self,
        projection: CodexMultiAgentV2ControlPlaneProjection,
    ) -> Result<CodexControlPlanePublicationReceipt, String> {
        if projection.schema_id != CODEX_MULTI_AGENT_V2_CONTROL_PLANE_SCHEMA_ID {
            return Err(format!(
                "Codex control-plane schema id must be {CODEX_MULTI_AGENT_V2_CONTROL_PLANE_SCHEMA_ID:?}, got {:?}",
                projection.schema_id
            ));
        }
        if projection.schema_version != CODEX_MULTI_AGENT_V2_CONTROL_PLANE_SCHEMA_VERSION {
            return Err(format!(
                "Codex control-plane schema version must be {CODEX_MULTI_AGENT_V2_CONTROL_PLANE_SCHEMA_VERSION:?}, got {:?}",
                projection.schema_version
            ));
        }
        projection.validate()?;

        let key = CodexControlPlaneKey {
            workspace_identity: projection.workspace_server.workspace_identity.clone(),
            root_session_id: projection.root_session_id.clone(),
        };
        let mut snapshots = self.snapshots.write().await;
        let admission = match snapshots.get(&key) {
            Some(current) => current.publication_admission(&projection)?,
            None => agent_semantic_context_product::codex_multi_agent_v2_control_plane::CodexControlPlanePublicationAdmission::Advance,
        };
        let idempotent = admission
            == agent_semantic_context_product::codex_multi_agent_v2_control_plane::CodexControlPlanePublicationAdmission::Idempotent;
        if !idempotent {
            snapshots.insert(key, Arc::new(projection.clone()));
        }
        Ok(CodexControlPlanePublicationReceipt {
            schema_id: CODEX_CONTROL_PLANE_PUBLICATION_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: CODEX_CONTROL_PLANE_PUBLICATION_RECEIPT_SCHEMA_VERSION.to_owned(),
            workspace_identity: projection.workspace_server.workspace_identity,
            root_session_id: projection.root_session_id,
            generation: projection.materialization.generation,
            source_digest: projection.materialization.source_digest,
            idempotent,
        })
    }

    pub async fn read(
        &self,
        workspace_identity: &str,
        root_session_id: &str,
    ) -> Option<Arc<CodexMultiAgentV2ControlPlaneProjection>> {
        self.snapshots
            .read()
            .await
            .get(&CodexControlPlaneKey {
                workspace_identity: workspace_identity.to_owned(),
                root_session_id: root_session_id.to_owned(),
            })
            .cloned()
    }

    pub async fn mark_workspace_server_observation(
        &self,
        server: WorkspaceServerProjection,
    ) -> usize {
        let mut snapshots = self.snapshots.write().await;
        snapshots
            .iter_mut()
            .filter(|(key, projection)| {
                key.workspace_identity == server.workspace_identity
                    && projection.workspace_server != server
            })
            .map(|(_, projection)| {
                *projection = Arc::new(
                    projection
                        .as_ref()
                        .clone()
                        .with_workspace_server(server.clone()),
                );
            })
            .count()
    }

    pub async fn downstream_dispatch_authorized(
        &self,
        workspace_identity: &str,
        root_session_id: &str,
        target_session_id: &str,
    ) -> bool {
        self.read(workspace_identity, root_session_id)
            .await
            .is_some_and(|projection| projection.downstream_dispatch_authorized(target_session_id))
    }
}
