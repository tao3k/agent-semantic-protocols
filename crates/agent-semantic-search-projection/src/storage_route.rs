//! Language-neutral storage profile and search-route decision contract.

pub const SEMANTIC_SEARCH_STORAGE_ROUTE_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-search-storage-route";
pub const SEMANTIC_SEARCH_STORAGE_ROUTE_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticMutationClass {
    Immutable,
    LowMutation,
    GenerationBound,
    OwnerDynamic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticSharingScope {
    Global,
    ToolchainVersion,
    PackageVersion,
    WorkspaceGeneration,
    OwnerContent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticSearchStorageClass {
    StaticSharedDatabase,
    GenerationShallowDatabase,
    OwnerLocalResident,
    ExactResident,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticSearchQueryRoute {
    StaticDatabaseIndex,
    ShallowDatabaseIndex,
    ResidentMemory,
    ExactMmap,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderGraphEvidence {
    pub provider_id: String,
    pub algorithm_id: String,
    pub evidence_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticSearchAlgorithmEvidence {
    pub tree_depth: u32,
    pub traversal_radius: u32,
    pub graph_node_count: u64,
    pub graph_edge_count: u64,
    pub merkle_delta_owner_count: u32,
    pub dependency_fan_out: u32,
    pub cross_workspace_reuse_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub component_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cut_vertex_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provider_graph_evidence: Vec<ProviderGraphEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticSearchStorageProfile {
    pub mutation_class: SemanticMutationClass,
    pub sharing_scope: SemanticSharingScope,
    pub identity_digest: String,
    pub algorithm_evidence: SemanticSearchAlgorithmEvidence,
}

impl SemanticSearchStorageProfile {
    pub fn validate(&self) -> Result<(), String> {
        if self.identity_digest.is_empty() {
            return Err("semantic search storage profile requires identity digest".to_owned());
        }
        for evidence in &self.algorithm_evidence.provider_graph_evidence {
            if evidence.provider_id.is_empty()
                || evidence.algorithm_id.is_empty()
                || evidence.evidence_digest.is_empty()
            {
                return Err(
                    "provider graph evidence requires provider, algorithm, and digest".to_owned(),
                );
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticSearchRouteDecision {
    pub schema_id: String,
    pub schema_version: String,
    pub profile: SemanticSearchStorageProfile,
    pub storage_class: SemanticSearchStorageClass,
    pub query_route: SemanticSearchQueryRoute,
}

impl SemanticSearchRouteDecision {
    fn assemble(
        profile: SemanticSearchStorageProfile,
        storage_class: SemanticSearchStorageClass,
        query_route: SemanticSearchQueryRoute,
    ) -> Result<Self, String> {
        let decision = Self {
            schema_id: SEMANTIC_SEARCH_STORAGE_ROUTE_SCHEMA_ID.to_owned(),
            schema_version: SEMANTIC_SEARCH_STORAGE_ROUTE_SCHEMA_VERSION.to_owned(),
            profile,
            storage_class,
            query_route,
        };
        decision.validate()?;
        Ok(decision)
    }

    pub fn static_database(profile: SemanticSearchStorageProfile) -> Result<Self, String> {
        Self::assemble(
            profile,
            SemanticSearchStorageClass::StaticSharedDatabase,
            SemanticSearchQueryRoute::StaticDatabaseIndex,
        )
    }

    pub fn shallow_database(profile: SemanticSearchStorageProfile) -> Result<Self, String> {
        Self::assemble(
            profile,
            SemanticSearchStorageClass::GenerationShallowDatabase,
            SemanticSearchQueryRoute::ShallowDatabaseIndex,
        )
    }

    pub fn resident_memory(profile: SemanticSearchStorageProfile) -> Result<Self, String> {
        Self::assemble(
            profile,
            SemanticSearchStorageClass::OwnerLocalResident,
            SemanticSearchQueryRoute::ResidentMemory,
        )
    }

    pub fn exact_mmap(profile: SemanticSearchStorageProfile) -> Result<Self, String> {
        Self::assemble(
            profile,
            SemanticSearchStorageClass::ExactResident,
            SemanticSearchQueryRoute::ExactMmap,
        )
    }

    pub fn validate(&self) -> Result<(), String> {
        self.profile.validate()?;
        if self.schema_id != SEMANTIC_SEARCH_STORAGE_ROUTE_SCHEMA_ID
            || self.schema_version != SEMANTIC_SEARCH_STORAGE_ROUTE_SCHEMA_VERSION
        {
            return Err("semantic search route schema identity is invalid".to_owned());
        }
        let valid_pair = matches!(
            (self.storage_class, self.query_route),
            (
                SemanticSearchStorageClass::StaticSharedDatabase,
                SemanticSearchQueryRoute::StaticDatabaseIndex
            ) | (
                SemanticSearchStorageClass::GenerationShallowDatabase,
                SemanticSearchQueryRoute::ShallowDatabaseIndex
            ) | (
                SemanticSearchStorageClass::OwnerLocalResident,
                SemanticSearchQueryRoute::ResidentMemory
            ) | (
                SemanticSearchStorageClass::ExactResident,
                SemanticSearchQueryRoute::ExactMmap
            )
        );
        if !valid_pair {
            return Err("semantic search storage class and query route disagree".to_owned());
        }
        let profile_matches_route = match self.query_route {
            SemanticSearchQueryRoute::StaticDatabaseIndex => {
                matches!(
                    self.profile.mutation_class,
                    SemanticMutationClass::Immutable | SemanticMutationClass::LowMutation
                ) && matches!(
                    self.profile.sharing_scope,
                    SemanticSharingScope::Global
                        | SemanticSharingScope::ToolchainVersion
                        | SemanticSharingScope::PackageVersion
                )
            }
            SemanticSearchQueryRoute::ShallowDatabaseIndex => {
                self.profile.mutation_class == SemanticMutationClass::GenerationBound
                    && self.profile.sharing_scope == SemanticSharingScope::WorkspaceGeneration
                    && self.profile.algorithm_evidence.tree_depth <= 1
                    && self.profile.algorithm_evidence.traversal_radius <= 1
            }
            SemanticSearchQueryRoute::ResidentMemory | SemanticSearchQueryRoute::ExactMmap => true,
        };
        profile_matches_route.then_some(()).ok_or_else(|| {
            "semantic search storage profile is incompatible with selected route".to_owned()
        })
    }

    pub fn validate_database_route(&self) -> Result<(), String> {
        self.validate()?;
        matches!(
            self.query_route,
            SemanticSearchQueryRoute::StaticDatabaseIndex
                | SemanticSearchQueryRoute::ShallowDatabaseIndex
        )
        .then_some(())
        .ok_or_else(|| "database adapter rejected non-database semantic search route".to_owned())
    }
}
