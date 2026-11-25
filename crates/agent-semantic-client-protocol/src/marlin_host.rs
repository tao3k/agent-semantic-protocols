// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Typed Marlin-native host integration binding, correlation lineage, and admission rules.

use serde::Deserialize;
use serde::Serialize;

use crate::schema_bundle::SCHEMA_BUNDLE_METHOD;
use crate::server_method_catalog::CANCELLATION_PROBE_METHOD;
use crate::server_method_catalog::GRAPH_EVALUATE_METHOD;
use crate::server_method_catalog::WORKSPACE_GENERATION_ENSURE_READY_METHOD;
use crate::server_method_catalog::WORKSPACE_QUERY_PLAYBOOK_METHOD;
use crate::server_method_catalog::WORKSPACE_SEARCH_PLAYBOOK_METHOD;

/// Schema identifier for the Marlin host integration contract.
pub const MARLIN_HOST_INTEGRATION_SCHEMA_ID: &str =
    "agent.semantic-protocols.marlin-host-integration";
/// Schema version for the Marlin host integration contract.
pub const MARLIN_HOST_INTEGRATION_SCHEMA_VERSION: &str = "1";
/// Canonical host platform discriminator for Marlin.
pub const MARLIN_HOST_PLATFORM: &str = "marlin";

/// Admitted northbound ASP ClientFrame methods available to Marlin.
pub const MARLIN_ADMITTED_METHODS: &[&str] = &[
    WORKSPACE_SEARCH_PLAYBOOK_METHOD,
    WORKSPACE_QUERY_PLAYBOOK_METHOD,
    WORKSPACE_GENERATION_ENSURE_READY_METHOD,
    GRAPH_EVALUATE_METHOD,
    SCHEMA_BUNDLE_METHOD,
    CANCELLATION_PROBE_METHOD,
];

macro_rules! marlin_id_newtype {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            #[must_use]
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }

            #[must_use]
            pub fn is_empty(&self) -> bool {
                self.0.trim().is_empty()
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }
    };
}

marlin_id_newtype!(
    MarlinHostId,
    "Typed identifier for a Marlin runtime host instance."
);
marlin_id_newtype!(
    MarlinProjectId,
    "Typed identifier for a Marlin-managed project."
);
marlin_id_newtype!(
    MarlinWorkspaceId,
    "Typed identifier for a canonical ASP workspace bound to Marlin."
);
marlin_id_newtype!(
    MarlinWorktreeId,
    "Typed identifier for an active Git worktree bound to Marlin."
);
marlin_id_newtype!(
    MarlinRootSessionId,
    "Typed identifier for the Marlin root session owning a trace lineage."
);
marlin_id_newtype!(
    MarlinSessionId,
    "Typed identifier for the active Marlin session issuing semantic requests."
);
marlin_id_newtype!(
    MarlinParentSessionId,
    "Typed identifier for the parent Marlin session in a sub-agent lineage."
);
marlin_id_newtype!(
    MarlinAgentLineageId,
    "Typed lineage path reported by Marlin for read-only observability correlation."
);

/// Capability profile admitted for a Marlin host integration binding.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MarlinCapabilityProfile {
    ReadOnlySemantic,
    VerifiedAstPatch,
    FullWorkspaceGeneration,
}

/// Authoritative Org parser port wired into ASP's topology and search engine.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MarlinOrgPortMode {
    /// Marlin (`marlin-agent-context` / `orgize-fold`) supplies typed Org projections.
    MarlinContextOrgFold,
}

/// Authoritative storage backend port wired into ASP's search engine.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MarlinStoragePortMode {
    /// Marlin (`marlin-agent-storage` / `TursoAgentStorage`) owns unified persistence.
    MarlinAgentStorageTurso,
    /// Zero-dependency in-memory reference port for unit tests and ephemeral runs.
    InMemoryReference,
}

/// Explicit two-port convergence binding between Marlin host services and ASP's search engine.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MarlinEnginePortBindings {
    pub org_port: MarlinOrgPortMode,
    pub storage_port: MarlinStoragePortMode,
}

