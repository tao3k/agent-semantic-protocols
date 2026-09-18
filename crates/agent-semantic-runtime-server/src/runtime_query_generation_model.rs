// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Generation state and terminal values shared by Runtime Query owners.

use std::collections::BTreeSet;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient;

use super::query_generation_calibration::RuntimeSearchGenerationBuildResourceReceipt;

pub struct RuntimeQueryGeneration {
    pub(super) generation_digest: String,
    pub(super) generation_token: AtomicU64,
    pub(super) resident: Option<Arc<RuntimeResidentReadClient>>,
    pub(super) resource_supervisor:
        agent_semantic_workspace_scheduler::RuntimeServerResourceSupervisor,
    pub(super) task_scope: agent_semantic_workspace_scheduler::RuntimeServerTaskScope,
    pub(super) execution_publication: Option<
        Arc<
            agent_semantic_content_identity::runtime_workspace_execution_publication::RuntimeWorkspaceExecutionPublication,
        >,
    >,
    pub(super) project_topology_attachment: Mutex<Option<RuntimeProjectTopologyCacheEntry>>,
    pub(super) project_topology_build_lock: tokio::sync::Mutex<()>,
    pub(super) resident_syntax_scope_evidence: Mutex<Option<RuntimeSyntaxScopeEvidence>>,
    pub(super) lexical_attachment_completion: tokio::sync::watch::Sender<bool>,
    pub(super) build_resource_receipt:
        std::sync::OnceLock<RuntimeSearchGenerationBuildResourceReceipt>,
    pub(super) search_materializations: Arc<Mutex<
        std::collections::HashMap<String, RuntimeSearchMaterializationState>,
    >>,
    pub(super) query_materializations: Arc<Mutex<
        std::collections::HashMap<String, RuntimeQueryMaterializationState>,
    >>,
    pub(super) materialization_tasks: Arc<Mutex<tokio::task::JoinSet<()>>>,
}

#[derive(Clone)]
pub(super) struct RuntimeProjectTopologyCacheEntry {
    topology_source_generation_digest: String,
    owner_scope: BTreeSet<String>,
    selector_scope: Option<BTreeSet<String>>,
    pub(super) attachment:
        Result<Arc<agent_semantic_topology::RuntimeProjectTopologyAttachment>, Arc<str>>,
    _memory_permit: Option<Arc<agent_semantic_workspace_scheduler::RuntimeServerResourcePermit>>,
}

impl RuntimeProjectTopologyCacheEntry {
    pub(super) fn new(
        topology_source_generation_digest: String,
        owner_scope: BTreeSet<String>,
        selector_scope: Option<BTreeSet<String>>,
        attachment: Result<
            Arc<agent_semantic_topology::RuntimeProjectTopologyAttachment>,
            Arc<str>,
        >,
        memory_permit: Option<agent_semantic_workspace_scheduler::RuntimeServerResourcePermit>,
    ) -> Self {
        Self {
            topology_source_generation_digest,
            owner_scope,
            selector_scope,
            attachment,
            _memory_permit: memory_permit.map(Arc::new),
        }
    }

    pub(super) fn matches(
        &self,
        topology_source_generation_digest: &str,
        requested: &BTreeSet<String>,
        requested_selectors: Option<&BTreeSet<String>>,
    ) -> bool {
        self.topology_source_generation_digest == topology_source_generation_digest
            && self.owner_scope == *requested
            && self.selector_scope.as_ref() == requested_selectors
    }

    pub(super) fn matches_request(
        &self,
        topology_source_generation_digest: &str,
        requested: &BTreeSet<String>,
        requested_selectors: Option<&BTreeSet<String>>,
    ) -> bool {
        self.matches(
            topology_source_generation_digest,
            requested,
            requested_selectors,
        ) || (requested.is_empty() && self.attachment.is_ok())
    }
}

#[derive(Clone)]
pub(crate) enum RuntimeSearchMaterializationState {
    Building(Arc<tokio::sync::watch::Sender<Option<RuntimeSearchTerminalState>>>),
    Ready(Arc<agent_semantic_client_protocol::ClientResponsePayload>),
    Failed(Arc<agent_semantic_client_server::AspClientDispatchError>),
}

#[derive(Clone)]
pub(crate) enum RuntimeSearchTerminalState {
    Ready(Arc<agent_semantic_client_protocol::ClientResponsePayload>),
    Failed(Arc<agent_semantic_client_server::AspClientDispatchError>),
}

impl RuntimeSearchTerminalState {
    pub(super) fn materialization_state(self) -> RuntimeSearchMaterializationState {
        match self {
            Self::Ready(value) => RuntimeSearchMaterializationState::Ready(value),
            Self::Failed(error) => RuntimeSearchMaterializationState::Failed(error),
        }
    }
}

#[derive(Clone)]
pub(crate) enum RuntimeQueryMaterializationState {
    Building(Arc<tokio::sync::watch::Sender<Option<RuntimeQueryTerminalState>>>),
    Ready(Arc<serde_json::Value>),
    Failed(Arc<agent_semantic_client_server::AspClientDispatchError>),
}

#[derive(Clone)]
pub(crate) enum RuntimeQueryTerminalState {
    Ready(Arc<serde_json::Value>),
    Failed(Arc<agent_semantic_client_server::AspClientDispatchError>),
}

impl RuntimeQueryTerminalState {
    pub(super) fn materialization_state(self) -> RuntimeQueryMaterializationState {
        match self {
            Self::Ready(value) => RuntimeQueryMaterializationState::Ready(value),
            Self::Failed(error) => RuntimeQueryMaterializationState::Failed(error),
        }
    }
}

pub(super) struct RuntimeSyntaxScopeEvidence {
    pub(super) plan_digest: String,
    pub(super) owner_scope: BTreeSet<String>,
    pub(super) evidence:
        Arc<Vec<agent_semantic_client_protocol::AspClientWorkspaceSyntaxQueryEvidence>>,
}
