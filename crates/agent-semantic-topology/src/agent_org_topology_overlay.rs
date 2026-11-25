// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Content-bound semantic topology contributed by an external reasoning Agent.

use crate::{OrgDocumentProjectionPort, OrgOverlayDocumentProjection};
use serde::{Deserialize, Serialize};

pub const AGENT_ORG_TOPOLOGY_OVERLAY_SCHEMA_ID: &str =
    "agent.semantic-protocols.agent-org-topology-overlay";
pub const AGENT_ORG_TOPOLOGY_OVERLAY_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentTopologyRelationship {
    pub from_selector: String,
    pub relation: String,
    pub to_selector: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentOrgTopologyOverlay {
    pub schema_id: String,
    pub schema_version: String,
    pub source_generation_digest: String,
    pub base_topology_generation_digest: String,
    pub selector: String,
    pub evidence_digest: String,
    pub agent_identity_digest: String,
    pub prompt_digest: String,
    pub summary: String,
    pub summary_digest: String,
    pub org_source: String,
    pub org_source_digest: String,
    pub org_ast_digest: String,
    pub overlay_digest: String,
    pub relationships: Vec<AgentTopologyRelationship>,
    pub terminal: AgentOrgTopologyOverlayTerminal,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentOrgTopologyOverlayTerminal {
    pub state: String,
    pub terminal_count: u64,
    pub reason_kind: Option<String>,
}

impl AgentOrgTopologyOverlay {
    /// Admit an Agent-authored Org overlay using a host-supplied Org projection
    /// (for example from Marlin's `marlin-agent-context` / `orgize-fold` parser).
    #[expect(
        clippy::too_many_arguments,
        reason = "the overlay identity product must remain explicit at its admission boundary"
    )]
    pub fn admit_with_org_projection(
        source_generation_digest: impl Into<String>,
        base_topology_generation_digest: impl Into<String>,
        selector: impl Into<String>,
        evidence_digest: impl Into<String>,
        agent_identity_digest: impl Into<String>,
        prompt_digest: impl Into<String>,
        summary: impl Into<String>,
        org_projection: &OrgOverlayDocumentProjection,
        mut relationships: Vec<AgentTopologyRelationship>,
    ) -> Result<Self, AgentOrgTopologyOverlayError> {
        let source_generation_digest = source_generation_digest.into();
        let base_topology_generation_digest = base_topology_generation_digest.into();
        let selector = selector.into();
        let evidence_digest = evidence_digest.into();
        let agent_identity_digest = agent_identity_digest.into();
        let prompt_digest = prompt_digest.into();
        let summary = summary.into();
        let org_source = org_projection.org_source.clone();
        let org_ast_digest = org_projection.org_ast_digest.clone();
        for (field, value) in [
            ("sourceGenerationDigest", source_generation_digest.as_str()),
            (
                "baseTopologyGenerationDigest",
                base_topology_generation_digest.as_str(),
            ),
            ("evidenceDigest", evidence_digest.as_str()),
            ("agentIdentityDigest", agent_identity_digest.as_str()),
            ("promptDigest", prompt_digest.as_str()),
            ("orgAstDigest", org_ast_digest.as_str()),
        ] {
            require_digest(field, value)?;
        }
        if selector.is_empty() || summary.trim().is_empty() || org_source.trim().is_empty() {
            return Err(invalid("agent-org-topology-overlay-content-empty"));
        }
        if relationships.is_empty()
            || relationships.iter().any(|edge| {
                edge.from_selector.is_empty()
                    || edge.to_selector.is_empty()
                    || !valid_relation(&edge.relation)
                    || (edge.from_selector != selector && edge.to_selector != selector)
            })
        {
            return Err(invalid("agent-org-topology-overlay-relationship-invalid"));
        }
        relationships.sort();
        if relationships.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(invalid("agent-org-topology-overlay-relationship-duplicate"));
        }
        if !org_projection.has_headline {
            return Err(invalid("agent-org-topology-overlay-org-ast-empty"));
        }
        let summary_digest = digest(summary.as_bytes());
        let org_source_digest = digest(org_source.as_bytes());
        let identity = serde_json::to_vec(&(
            &source_generation_digest,
            &base_topology_generation_digest,
            &selector,
            &evidence_digest,
            &agent_identity_digest,
            &prompt_digest,
            &summary_digest,
            &org_source_digest,
            &org_ast_digest,
            &relationships,
        ))
        .map_err(|_| invalid("agent-org-topology-overlay-identity-encode-failed"))?;
        let overlay_digest = digest(&identity);
        Ok(Self {
            schema_id: AGENT_ORG_TOPOLOGY_OVERLAY_SCHEMA_ID.to_owned(),
            schema_version: AGENT_ORG_TOPOLOGY_OVERLAY_SCHEMA_VERSION.to_owned(),
            source_generation_digest,
            base_topology_generation_digest,
            selector,
            evidence_digest,
            agent_identity_digest,
            prompt_digest,
            summary,
            summary_digest,
            org_source,
            org_source_digest,
            org_ast_digest,
            overlay_digest,
            relationships,
            terminal: AgentOrgTopologyOverlayTerminal {
                state: "admitted".to_owned(),
                terminal_count: 1,
                reason_kind: None,
            },
        })
    }

    /// Admit an Agent-authored Org overlay through a host-supplied [`OrgDocumentProjectionPort`].
    #[expect(
        clippy::too_many_arguments,
        reason = "the overlay identity product must remain explicit at its admission boundary"
    )]
    pub fn admit_with_port(
        source_generation_digest: impl Into<String>,
        base_topology_generation_digest: impl Into<String>,
        selector: impl Into<String>,
        evidence_digest: impl Into<String>,
        agent_identity_digest: impl Into<String>,
        prompt_digest: impl Into<String>,
        summary: impl Into<String>,
        org_source: impl Into<String>,
        relationships: Vec<AgentTopologyRelationship>,
        port: &dyn OrgDocumentProjectionPort,
    ) -> Result<Self, AgentOrgTopologyOverlayError> {
        let org_source = org_source.into();
        if org_source.trim().is_empty() {
            return Err(invalid("agent-org-topology-overlay-content-empty"));
        }
        let projection = port
            .inspect_overlay(&org_source)
            .map_err(|_| invalid("agent-org-topology-overlay-org-ast-empty"))?;
        Self::admit_with_org_projection(
            source_generation_digest,
            base_topology_generation_digest,
            selector,
            evidence_digest,
            agent_identity_digest,
            prompt_digest,
            summary,
            &projection,
            relationships,
        )
    }

    #[cfg(feature = "embedded-orgize")]
    #[expect(
        clippy::too_many_arguments,
        reason = "the overlay identity product must remain explicit at its admission boundary"
    )]
    pub fn admit(
        source_generation_digest: impl Into<String>,
        base_topology_generation_digest: impl Into<String>,
        selector: impl Into<String>,
        evidence_digest: impl Into<String>,
        agent_identity_digest: impl Into<String>,
        prompt_digest: impl Into<String>,
        summary: impl Into<String>,
        org_source: impl Into<String>,
        relationships: Vec<AgentTopologyRelationship>,
    ) -> Result<Self, AgentOrgTopologyOverlayError> {
        Self::admit_with_port(
            source_generation_digest,
            base_topology_generation_digest,
            selector,
            evidence_digest,
            agent_identity_digest,
            prompt_digest,
            summary,
            org_source,
            relationships,
            &crate::EmbeddedOrgizeProjectionPort,
        )
    }

    /// Recomputes every derived identity using a host-supplied [`OrgDocumentProjectionPort`].
    pub fn validate_with_port(
        &self,
        port: &dyn OrgDocumentProjectionPort,
    ) -> Result<(), AgentOrgTopologyOverlayError> {
        let rebuilt = Self::admit_with_port(
            self.source_generation_digest.clone(),
            self.base_topology_generation_digest.clone(),
            self.selector.clone(),
            self.evidence_digest.clone(),
            self.agent_identity_digest.clone(),
            self.prompt_digest.clone(),
            self.summary.clone(),
            self.org_source.clone(),
            self.relationships.clone(),
            port,
        )?;
        if rebuilt == *self {
            Ok(())
        } else {
            Err(invalid(
                "agent-org-topology-overlay-derived-identity-mismatch",
            ))
        }
    }

    /// Joins the independently built overlay to one exact Search topology
    /// identity using a host-supplied [`OrgDocumentProjectionPort`].
    pub fn validate_against_base_with_port(
        &self,
        expected_source_generation_digest: &str,
        expected_base_topology_generation_digest: &str,
        port: &dyn OrgDocumentProjectionPort,
    ) -> Result<(), AgentOrgTopologyOverlayError> {
        self.validate_with_port(port)?;
        if self.source_generation_digest != expected_source_generation_digest
            || self.base_topology_generation_digest != expected_base_topology_generation_digest
        {
            return Err(invalid("agent-org-topology-overlay-base-binding-mismatch"));
        }
        Ok(())
    }

    /// Recomputes every derived identity. Serialized overlay packets cannot
    /// self-attest by carrying stale summary, Org AST, or overlay digests.
    #[cfg(feature = "embedded-orgize")]
    pub fn validate(&self) -> Result<(), AgentOrgTopologyOverlayError> {
        self.validate_with_port(&crate::EmbeddedOrgizeProjectionPort)
    }

    /// Joins the independently built overlay to one exact Search topology
    /// identity without modifying or reconstructing that base topology.
    #[cfg(feature = "embedded-orgize")]
    pub fn validate_against_base(
        &self,
        expected_source_generation_digest: &str,
        expected_base_topology_generation_digest: &str,
    ) -> Result<(), AgentOrgTopologyOverlayError> {
        self.validate_against_base_with_port(
            expected_source_generation_digest,
            expected_base_topology_generation_digest,
            &crate::EmbeddedOrgizeProjectionPort,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentOrgTopologyOverlayError {
    pub reason_kind: &'static str,
}

impl std::fmt::Display for AgentOrgTopologyOverlayError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "reasonKind={}", self.reason_kind)
    }
}

impl std::error::Error for AgentOrgTopologyOverlayError {}

fn require_digest(field: &'static str, value: &str) -> Result<(), AgentOrgTopologyOverlayError> {
    if value.len() == 75
        && value.starts_with("blake3-256:")
        && value[11..].bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        Ok(())
    } else {
        let _ = field;
        Err(invalid("agent-org-topology-overlay-digest-invalid"))
    }
}

fn valid_relation(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase() || (index > 0 && (byte.is_ascii_digit() || byte == b'-'))
        })
}

fn digest(bytes: &[u8]) -> String {
    format!("blake3-256:{}", blake3::hash(bytes).to_hex())
}

fn invalid(reason_kind: &'static str) -> AgentOrgTopologyOverlayError {
    AgentOrgTopologyOverlayError { reason_kind }
}