impl MarlinEnginePortBindings {
    /// Default converged Marlin embedding: Marlin owns Org parsing and Turso storage.
    #[must_use]
    pub const fn marlin_converged_default() -> Self {
        Self {
            org_port: MarlinOrgPortMode::MarlinContextOrgFold,
            storage_port: MarlinStoragePortMode::MarlinAgentStorageTurso,
        }
    }
}

/// Read-only Marlin session lineage metadata propagated for observability correlation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MarlinHostCorrelation {
    pub root_session_id: MarlinRootSessionId,
    pub session_id: MarlinSessionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_session_id: Option<MarlinParentSessionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_lineage: Option<MarlinAgentLineageId>,
}

/// Typed host integration contract binding Marlin (`marlin-agent-core`) to ASP.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MarlinHostIntegrationBinding {
    pub schema_id: String,
    pub schema_version: String,
    pub platform: String,
    pub host_id: MarlinHostId,
    pub project_id: MarlinProjectId,
    pub workspace_id: MarlinWorkspaceId,
    pub worktree_id: MarlinWorktreeId,
    pub capability_profile: MarlinCapabilityProfile,
    pub admitted_methods: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine_ports: Option<MarlinEnginePortBindings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation: Option<MarlinHostCorrelation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_home: Option<String>,
}

/// Deterministic validation error for a Marlin host integration binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MarlinHostIntegrationError {
    SchemaMismatch,
    UnsupportedPlatform(String),
    MissingField(&'static str),
    ParentSessionAliasesCurrent,
    EmptyAdmittedMethods,
    UnadmittedMethod(String),
}

impl MarlinHostCorrelation {
    /// Construct and validate read-only Marlin session correlation metadata.
    pub fn new(
        root_session_id: impl Into<MarlinRootSessionId>,
        session_id: impl Into<MarlinSessionId>,
        parent_session_id: Option<MarlinParentSessionId>,
        agent_lineage: Option<MarlinAgentLineageId>,
    ) -> Result<Self, MarlinHostIntegrationError> {
        let correlation = Self {
            root_session_id: root_session_id.into(),
            session_id: session_id.into(),
            parent_session_id,
            agent_lineage,
        };
        correlation.validate()?;
        Ok(correlation)
    }

    /// Validate that correlation identifiers are non-empty and free of parent/current aliasing.
    pub fn validate(&self) -> Result<(), MarlinHostIntegrationError> {
        if self.root_session_id.is_empty() {
            return Err(MarlinHostIntegrationError::MissingField("rootSessionId"));
        }
        if self.session_id.is_empty() {
            return Err(MarlinHostIntegrationError::MissingField("sessionId"));
        }
        if let Some(parent) = &self.parent_session_id {
            if parent.is_empty() {
                return Err(MarlinHostIntegrationError::MissingField("parentSessionId"));
            }
            if parent.as_str() == self.session_id.as_str() {
                return Err(MarlinHostIntegrationError::ParentSessionAliasesCurrent);
            }
        }
        if let Some(lineage) = &self.agent_lineage
            && lineage.is_empty()
        {
            return Err(MarlinHostIntegrationError::MissingField("agentLineage"));
        }
        Ok(())
    }
}

impl MarlinHostIntegrationBinding {
    /// Construct a validated Marlin host binding using the default admitted method surface.
    pub fn new(
        host_id: impl Into<MarlinHostId>,
        project_id: impl Into<MarlinProjectId>,
        workspace_id: impl Into<MarlinWorkspaceId>,
        worktree_id: impl Into<MarlinWorktreeId>,
        capability_profile: MarlinCapabilityProfile,
        correlation: Option<MarlinHostCorrelation>,
        state_home: Option<String>,
    ) -> Result<Self, MarlinHostIntegrationError> {
        let binding = Self {
            schema_id: MARLIN_HOST_INTEGRATION_SCHEMA_ID.to_owned(),
            schema_version: MARLIN_HOST_INTEGRATION_SCHEMA_VERSION.to_owned(),
            platform: MARLIN_HOST_PLATFORM.to_owned(),
            host_id: host_id.into(),
            project_id: project_id.into(),
            workspace_id: workspace_id.into(),
            worktree_id: worktree_id.into(),
            capability_profile,
            admitted_methods: MARLIN_ADMITTED_METHODS
                .iter()
                .map(|method| (*method).to_owned())
                .collect(),
            engine_ports: Some(MarlinEnginePortBindings::marlin_converged_default()),
            correlation,
            state_home,
        };
        binding.validate()?;
        Ok(binding)
    }

