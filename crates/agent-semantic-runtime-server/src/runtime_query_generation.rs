// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Immutable Runtime Query generation with independently settled Search attachments.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient;

use super::query_generation_calibration::RuntimeSearchGenerationBuildResourceReceipt;

/// Immutable Runtime handle for one admitted resident Query generation.
///
/// Tantivy and the lightweight Search-core Project Topology are generation-scoped
/// attachments. Request-local parser and Relation enrichment remains a lazy
/// overlay; none of its failures can revoke exact native Query reads from the base.
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
    attachment: Result<Arc<agent_semantic_topology::RuntimeProjectTopologyAttachment>, Arc<str>>,
    _memory_permit: Option<Arc<agent_semantic_workspace_scheduler::RuntimeServerResourcePermit>>,
}

impl RuntimeProjectTopologyCacheEntry {
    pub(super) fn new(
        topology_source_generation_digest: String,
        owner_scope: BTreeSet<String>,
        attachment: Result<
            Arc<agent_semantic_topology::RuntimeProjectTopologyAttachment>,
            Arc<str>,
        >,
        memory_permit: Option<agent_semantic_workspace_scheduler::RuntimeServerResourcePermit>,
    ) -> Self {
        Self {
            topology_source_generation_digest,
            owner_scope,
            attachment,
            _memory_permit: memory_permit.map(Arc::new),
        }
    }

    pub(super) fn matches(
        &self,
        topology_source_generation_digest: &str,
        requested: &BTreeSet<String>,
    ) -> bool {
        self.topology_source_generation_digest == topology_source_generation_digest
            && self.owner_scope == *requested
    }
}

#[derive(Clone)]
pub(crate) enum RuntimeSearchMaterializationState {
    Building(Arc<tokio::sync::watch::Sender<Option<RuntimeSearchTerminalState>>>),
    Ready(Arc<serde_json::Value>),
    Failed(Arc<agent_semantic_client_server::AspClientDispatchError>),
}

#[derive(Clone)]
pub(crate) enum RuntimeSearchTerminalState {
    Ready(Arc<serde_json::Value>),
    Failed(Arc<agent_semantic_client_server::AspClientDispatchError>),
}

impl RuntimeSearchTerminalState {
    fn materialization_state(self) -> RuntimeSearchMaterializationState {
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
    fn materialization_state(self) -> RuntimeQueryMaterializationState {
        match self {
            Self::Ready(value) => RuntimeQueryMaterializationState::Ready(value),
            Self::Failed(error) => RuntimeQueryMaterializationState::Failed(error),
        }
    }
}

pub(super) struct RuntimeSyntaxScopeEvidence {
    plan_digest: String,
    owner_scope: BTreeSet<String>,
    evidence: Arc<Vec<agent_semantic_client_protocol::AspClientWorkspaceSyntaxQueryEvidence>>,
}

impl RuntimeQueryGeneration {
    pub(super) fn inherit_materialization_authorities(&mut self, previous: &Self) {
        if previous.generation_digest == self.generation_digest {
            self.search_materializations = Arc::clone(&previous.search_materializations);
            self.query_materializations = Arc::clone(&previous.query_materializations);
            self.materialization_tasks = Arc::clone(&previous.materialization_tasks);
        }
    }

    pub(crate) fn contains_provider_targets(
        &self,
        targets: &[agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationProviderTarget],
    ) -> bool {
        targets.is_empty()
            || self.resident.as_ref().is_some_and(|resident| {
                targets.iter().all(|target| {
                    resident.contains_provider_authority(
                        &target.language_id,
                        target.provider_id.as_deref(),
                    )
                })
            })
    }

