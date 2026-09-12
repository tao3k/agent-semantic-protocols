// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Immutable Runtime Query generation with independently settled Search attachments.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, OnceLock};

use agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient;

use super::query_generation_calibration::RuntimeSearchGenerationBuildResourceReceipt;

/// Immutable Runtime handle for one admitted resident Query generation.
///
/// Tantivy is a generation-scoped Search attachment. Graph and Project
/// Topology are request-scoped projections over lazily materialized owners;
/// none of their failures can revoke exact native Query reads from the base.
pub struct RuntimeQueryGeneration {
    pub(super) generation_digest: String,
    pub(super) generation_token: AtomicU64,
    pub(super) resident: Option<Arc<RuntimeResidentReadClient>>,
    pub(super) execution_publication: Option<
        Arc<
            agent_semantic_content_identity::runtime_workspace_execution_publication::RuntimeWorkspaceExecutionPublication,
        >,
    >,
    pub(super) project_topology_attachment:
        OnceLock<
            Result<
                Arc<agent_semantic_topology::RuntimeProjectTopologyAttachment>,
                Arc<str>,
            >,
        >,
    pub(super) project_topology_completion: tokio::sync::watch::Sender<bool>,
    pub(super) lexical_attachment_completion: tokio::sync::watch::Sender<bool>,
    pub(super) build_resource_receipt:
        std::sync::OnceLock<RuntimeSearchGenerationBuildResourceReceipt>,
    pub(super) search_materializations: Mutex<
        std::collections::HashMap<String, RuntimeSearchMaterializationState>,
    >,
    pub(super) query_materializations: Mutex<
        std::collections::HashMap<String, RuntimeQueryMaterializationState>,
    >,
}

#[derive(Clone)]
pub(crate) enum RuntimeSearchMaterializationState {
    Building(Arc<tokio::sync::watch::Sender<bool>>),
    Ready(Arc<serde_json::Value>),
    Failed(Arc<agent_semantic_client_server::AspClientDispatchError>),
}

#[derive(Clone)]
pub(crate) enum RuntimeQueryMaterializationState {
    Building(Arc<tokio::sync::watch::Sender<bool>>),
    Ready(Arc<serde_json::Value>),
    Failed(Arc<agent_semantic_client_server::AspClientDispatchError>),
}

