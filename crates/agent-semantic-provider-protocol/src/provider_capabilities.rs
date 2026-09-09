// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Provider-owned capability descriptors shared by Server, Runtime, and Hook.
//!
//! These are registration-protocol data. Keeping them in the Provider
//! Protocol crate prevents Runtime consumers from depending on the Hook
//! compiler merely to name or serialize provider capabilities.

use serde::Deserialize;
use serde::Serialize;

/// Provider-owned search surfaces that ASP may delegate instead of approximating.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderSearchCapabilities {
    pub owner_items: bool,
    pub semantic_facts: bool,
    pub dependency_topology: bool,
    pub dependency_topology_metadata: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_snapshot: Option<ProviderSourceSnapshotDescriptor>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
struct ProviderSourceDescriptorId(String);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
struct ProviderSourceDescriptorVersion(String);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
struct ProviderSourceSchemaId(String);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
struct ProviderSourceSnapshotAlgorithm(String);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
struct ProviderSourceSnapshotAuthority(String);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
struct ProviderExactSelectorResolution(String);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
struct ProviderSourceOverlayMode(String);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderSourceSnapshotDescriptor {
    descriptor_id: ProviderSourceDescriptorId,
    descriptor_version: ProviderSourceDescriptorVersion,
    language_id: agent_semantic_config::LanguageId,
    packet_schema_id: ProviderSourceSchemaId,
    exact_source_packet_schema_id: ProviderSourceSchemaId,
    canonical_item_selector_schema_id: ProviderSourceSchemaId,
    source_snapshot_envelope_schema_id: ProviderSourceSchemaId,
    derived_artifact_evidence_schema_id: ProviderSourceSchemaId,
    algorithm: ProviderSourceSnapshotAlgorithm,
    authority: ProviderSourceSnapshotAuthority,
    exact_selector_resolution: ProviderExactSelectorResolution,
    overlay_mode: ProviderSourceOverlayMode,
}

impl ProviderSourceSnapshotDescriptor {
    pub fn descriptor_id(&self) -> &str {
        &self.descriptor_id.0
    }

    pub fn descriptor_version(&self) -> &str {
        &self.descriptor_version.0
    }

    pub fn language_id(&self) -> &str {
        self.language_id.as_str()
    }

    pub fn packet_schema_id(&self) -> &str {
        &self.packet_schema_id.0
    }

    pub fn exact_source_packet_schema_id(&self) -> &str {
        &self.exact_source_packet_schema_id.0
    }

    pub fn canonical_item_selector_schema_id(&self) -> &str {
        &self.canonical_item_selector_schema_id.0
    }

    pub fn source_snapshot_envelope_schema_id(&self) -> &str {
        &self.source_snapshot_envelope_schema_id.0
    }

    pub fn derived_artifact_evidence_schema_id(&self) -> &str {
        &self.derived_artifact_evidence_schema_id.0
    }

    pub fn algorithm(&self) -> &str {
        &self.algorithm.0
    }

    pub fn authority(&self) -> &str {
        &self.authority.0
    }

    pub fn exact_selector_resolution(&self) -> &str {
        &self.exact_selector_resolution.0
    }

    pub fn overlay_mode(&self) -> &str {
        &self.overlay_mode.0
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderSemanticFactsDescriptor {
    pub descriptor_id: String,
    pub descriptor_version: String,
    pub packet_schema_ids: Vec<String>,
    pub fact_kinds: Vec<String>,
    pub intent_axes: Vec<ProviderSemanticFactsIntentAxis>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
struct ProviderSemanticFactAxis(String);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
struct ProviderSemanticFactTerm(String);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderSemanticFactsIntentAxis {
    axis: ProviderSemanticFactAxis,
    terms: Vec<ProviderSemanticFactTerm>,
    #[serde(default)]
    roles: Vec<ProviderQueryPackTermRole>,
}

impl ProviderSemanticFactsIntentAxis {
    pub fn axis(&self) -> &str {
        &self.axis.0
    }

    pub fn terms(&self) -> impl Iterator<Item = &str> {
        self.terms.iter().map(|term| term.0.as_str())
    }

    pub fn roles(&self) -> &[ProviderQueryPackTermRole] {
        &self.roles
    }

    /// Return this intent axis with a replaced typed term set.
    #[must_use]
    pub fn with_terms<I, S>(mut self, terms: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.terms = terms
            .into_iter()
            .map(|term| ProviderSemanticFactTerm(term.into()))
            .collect();
        self
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
struct ProviderQueryPackDescriptorId(String);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
struct ProviderQueryPackDescriptorVersion(String);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
struct ProviderSemanticFactsDescriptorId(String);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderQueryPackDescriptor {
    descriptor_id: ProviderQueryPackDescriptorId,
    descriptor_version: ProviderQueryPackDescriptorVersion,
    language_id: agent_semantic_config::LanguageId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    semantic_facts_descriptor_id: Option<ProviderSemanticFactsDescriptorId>,
    #[serde(default)]
    term_role_overrides: Vec<ProviderQueryPackTermRoleOverride>,
    recipes: Vec<ProviderQueryPackRecipe>,
}

impl ProviderQueryPackDescriptor {
    pub fn descriptor_id(&self) -> &str {
        &self.descriptor_id.0
    }

    pub fn descriptor_version(&self) -> &str {
        &self.descriptor_version.0
    }

    pub fn language_id(&self) -> &str {
        self.language_id.as_str()
    }

    pub fn semantic_facts_descriptor_id(&self) -> Option<&str> {
        self.semantic_facts_descriptor_id
            .as_ref()
            .map(|value| value.0.as_str())
    }

    pub fn term_role_overrides(&self) -> &[ProviderQueryPackTermRoleOverride] {
        &self.term_role_overrides
    }

    pub fn recipes(&self) -> &[ProviderQueryPackRecipe] {
        &self.recipes
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderQueryPackTermRole {
    Context,
    Concept,
    Symbol,
    Literal,
    DiagnosticCode,
}

impl ProviderQueryPackTermRole {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Context => "context",
            Self::Concept => "concept",
            Self::Symbol => "symbol",
            Self::Literal => "literal",
            Self::DiagnosticCode => "diagnostic-code",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderQueryPackTermRoleOverride {
    pub term: String,
    pub role: ProviderQueryPackTermRole,
    #[serde(default)]
    pub case_sensitive: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderQueryPackRecipe {
    pub recipe_id: String,
    pub trigger: ProviderQueryPackTrigger,
    pub clauses: Vec<ProviderQueryPackClause>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderQueryPackTrigger {
    pub terms: Vec<String>,
    pub r#match: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderQueryPackClause {
    pub terms: Vec<String>,
    #[serde(default)]
    pub roles: Vec<ProviderQueryPackTermRole>,
    #[serde(default)]
    pub intent_axes: Vec<String>,
}