    /// Opens the resident Search authority and derives its fusion capabilities.
    pub async fn open(
        pointer_path: &std::path::Path,
        project_root: &std::path::Path,
        resource_supervisor: agent_semantic_workspace_scheduler::RuntimeServerResourceSupervisor,
        task_scope: agent_semantic_workspace_scheduler::RuntimeServerTaskScope,
    ) -> Result<Self, String> {
        let resident = RuntimeResidentReadClient::open(pointer_path, project_root).await?;
        let generation_digest = resident.generation_digest();
        resident
            .search_generation_authority()
            .content_search_generation
            .validate()?;
        Ok(Self {
            generation_digest,
            generation_token: AtomicU64::new(0),
            resident: Some(Arc::new(resident)),
            resource_supervisor,
            task_scope,
            execution_publication: None,
            project_topology_attachment: Mutex::new(None),
            project_topology_build_lock: tokio::sync::Mutex::new(()),
            resident_syntax_scope_evidence: Mutex::new(None),
            lexical_attachment_completion: tokio::sync::watch::channel(false).0,
            build_resource_receipt: std::sync::OnceLock::new(),
            search_materializations: Arc::new(Mutex::new(std::collections::HashMap::new())),
            query_materializations: Arc::new(Mutex::new(std::collections::HashMap::new())),
            materialization_tasks: Arc::new(Mutex::new(tokio::task::JoinSet::new())),
        })
    }

    /// Reopens one durable generation together with its exact content-bound
    /// Runtime execution publication.
    pub async fn open_with_execution_publication(
        pointer_path: &std::path::Path,
        project_root: &std::path::Path,
        execution_publication: agent_semantic_content_identity::runtime_workspace_execution_publication::RuntimeWorkspaceExecutionPublication,
        resource_supervisor: agent_semantic_workspace_scheduler::RuntimeServerResourceSupervisor,
        task_scope: agent_semantic_workspace_scheduler::RuntimeServerTaskScope,
    ) -> Result<Self, String> {
        let resident = RuntimeResidentReadClient::open(pointer_path, project_root).await?;
        Self::from_resident_with_execution_publication(
            resident,
            execution_publication,
            resource_supervisor,
            task_scope,
        )
    }

    pub fn from_resident(
        resident: agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient,
        resource_supervisor: agent_semantic_workspace_scheduler::RuntimeServerResourceSupervisor,
        task_scope: agent_semantic_workspace_scheduler::RuntimeServerTaskScope,
    ) -> Result<Self, String> {
        let generation_digest = resident.generation_digest();
        resident
            .search_generation_authority()
            .content_search_generation
            .validate()?;
        Ok(Self {
            generation_digest,
            generation_token: AtomicU64::new(0),
            resident: Some(Arc::new(resident)),
            resource_supervisor,
            task_scope,
            execution_publication: None,
            project_topology_attachment: Mutex::new(None),
            project_topology_build_lock: tokio::sync::Mutex::new(()),
            resident_syntax_scope_evidence: Mutex::new(None),
            lexical_attachment_completion: tokio::sync::watch::channel(false).0,
            build_resource_receipt: std::sync::OnceLock::new(),
            search_materializations: Arc::new(Mutex::new(std::collections::HashMap::new())),
            query_materializations: Arc::new(Mutex::new(std::collections::HashMap::new())),
            materialization_tasks: Arc::new(Mutex::new(tokio::task::JoinSet::new())),
        })
    }

    pub(crate) fn spawn_materialization<F>(
        &self,
        name: &'static str,
        future: F,
    ) -> Result<(), String>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let permit = self.task_scope.permit(name)?;
        let mut tasks = self
            .materialization_tasks
            .lock()
            .map_err(|_| "Query materialization task registry is poisoned".to_owned())?;
        while tasks.try_join_next().is_some() {}
        tasks.spawn(async move {
            future.await;
            permit.complete();
        });
        Ok(())
    }

    pub(crate) fn search_materialization(
        &self,
        key: &str,
    ) -> Result<Option<RuntimeSearchMaterializationState>, String> {
        Ok(self
            .search_materializations
            .lock()
            .map_err(|_| "Runtime Search materialization cache poisoned".to_owned())?
            .get(key)
            .cloned())
    }

    /// Atomically claim one generation-bound Search materialization.
    pub(crate) fn begin_search_materialization(&self, key: String) -> Result<bool, String> {
        let mut materializations = self
            .search_materializations
            .lock()
            .map_err(|_| "Runtime Search materialization cache poisoned".to_owned())?;
        if materializations.contains_key(&key) {
            return Ok(false);
        }
        materializations.insert(
            key,
            RuntimeSearchMaterializationState::Building(Arc::new(
                tokio::sync::watch::channel(None).0,
            )),
        );
        Ok(true)
    }