impl RuntimeQueryGeneration {
    /// Opens the resident Search authority and derives its fusion capabilities.
    pub async fn open(
        pointer_path: &std::path::Path,
        project_root: &std::path::Path,
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
            execution_publication: None,
            project_topology_attachment: OnceLock::new(),
            project_topology_completion: tokio::sync::watch::channel(false).0,
            lexical_attachment_completion: tokio::sync::watch::channel(false).0,
            build_resource_receipt: std::sync::OnceLock::new(),
            search_materializations: Mutex::new(std::collections::HashMap::new()),
            query_materializations: Mutex::new(std::collections::HashMap::new()),
        })
    }

    pub fn from_resident(
        resident: agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient,
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
            execution_publication: None,
            project_topology_attachment: OnceLock::new(),
            project_topology_completion: tokio::sync::watch::channel(false).0,
            lexical_attachment_completion: tokio::sync::watch::channel(false).0,
            build_resource_receipt: std::sync::OnceLock::new(),
            search_materializations: Mutex::new(std::collections::HashMap::new()),
            query_materializations: Mutex::new(std::collections::HashMap::new()),
        })
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
                tokio::sync::watch::channel(false).0,
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
            Ok(value) => RuntimeSearchMaterializationState::Ready(Arc::new(value)),
            Err(error) => RuntimeSearchMaterializationState::Failed(Arc::new(error)),
        };
        materializations.insert(key, terminal);
        completion.send_replace(true);
        Ok(())
    }

    pub(crate) async fn await_search_materialization(
        &self,
        key: &str,
    ) -> Result<RuntimeSearchMaterializationState, String> {
        match self.search_materialization(key)? {
            Some(RuntimeSearchMaterializationState::Building(completion)) => {
                let mut receiver = completion.subscribe();
                if !*receiver.borrow_and_update() {
                    receiver
                        .changed()
                        .await
                        .map_err(|_| "Search completion channel closed".to_owned())?;
                }
                self.search_materialization(key)?
                    .ok_or_else(|| "Search materialization disappeared".to_owned())
            }
            Some(terminal) => Ok(terminal),
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
                tokio::sync::watch::channel(false).0,
            )),
        );
        Ok(true)
    }

    pub(crate) fn publish_query_materialization(
        &self,
        key: String,
        result: Result<serde_json::Value, agent_semantic_client_server::AspClientDispatchError>,
    ) -> Result<(), String> {
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
            Ok(value) => RuntimeQueryMaterializationState::Ready(Arc::new(value)),
            Err(error) => RuntimeQueryMaterializationState::Failed(Arc::new(error)),
        };
        materializations.insert(key, terminal);
        completion.send_replace(true);
        Ok(())
    }

    pub(crate) async fn await_query_materialization(
        &self,
        key: &str,
    ) -> Result<RuntimeQueryMaterializationState, String> {
        match self.query_materialization(key)? {
            Some(RuntimeQueryMaterializationState::Building(completion)) => {
                let mut receiver = completion.subscribe();
                if !*receiver.borrow_and_update() {
                    receiver
                        .changed()
                        .await
                        .map_err(|_| "Query completion channel closed".to_owned())?;
                }
                self.query_materialization(key)?
                    .ok_or_else(|| "Query materialization disappeared".to_owned())
            }
            Some(terminal) => Ok(terminal),
            None => Err("Query materialization has no claim".to_owned()),
        }
    }

    pub fn from_resident_with_execution_publication(
        resident: agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient,
        execution_publication: agent_semantic_content_identity::runtime_workspace_execution_publication::RuntimeWorkspaceExecutionPublication,
    ) -> Result<Self, String> {
        let generation = Self::from_resident(resident)?;
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

    /// Atomically joins the already admitted Runtime generation and Project
    /// Topology product. Lower-level resident reads can exist without this
    /// attachment; Search Playbook cannot.
    pub fn with_project_topology_attachment(
        self,
        attachment: agent_semantic_topology::RuntimeProjectTopologyAttachment,
    ) -> Result<Self, String> {
        let execution_publication = self.execution_publication.as_deref().ok_or_else(|| {
            "reasonKind=runtime-project-topology-execution-publication-missing".to_owned()
        })?;
        if attachment.runtime_generation_digest() != self.generation_digest
            || attachment.runtime_execution_binding()
                != &execution_publication.runtime_execution_binding
        {
            return Err("reasonKind=runtime-project-topology-attachment-mismatch".to_owned());
        }
        self.project_topology_attachment
            .set(Ok(Arc::new(attachment)))
            .map_err(|_| "reasonKind=runtime-project-topology-attachment-already-set".to_owned())?;
        self.project_topology_completion.send_replace(true);
        Ok(self)
    }

    /// Builds and admits the Project Topology attachment from the exact
    /// resident parser generation. The CPU-heavy closure builder runs on its
    /// bounded blocking lane and never participates in SearchCoreReady.
    pub async fn build_and_attach_project_topology(
        &self,
        project_root: &std::path::Path,
    ) -> Result<(), String> {
        let result = self
            .build_project_topology(project_root, self.resident(), None)
            .await;
        let stored = result.map(Arc::new).map_err(Arc::<str>::from);
        let returned = stored
            .as_ref()
            .map(|_| ())
            .map_err(|error| error.to_string());
        self.project_topology_attachment
            .set(stored)
            .map_err(|_| "reasonKind=runtime-project-topology-attachment-already-set".to_owned())?;
        self.project_topology_completion.send_replace(true);
        returned
    }

    pub(crate) fn publish_lexical_attachment_terminal(&self) {
        self.lexical_attachment_completion.send_replace(true);
    }

    pub(crate) fn fail_lexical_attachment(&self, error: &str) {
        self.resident().fail_lexical_attachment(error);
        self.lexical_attachment_completion.send_replace(true);
    }

    pub(crate) async fn build_project_topology_for_owner_scope(
        &self,
        project_root: &std::path::Path,
        resident: &RuntimeResidentReadClient,
        owner_scope: &BTreeSet<String>,
    ) -> Result<agent_semantic_topology::RuntimeProjectTopologyAttachment, String> {
        self.build_project_topology(project_root, resident, Some(owner_scope))
            .await
    }

    async fn build_project_topology(
        &self,
        project_root: &std::path::Path,
        resident: &RuntimeResidentReadClient,
        owner_scope: Option<&BTreeSet<String>>,
    ) -> Result<agent_semantic_topology::RuntimeProjectTopologyAttachment, String> {
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

        let source = resident
            .topology_source_segments()?
            .into_iter()
            .filter(|segment| owner_scope.is_none_or(|owners| owners.contains(&segment.owner_path)))
            .collect::<Vec<_>>();
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
        for segment in source {
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
        let candidate =
            agent_semantic_topology::ProjectTopologyGenerationBuilder::new(identity, limits)
                .build_from_scratch(topology_segments)
                .await
                .map_err(|error| error.to_string())?;
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
        attachment_candidate
            .admit(&attachment_receipts)
            .map_err(|error| error.to_string())
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

    /// Borrows the immutable source/Runtime product installed with this generation.
    pub fn execution_publication(
        &self,
    ) -> Option<&agent_semantic_content_identity::runtime_workspace_execution_publication::RuntimeWorkspaceExecutionPublication>
    {
        self.execution_publication.as_deref()
    }

    /// Borrows the exact topology authority required by Search Playbook.
    pub fn require_search_playbook_topology_attachment(
        &self,
    ) -> Result<&agent_semantic_topology::RuntimeProjectTopologyAttachment, String> {
        match self.project_topology_attachment.get() {
            Some(Ok(attachment)) => Ok(attachment),
            Some(Err(error)) => Err(error.to_string()),
            None => Err("reasonKind=runtime-project-topology-attachment-missing".to_owned()),
        }
    }

    pub async fn await_search_playbook_topology_attachment(
        &self,
    ) -> Result<&agent_semantic_topology::RuntimeProjectTopologyAttachment, String> {
        if self.project_topology_attachment.get().is_none() {
            let mut completion = self.project_topology_completion.subscribe();
            if !*completion.borrow_and_update() {
                completion.changed().await.map_err(|_| {
                    "reasonKind=runtime-project-topology-completion-closed".to_owned()
                })?;
            }
        }
        self.require_search_playbook_topology_attachment()
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