    /// Validate every fail-closed Marlin host integration invariant.
    pub fn validate(&self) -> Result<(), MarlinHostIntegrationError> {
        if self.schema_id != MARLIN_HOST_INTEGRATION_SCHEMA_ID
            || self.schema_version != MARLIN_HOST_INTEGRATION_SCHEMA_VERSION
        {
            return Err(MarlinHostIntegrationError::SchemaMismatch);
        }
        if self.platform != MARLIN_HOST_PLATFORM {
            return Err(MarlinHostIntegrationError::UnsupportedPlatform(
                self.platform.clone(),
            ));
        }
        for (field, value) in [
            ("hostId", self.host_id.as_str()),
            ("projectId", self.project_id.as_str()),
            ("workspaceId", self.workspace_id.as_str()),
            ("worktreeId", self.worktree_id.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(MarlinHostIntegrationError::MissingField(field));
            }
        }
        if self.admitted_methods.is_empty() {
            return Err(MarlinHostIntegrationError::EmptyAdmittedMethods);
        }
        for method in &self.admitted_methods {
            if !MARLIN_ADMITTED_METHODS.contains(&method.as_str()) {
                return Err(MarlinHostIntegrationError::UnadmittedMethod(method.clone()));
            }
        }
        if let Some(correlation) = &self.correlation {
            correlation.validate()?;
        }
        if let Some(state_home) = &self.state_home
            && state_home.trim().is_empty()
        {
            return Err(MarlinHostIntegrationError::MissingField("stateHome"));
        }
        Ok(())
    }

    /// Read a typed Marlin host binding once from an explicit environment lookup closure.
    pub fn from_environment<F>(mut read: F) -> Result<Self, MarlinHostIntegrationError>
    where
        F: FnMut(&str) -> Option<String>,
    {
        if let Some(platform) = read("ASP_HOST_PLATFORM")
            && platform != MARLIN_HOST_PLATFORM
        {
            return Err(MarlinHostIntegrationError::UnsupportedPlatform(platform));
        }
        let host_id =
            read("MARLIN_HOST_ID").ok_or(MarlinHostIntegrationError::MissingField("hostId"))?;
        let project_id = read("MARLIN_PROJECT_ID")
            .ok_or(MarlinHostIntegrationError::MissingField("projectId"))?;
        let workspace_id = read("MARLIN_WORKSPACE_ID")
            .ok_or(MarlinHostIntegrationError::MissingField("workspaceId"))?;
        let worktree_id = read("MARLIN_WORKTREE_ID")
            .ok_or(MarlinHostIntegrationError::MissingField("worktreeId"))?;
        let correlation = match read("MARLIN_SESSION_ID") {
            Some(session_id) => {
                let root_session_id =
                    read("MARLIN_ROOT_SESSION_ID").unwrap_or_else(|| session_id.clone());
                let parent_session_id =
                    read("MARLIN_PARENT_SESSION_ID").map(MarlinParentSessionId::new);
                let agent_lineage = read("MARLIN_AGENT_LINEAGE").map(MarlinAgentLineageId::new);
                Some(MarlinHostCorrelation::new(
                    root_session_id,
                    session_id,
                    parent_session_id,
                    agent_lineage,
                )?)
            }
            None => None,
        };
        let state_home = read("ASP_STATE_HOME");
        Self::new(
            host_id,
            project_id,
            workspace_id,
            worktree_id,
            MarlinCapabilityProfile::FullWorkspaceGeneration,
            correlation,
            state_home,
        )
    }
}

#[cfg(test)]
#[path = "../tests/unit/marlin_host.rs"]
mod tests;