    pub(crate) fn publish_search_materialization(
        &self,
        key: String,
        result: Result<serde_json::Value, agent_semantic_client_server::AspClientDispatchError>,
    ) -> Result<(), String> {
        let mut materializations = self
            .search_materializations
            .lock()
            .map_err(|_| "Runtime Search materialization cache poisoned".to_owned())?;
        let Some(RuntimeSearchMaterializationState::Building(completion)) =
            materializations.get(&key)
        else {
            return Err("Runtime Search materialization lost its Building claim".to_owned());
        };
        let completion = Arc::clone(completion);
        let terminal = match result {
            Ok(value) => RuntimeSearchTerminalState::Ready(Arc::new(value)),
            Err(error) => RuntimeSearchTerminalState::Failed(Arc::new(error)),
        };
        materializations.remove(&key);
        let terminal_capacity = self.resource_supervisor.effective_cpu().max(2);
        while materializations
            .values()
            .filter(|state| !matches!(state, RuntimeSearchMaterializationState::Building(_)))
            .count()
            >= terminal_capacity
        {
            let evicted = materializations
                .iter()
                .filter(|(_, state)| {
                    !matches!(state, RuntimeSearchMaterializationState::Building(_))
                })
                .map(|(key, _)| key)
                .min()
                .cloned();
            let Some(evicted) = evicted else { break };
            materializations.remove(&evicted);
        }
        materializations.insert(key, terminal.clone().materialization_state());
        completion.send_replace(Some(terminal));
        Ok(())
    }

    pub(crate) async fn await_search_materialization(
        &self,
        key: &str,
    ) -> Result<RuntimeSearchTerminalState, String> {
        match self.search_materialization(key)? {
            Some(RuntimeSearchMaterializationState::Building(completion)) => {
                let mut receiver = completion.subscribe();
                if receiver.borrow_and_update().is_none() {
                    receiver
                        .changed()
                        .await
                        .map_err(|_| "Search completion channel closed".to_owned())?;
                }
                receiver
                    .borrow()
                    .clone()
                    .ok_or_else(|| "Search completion did not publish a terminal".to_owned())
            }
            Some(RuntimeSearchMaterializationState::Ready(value)) => {
                Ok(RuntimeSearchTerminalState::Ready(value))
            }
            Some(RuntimeSearchMaterializationState::Failed(error)) => {
                Ok(RuntimeSearchTerminalState::Failed(error))
            }
            None => Err("Search materialization has no claim".to_owned()),
        }
    }

    pub(crate) fn query_materialization(
        &self,
        key: &str,
    ) -> Result<Option<RuntimeQueryMaterializationState>, String> {
        Ok(self
            .query_materializations
            .lock()
            .map_err(|_| "Runtime Query materialization cache poisoned".to_owned())?
            .get(key)
            .cloned())
    }

    /// Atomically claim one generation-bound Query materialization.
    pub(crate) fn begin_query_materialization(&self, key: String) -> Result<bool, String> {
        let mut materializations = self
            .query_materializations
            .lock()
            .map_err(|_| "Runtime Query materialization cache poisoned".to_owned())?;
        if materializations.contains_key(&key) {
            return Ok(false);
        }
        materializations.insert(
            key,
            RuntimeQueryMaterializationState::Building(Arc::new(
                tokio::sync::watch::channel(None).0,
            )),
        );
        Ok(true)
    }

    pub(crate) fn publish_query_materialization(
        &self,
        key: String,
        result: Result<serde_json::Value, agent_semantic_client_server::AspClientDispatchError>,
    ) -> Result<(), String> {
        let terminal_slot = query_materialization_slot(&key)?;
        let mut materializations = self
            .query_materializations
            .lock()
            .map_err(|_| "Runtime Query materialization cache poisoned".to_owned())?;
        let Some(RuntimeQueryMaterializationState::Building(completion)) =
            materializations.get(&key)
        else {
            return Err("Runtime Query materialization lost its Building claim".to_owned());
        };
        let completion = Arc::clone(completion);
        let terminal = match result {
            Ok(value) => RuntimeQueryTerminalState::Ready(Arc::new(value)),
            Err(error) => RuntimeQueryTerminalState::Failed(Arc::new(error)),
        };
        materializations.remove(&key);
        materializations.retain(|existing_key, state| {
            matches!(state, RuntimeQueryMaterializationState::Building(_))
                || query_materialization_slot(existing_key).ok() != Some(terminal_slot)
        });
        materializations.insert(key, terminal.clone().materialization_state());
        completion.send_replace(Some(terminal));
        Ok(())
    }

    pub(crate) async fn await_query_materialization(
        &self,
        key: &str,
    ) -> Result<RuntimeQueryTerminalState, String> {
        match self.query_materialization(key)? {
            Some(RuntimeQueryMaterializationState::Building(completion)) => {
                let mut receiver = completion.subscribe();
                if receiver.borrow_and_update().is_none() {
                    receiver
                        .changed()
                        .await
                        .map_err(|_| "Query completion channel closed".to_owned())?;
                }
                receiver
                    .borrow()
                    .clone()
                    .ok_or_else(|| "Query completion did not publish a terminal".to_owned())
            }
            Some(RuntimeQueryMaterializationState::Ready(value)) => {
                Ok(RuntimeQueryTerminalState::Ready(value))
            }
            Some(RuntimeQueryMaterializationState::Failed(error)) => {
                Ok(RuntimeQueryTerminalState::Failed(error))
            }
            None => Err("Query materialization has no claim".to_owned()),
        }
    }

    pub fn from_resident_with_execution_publication(
        resident: agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient,
        execution_publication: agent_semantic_content_identity::runtime_workspace_execution_publication::RuntimeWorkspaceExecutionPublication,
        resource_supervisor: agent_semantic_workspace_scheduler::RuntimeServerResourceSupervisor,
        task_scope: agent_semantic_workspace_scheduler::RuntimeServerTaskScope,
    ) -> Result<Self, String> {
        let generation = Self::from_resident(resident, resource_supervisor, task_scope)?;
        execution_publication.validate().map_err(|error| {
            format!("validate resident Runtime execution publication: {error:?}")
        })?;
        if execution_publication.generation_digest.as_str() != generation.generation_digest
            || !execution_source_root_matches_resident(
                execution_publication.source_root_digest.as_str(),
                &generation.resident().source_root_digest(),
            )
        {
            return Err(
                "reasonKind=runtime-query-generation-execution-publication-mismatch".to_owned(),
            );
        }
        Ok(Self {
            execution_publication: Some(Arc::new(execution_publication)),
            ..generation
        })
    }

    pub(crate) async fn acquire_search_resources(
        &self,
        request: agent_semantic_workspace_scheduler::RuntimeServerResourceRequest,
    ) -> Result<agent_semantic_workspace_scheduler::RuntimeServerResourcePermit, String> {
        self.resource_supervisor.acquire(request).await
    }

    pub(crate) fn spawn_search_blocking<T, F>(
        &self,
        name: &'static str,
        operation: F,
    ) -> Result<agent_semantic_workspace_scheduler::RuntimeServerOwnedTask<T>, String>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        self.task_scope.spawn_blocking(name, operation)
    }

    pub(crate) fn publish_lexical_attachment_terminal(&self) {
        self.lexical_attachment_completion.send_replace(true);
    }

    pub(crate) fn fail_lexical_attachment(&self, error: &str) {
        self.resident().fail_lexical_attachment(error);
        self.lexical_attachment_completion.send_replace(true);
    }

    pub(crate) async fn build_or_get_project_topology(
        &self,
        project_root: &std::path::Path,
        resident: &RuntimeResidentReadClient,
        owner_scope: &BTreeSet<String>,
    ) -> Result<Arc<agent_semantic_topology::RuntimeProjectTopologyAttachment>, String> {
        if owner_scope.is_empty() {
            return Err("reasonKind=runtime-project-topology-owner-scope-empty".to_owned());
        }
        let topology_source_generation_digest = resident.topology_source_generation_digest()?;
        if let Some(stored) = self
            .project_topology_attachment
            .lock()
            .map_err(|_| "Runtime Project Topology cache poisoned".to_owned())?
            .as_ref()
            .filter(|stored| stored.matches(&topology_source_generation_digest, owner_scope))
            .cloned()
        {
            return stored.attachment.map_err(|error| error.to_string());
        }
        let _build = self.project_topology_build_lock.lock().await;
        if let Some(stored) = self
            .project_topology_attachment
            .lock()
            .map_err(|_| "Runtime Project Topology cache poisoned".to_owned())?
            .as_ref()
            .filter(|stored| stored.matches(&topology_source_generation_digest, owner_scope))
            .cloned()
        {
            return stored.attachment.map_err(|error| error.to_string());
        }
        let built = self
            .build_project_topology(project_root, resident, owner_scope)
            .await;
        let (stored, memory_permit) = match built {
            Ok((attachment, permit)) => (Ok(Arc::new(attachment)), Some(permit)),
            Err(error) => (Err(Arc::<str>::from(error)), None),
        };
        let returned = stored
            .as_ref()
            .map(Arc::clone)
            .map_err(|error| error.to_string());
        self.project_topology_attachment
            .lock()
            .map_err(|_| "Runtime Project Topology cache poisoned".to_owned())?
            .replace(RuntimeProjectTopologyCacheEntry::new(
                topology_source_generation_digest,
                owner_scope.clone(),
                stored,
                memory_permit,
            ));
        returned
    }

    pub(crate) fn resident_syntax_scope_evidence(
        &self,
        plan_digest: &str,
        owner_scope: &BTreeSet<String>,
    ) -> Result<
        Option<Arc<Vec<agent_semantic_client_protocol::AspClientWorkspaceSyntaxQueryEvidence>>>,
        String,
    > {
        Ok(self
            .resident_syntax_scope_evidence
            .lock()
            .map_err(|_| "Runtime resident syntax scope cache poisoned".to_owned())?
            .as_ref()
            .filter(|cached| {
                cached.plan_digest == plan_digest && cached.owner_scope == *owner_scope
            })
            .map(|cached| Arc::clone(&cached.evidence)))
    }

    pub(crate) fn publish_resident_syntax_scope_evidence(
        &self,
        plan_digest: String,
        owner_scope: BTreeSet<String>,
        evidence: Arc<Vec<agent_semantic_client_protocol::AspClientWorkspaceSyntaxQueryEvidence>>,
    ) -> Result<(), String> {
        self.resident_syntax_scope_evidence
            .lock()
            .map_err(|_| "Runtime resident syntax scope cache poisoned".to_owned())?
            .replace(RuntimeSyntaxScopeEvidence {
                plan_digest,
                owner_scope,
                evidence,
            });
        Ok(())
    }

    async fn build_project_topology(
        &self,
        project_root: &std::path::Path,
        resident: &RuntimeResidentReadClient,
        owner_scope: &BTreeSet<String>,
    ) -> Result<
        (
            agent_semantic_topology::RuntimeProjectTopologyAttachment,
            agent_semantic_workspace_scheduler::RuntimeServerResourcePermit,
        ),
        String,
    > {
        let execution_publication = self.execution_publication.as_deref().ok_or_else(|| {
            "reasonKind=runtime-project-topology-execution-publication-missing".to_owned()
        })?;
        let runtime_binding = &execution_publication.runtime_execution_binding;
        let manifest =
            agent_semantic_topology::ProjectTopologyManifest::load_from_project_root(project_root)
                .map_err(|error| error.to_string())?;
        if manifest.project_workspace() != &runtime_binding.project_workspace {
            return Err("reasonKind=runtime-project-topology-manifest-binding-mismatch".to_owned());
        }

        let source = resident.topology_source_segments_for_owner_scope(owner_scope)?;
        if source.is_empty() {
            return Err("reasonKind=runtime-project-topology-source-empty".to_owned());
        }
        let mut admitted_nodes = BTreeSet::<(
            agent_semantic_content_identity::ProviderRelationEndpointKindV1,
            String,
        )>::new();
        for segment in &source {
            admitted_nodes.insert((
                agent_semantic_content_identity::ProviderRelationEndpointKindV1::Owner,
                segment.owner_path.clone(),
            ));
            admitted_nodes.extend(segment.selectors.iter().cloned().map(|selector| {
                (
                    agent_semantic_content_identity::ProviderRelationEndpointKindV1::Item,
                    selector,
                )
            }));
        }

        let mut topology_segments = Vec::with_capacity(source.len());
        let mut input_edge_count = 0usize;
        let mut node_count = 0usize;
        let mut source_descriptor_bytes = 0usize;
        for segment in source {
            source_descriptor_bytes = source_descriptor_bytes
                .saturating_add(segment.owner_path.len())
                .saturating_add(segment.content_digest.len())
                .saturating_add(segment.selectors.iter().map(String::len).sum::<usize>());
            let language_id = segment
                .authority
                .as_ref()
                .map(|authority| authority.language_id.as_str())
                .or_else(|| {
                    segment
                        .selectors
                        .first()
                        .and_then(|selector| selector.split_once("://").map(|(lang, _)| lang))
                })
                .unwrap_or("unknown");
            let mut nodes = Vec::new();
            nodes.push(
                agent_semantic_topology::ProjectTopologySourceNode::new_owner(
                    topology_node_id("owner", &segment.owner_path),
                    language_id,
                    segment.owner_path.clone(),
                )
                .map_err(|error| error.to_string())?,
            );
            for selector in &segment.selectors {
                nodes.push(
                    agent_semantic_topology::ProjectTopologySourceNode::new(
                        topology_node_id("item", selector),
                        selector.clone(),
                    )
                    .map_err(|error| error.to_string())?,
                );
            }
            let mut edges = Vec::with_capacity(
                segment
                    .relations
                    .len()
                    .saturating_add(segment.selectors.len()),
            );
            for selector in &segment.selectors {
                let edge_identity = format!(
                    "{}\0owner\0{}\0CONTAINS\0item\0{}",
                    segment.owner_path, segment.owner_path, selector,
                );
                edges.push(
                    agent_semantic_topology::ProjectTopologyDirectEdge::new(
                        topology_edge_id(&edge_identity),
                        "CONTAINS",
                        topology_node_id("owner", &segment.owner_path),
                        topology_node_id("item", selector),
                    )
                    .map_err(|error| error.to_string())?,
                );
            }
            for owned in &segment.relations {
                let relation = &owned.relation;
                source_descriptor_bytes = source_descriptor_bytes
                    .saturating_add(owned.owner_path.as_str().len())
                    .saturating_add(relation.from.id.len())
                    .saturating_add(relation.kind.as_str().len())
                    .saturating_add(relation.to.id.len());
                if relation.from.kind
                    == agent_semantic_content_identity::ProviderRelationEndpointKindV1::Owner
                    && relation.from.id == segment.owner_path
                    && relation.kind.as_str().eq_ignore_ascii_case("CONTAINS")
                    && relation.to.kind
                        == agent_semantic_content_identity::ProviderRelationEndpointKindV1::Item
                    && segment.selectors.contains(&relation.to.id)
                {
                    continue;
                }
                if [&relation.from, &relation.to]
                    .into_iter()
                    .any(|endpoint| !admitted_nodes.contains(&(endpoint.kind, endpoint.id.clone())))
                {
                    // This is a request-local topology cut. Cross-frontier
                    // relations are intentionally excluded rather than making
                    // an unrelated owner a first-Search parser dependency.
                    continue;
                }
                let from_kind = relation.from.kind.as_str();
                let to_kind = relation.to.kind.as_str();
                let edge_identity = format!(
                    "{}\0{}\0{}\0{}\0{}\0{}",
                    segment.owner_path,
                    from_kind,
                    relation.from.id,
                    relation.kind.as_str(),
                    to_kind,
                    relation.to.id,
                );
                edges.push(
                    agent_semantic_topology::ProjectTopologyDirectEdge::new(
                        topology_edge_id(&edge_identity),
                        relation.kind.as_str(),
                        topology_node_id(from_kind, &relation.from.id),
                        topology_node_id(to_kind, &relation.to.id),
                    )
                    .map_err(|error| error.to_string())?,
                );
            }
            node_count = node_count.saturating_add(nodes.len());
            input_edge_count = input_edge_count.saturating_add(edges.len());
            topology_segments.push(
                agent_semantic_topology::ProjectTopologySourceSegment::new(
                    segment.owner_path,
                    segment.content_digest,
                    nodes,
                    edges,
                )
                .map_err(|error| error.to_string())?,
            );
        }

        let search_authority = resident.search_generation_authority();
        let parser_catalog_digest = bound_topology_digest(
            "parser-catalog",
            [
                search_authority.provider_schema_digest.as_str(),
                runtime_binding
                    .content_binding
                    .identity
                    .provider_catalog_digest
                    .as_str(),
            ],
        );
        let topology_source_program =
            agent_semantic_topology::ProjectTopologySourceProgram::standard()
                .map_err(|error| error.to_string())?;
        let resolver_digest = bound_topology_digest(
            "topology-resolver",
            [
                search_authority.search_projection_manifest_digest.as_str(),
                search_authority.search_projection_analyzer_digest.as_str(),
                topology_source_program.source_digest(),
            ],
        );
        let inference_program = Arc::new(
            topology_source_program
                .inference_program()
                .map_err(|error| error.to_string())?,
        );
        let identity = agent_semantic_topology::ProjectTopologyGenerationIdentity::new(
            manifest.project_workspace().clone(),
            runtime_binding
                .content_binding
                .identity
                .source_generation_digest
                .as_str()
                .to_owned(),
            runtime_binding
                .content_binding
                .identity
                .provider_catalog_digest
                .as_str()
                .to_owned(),
            inference_program,
            parser_catalog_digest.clone(),
            resolver_digest,
            runtime_binding
                .content_binding
                .identity
                .schema_digest
                .as_str()
                .to_owned(),
        )
        .map_err(|error| error.to_string())?;
        let closure_limit = node_count
            .saturating_mul(node_count)
            .max(input_edge_count)
            .clamp(1, 1_000_000);
        let limits = agent_semantic_topology::ProjectTopologyClosureLimits::new(
            input_edge_count.max(1),
            closure_limit,
            closure_limit,
        )
        .map_err(|error| error.to_string())?;
        let builder =
            agent_semantic_topology::ProjectTopologyGenerationBuilder::new(identity, limits);
        let topology_memory_bytes =
            crate::runtime_asp_client::workspace_search_resources::project_topology_working_memory_bytes(
                source_descriptor_bytes,
                node_count,
                input_edge_count,
                closure_limit,
            );
        let topology_permit = self
            .acquire_search_resources(
                agent_semantic_workspace_scheduler::RuntimeServerResourceRequest {
                    cpu: 1,
                    memory_bytes: topology_memory_bytes,
                },
            )
            .await?;
        let topology_task =
            self.spawn_search_blocking("runtime-project-topology-build", move || {
                let mut topology_permit = topology_permit;
                let candidate = builder
                    .build_from_scratch_on_blocking_lane(topology_segments)
                    .map_err(|error| error.to_string());
                topology_permit.release_cpu();
                candidate.map(|candidate| (candidate, topology_permit))
            })?;
        let (candidate, topology_permit) = topology_task.join().await??;
        let topology_receipts = BTreeMap::from([(
            candidate.rebuild_receipt_id().to_owned(),
            candidate.rebuild_receipt().clone(),
        )]);
        let library = Arc::new(
            candidate
                .admit(&topology_receipts)
                .map_err(|error| error.to_string())?,
        );
        let attachment_candidate =
            agent_semantic_topology::RuntimeProjectTopologyAttachmentCandidate::build(
                self.generation_digest.clone().into(),
                runtime_binding.clone(),
                library,
                parser_catalog_digest,
            )
            .map_err(|error| error.to_string())?;
        let attachment_receipts = BTreeMap::from([(
            attachment_candidate.receipt_digest().to_owned(),
            attachment_candidate.inference_receipt().clone(),
        )]);
        let attachment = attachment_candidate
            .admit(&attachment_receipts)
            .map_err(|error| error.to_string())?;
        Ok((attachment, topology_permit))
    }

    /// Returns the exact resident-generation digest.
    pub fn generation_digest(&self) -> &str {
        &self.generation_digest
    }

    /// Returns the content generation consumed by all Search accelerators.
    pub fn content_generation_digest(&self) -> &str {
        &self
            .resident()
            .search_generation_authority()
            .content_search_generation
            .content_generation_digest
    }

    /// Returns the process-local monotonic observation token.
    pub fn generation_token(&self) -> u64 {
        self.generation_token.load(Ordering::Acquire)
    }

    /// Returns the measured build-resource receipt when calibration completed.
    pub fn build_resource_receipt(&self) -> Option<&RuntimeSearchGenerationBuildResourceReceipt> {
        self.build_resource_receipt.get()
    }

    /// Borrows the resident read authority.
    pub fn resident(&self) -> &RuntimeResidentReadClient {
        self.resident
            .as_deref()
            .expect("ready query generation always owns a resident read client")
    }

    /// Clones the generation-owned resident handle without reopening or
    /// rebuilding its process lease.
    pub(crate) fn resident_arc(&self) -> Arc<RuntimeResidentReadClient> {
        Arc::clone(
            self.resident
                .as_ref()
                .expect("ready query generation always owns a resident read client"),
        )
    }

    /// Borrows the immutable source/Runtime product installed with this generation.
    pub fn execution_publication(
        &self,
    ) -> Option<&agent_semantic_content_identity::runtime_workspace_execution_publication::RuntimeWorkspaceExecutionPublication>
    {
        self.execution_publication.as_deref()
    }

    pub async fn await_lexical_attachment(&self) -> Result<(), String> {
        if !*self.lexical_attachment_completion.borrow() {
            let mut completion = self.lexical_attachment_completion.subscribe();
            if !*completion.borrow_and_update() {
                completion.changed().await.map_err(|_| {
                    "reasonKind=runtime-search-lexical-completion-closed".to_owned()
                })?;
            }
        }
        Ok(())
    }

    pub fn native_syntax_state(&self) -> &'static str {
        // Parser facts are part of the immutable resident generation. There is
        // no second provider-owned native-syntax publication authority.
        "ready"
    }

    pub fn parser_owned_callable_selector_pairs(
        &self,
        owner_paths: &[String],
    ) -> Result<Vec<(String, String)>, String> {
        self.resident()
            .parser_owned_callable_selector_pairs(owner_paths)
    }

    /// Reads an exact selector from the parser-owned attachment without any
    /// filesystem, database, socket, or provider operation.
    pub fn read_runtime_selector(
        &self,
        projection_kind: agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind,
        structural_selector: &str,
    ) -> Result<
        agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead,
        String,
    > {
        self.resident()
            .read_runtime_selector(projection_kind, structural_selector)
    }

    #[expect(
        clippy::type_complexity,
        reason = "the V1 projection returns its three typed evidence collections"
    )]
    pub fn native_syntax_playbook_projection(
        &self,
        owner_paths: &[String],
    ) -> Result<
        (
            Vec<agent_semantic_search::NativeSyntaxProjection>,
            Vec<agent_semantic_search::NativeSyntaxRelation>,
            Vec<agent_semantic_search::NativeSyntaxDiagnostic>,
        ),
        String,
    > {
        self.resident()
            .native_syntax_playbook_projection(owner_paths)
    }
}

fn query_materialization_slot(key: &str) -> Result<&str, String> {
    let slot = key
        .split_once('\0')
        .map(|(slot, _)| slot)
        .ok_or_else(|| "Runtime Query materialization key has no projection slot".to_owned())?;
    match slot {
        "source" | "callable-skeleton" => Ok(slot),
        _ => Err("Runtime Query materialization key has no V1 projection slot".to_owned()),
    }
}

fn execution_source_root_matches_resident(
    execution_source_root: &str,
    resident_source_root: &str,
) -> bool {
    execution_source_root == resident_source_root
        || execution_source_root
            .strip_prefix("blake3-256:")
            .is_some_and(|content| content == resident_source_root)
}

fn topology_node_id(kind: &str, value: &str) -> String {
    let digest = bound_topology_digest("topology-node", [kind, value]);
    format!("{kind}-{}", &digest["blake3-256:".len()..][..20])
}

fn topology_edge_id(value: &str) -> String {
    let digest = bound_topology_digest("topology-edge", [value]);
    format!("edge-{}", &digest["blake3-256:".len()..][..20])
}

fn bound_topology_digest<'a>(domain: &str, parts: impl IntoIterator<Item = &'a str>) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&(domain.len() as u64).to_be_bytes());
    hasher.update(domain.as_bytes());
    for part in parts {
        hasher.update(&(part.len() as u64).to_be_bytes());
        hasher.update(part.as_bytes());
    }
    format!("blake3-256:{}", hasher.finalize().to_hex())
}

#[cfg(test)]
#[path = "../tests/unit/runtime_query_generation.rs"]
mod tests;
