//! Typed callable skeleton projections bound to exact structural selectors.

use crate::exact_structural_selector::{
    ExactStructuralSelectorV1, ExactStructuralSelectorValidationError,
};
use crate::projection_evidence_context::{
    ExactStructuralSelectorReferenceV1, ProjectionEvidenceContextValidationError,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::fmt;

/// Stable schema identity for callable-skeleton projection payloads.
pub const CALLABLE_SKELETON_PAYLOAD_SCHEMA_ID: &str = "agent.semantic-protocols.callable-skeleton";

/// Provider-defined callable category carried as a validated value object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CallableDescriptorKindV1(String);

impl CallableDescriptorKindV1 {
    /// Creates a callable category, rejecting an empty provider value.
    pub fn new(value: impl Into<String>) -> Result<Self, CallableSkeletonValidationError> {
        let value = value.into();
        if value.is_empty() {
            return Err(CallableSkeletonValidationError::EmptyRequiredField);
        }
        Ok(Self(value))
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Raw DTO boundary for one provider callable descriptor in schema version 1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CallableDescriptorV1 {
    pub kind: CallableDescriptorKindV1,
    pub display_name: String,
    pub signature: String,
}

/// Closed node-category catalog for a callable control-flow skeleton.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CallableSkeletonNodeKindV1 {
    Callable,
    Branch,
    Arm,
    Loop,
    Exception,
    ResourceScope,
    Invocation,
    Binding,
    Exit,
    Suspension,
    NestedDeclaration,
    LanguageExtension,
}

/// Raw DTO boundary for optional display and byte locations in provider source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceLocatorHintV1 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_line_start: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_line_end: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_byte_start: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_byte_end: Option<u64>,
}

/// Raw DTO boundary for one typed node in a callable skeleton projection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CallableSkeletonNodeV1 {
    pub node_id: String,
    pub kind: CallableSkeletonNodeKindV1,
    pub label: String,
    pub order: u64,
    pub queryable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_locator_hint: Option<SourceLocatorHintV1>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub language_facts: BTreeMap<String, Value>,
}

/// Full provider admission identity or compact Runtime resident reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CallableSkeletonRootSelectorV1 {
    Inline(ExactStructuralSelectorV1),
    Referenced(ExactStructuralSelectorReferenceV1),
}

impl CallableSkeletonRootSelectorV1 {
    /// Returns the exact structural selector represented by either wire form.
    pub fn selector(&self) -> &str {
        match self {
            Self::Inline(selector) => &selector.selector,
            Self::Referenced(selector) => &selector.selector,
        }
    }
}

/// Provider-defined relation category carried as a validated value object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CallableSkeletonRelationKindV1(String);

impl CallableSkeletonRelationKindV1 {
    /// Creates a relation category, rejecting an empty provider value.
    pub fn new(value: impl Into<String>) -> Result<Self, CallableSkeletonValidationError> {
        let value = value.into();
        if value.is_empty() {
            return Err(CallableSkeletonValidationError::InvalidRelation);
        }
        Ok(Self(value))
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Raw DTO boundary for a directed relation between two skeleton nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CallableSkeletonRelationV1 {
    pub from_node_id: String,
    pub to_node_id: String,
    pub kind: CallableSkeletonRelationKindV1,
}

/// Raw DTO boundary for byte and token accounting in schema version 1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CallableSkeletonCostV1 {
    pub source_bytes: u64,
    pub projected_bytes: u64,
    pub omitted_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_source_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_projected_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_estimator: Option<String>,
}

/// Raw DTO boundary for a resolved native exact callable projection.
///
/// The schema owns the wire version. Rust callers validate every authority
/// field independently so transport drift cannot collapse into an opaque
/// identity-mismatch error.
/// Projection mode selected by the provider-native exact projection boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProviderNativeProjectionModeV1(String);

impl ProviderNativeProjectionModeV1 {
    /// Creates a non-empty provider-native projection mode.
    pub fn new(value: impl Into<String>) -> Result<Self, CallableSkeletonValidationError> {
        let value = value.into();
        if value.is_empty() {
            return Err(CallableSkeletonValidationError::EmptyRequiredField);
        }
        Ok(Self(value))
    }

    fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Provider-native exact projection encoded at the stable JSON wire boundary.
///
/// Raw DTO boundary: the identity strings preserve provider-owned schema values;
/// semantic validation occurs before this projection is admitted.
pub struct ProviderNativeExactProjection {
    pub schema_id: String,
    pub schema_version: String,
    pub language_id: String,
    pub provider_id: String,
    pub owner_path: String,
    pub requested_structural_selector: String,
    pub structural_selector: String,
    pub projection_mode: ProviderNativeProjectionModeV1,
    pub normalized_parser_facts: Value,
    pub projection_payload: CallableSkeletonPayload,
    pub source_content_digest: String,
    pub source_byte_start: u64,
    pub source_byte_end: u64,
}

/// Semantic field boundary for expected provider-native response authority.
#[derive(Debug, Clone, Copy)]
pub struct ProviderNativeExactAuthority<'a> {
    pub language_id: &'a str,
    pub provider_id: &'a str,
    pub owner_path: &'a str,
    pub requested_structural_selector: &'a str,
}

impl ProviderNativeExactProjection {
    /// Validates the response envelope and reports the exact drifting field.
    pub fn validate_authority(
        &self,
        expected: ProviderNativeExactAuthority<'_>,
    ) -> Result<(), String> {
        for (field, actual, expected) in [
            (
                "schemaId",
                self.schema_id.as_str(),
                "agent.semantic-protocols.provider-native-exact-projection",
            ),
            ("schemaVersion", self.schema_version.as_str(), "1"),
            (
                "languageId",
                self.language_id.as_str(),
                expected.language_id,
            ),
            (
                "providerId",
                self.provider_id.as_str(),
                expected.provider_id,
            ),
            ("ownerPath", self.owner_path.as_str(), expected.owner_path),
            (
                "requestedStructuralSelector",
                self.requested_structural_selector.as_str(),
                expected.requested_structural_selector,
            ),
            (
                "projectionMode",
                self.projection_mode.as_str(),
                "callable-skeleton",
            ),
        ] {
            if actual != expected {
                return Err(format!(
                    "provider-native exact response {field} mismatch: expected={expected} actual={actual}"
                ));
            }
        }
        if self.source_byte_start > self.source_byte_end {
            return Err("provider-native exact response source byte range is inverted".to_owned());
        }
        Ok(())
    }
}

/// Raw DTO boundary for the complete callable-skeleton payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CallableSkeletonPayload {
    pub root_node_id: String,
    pub callable: CallableDescriptorV1,
    pub nodes: Vec<CallableSkeletonNodeV1>,
    pub relations: Vec<CallableSkeletonRelationV1>,
    pub cost: CallableSkeletonCostV1,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub omission_reasons: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub language_facts: BTreeMap<String, Value>,
}

#[cfg(test)]
#[path = "../tests/unit/callable_skeleton_projection/provider_contract.rs"]
mod provider_contract_tests;

impl CallableSkeletonPayload {
    /// Returns the provider projection mode requested from native parsers.
    pub const fn projection_mode() -> &'static str {
        "skeleton"
    }

    /// Returns the language-neutral semantic projection kind.
    pub const fn projection_kind() -> &'static str {
        "callable-skeleton"
    }

    /// Validates node identity, selector scope, relations, and cost accounting.
    pub fn validate(&self) -> Result<(), CallableSkeletonValidationError> {
        self.validate_payload()
    }

    fn validate_payload(&self) -> Result<(), CallableSkeletonValidationError> {
        if self.root_node_id.is_empty() || self.callable.kind.is_empty() {
            return Err(CallableSkeletonValidationError::EmptyRequiredField);
        }
        if self.nodes.is_empty() {
            return Err(CallableSkeletonValidationError::EmptyNodes);
        }

        let mut node_ids = HashSet::with_capacity(self.nodes.len());
        for node in &self.nodes {
            if node.node_id.is_empty() || !node_ids.insert(node.node_id.as_str()) {
                return Err(CallableSkeletonValidationError::DuplicateOrEmptyNodeId(
                    node.node_id.clone(),
                ));
            }
            match (
                node.queryable,
                node.selector.as_deref(),
                node.selector_ref.as_deref(),
            ) {
                (true, Some(selector), None) => self.validate_child_selector(node, selector)?,
                (true, None, Some(selector_ref)) => {
                    self.validate_child_selector_ref(node, selector_ref)?
                }
                (true, None, None) => {
                    return Err(CallableSkeletonValidationError::MissingChildSelector(
                        node.node_id.clone(),
                    ));
                }
                (true, Some(_), Some(_)) => {
                    return Err(CallableSkeletonValidationError::AmbiguousChildSelector(
                        node.node_id.clone(),
                    ));
                }
                (false, Some(_), _) | (false, _, Some(_)) => {
                    return Err(CallableSkeletonValidationError::UnexpectedChildSelector(
                        node.node_id.clone(),
                    ));
                }
                (false, None, None) => {}
            }
        }
        if !node_ids.contains(self.root_node_id.as_str()) {
            return Err(CallableSkeletonValidationError::MissingRootNode);
        }
        for relation in &self.relations {
            if relation.kind.is_empty()
                || !node_ids.contains(relation.from_node_id.as_str())
                || !node_ids.contains(relation.to_node_id.as_str())
            {
                return Err(CallableSkeletonValidationError::InvalidRelation);
            }
        }
        if self
            .cost
            .source_bytes
            .saturating_sub(self.cost.projected_bytes)
            != self.cost.omitted_bytes
        {
            return Err(CallableSkeletonValidationError::CostAccounting);
        }
        match (
            self.cost.estimated_source_tokens,
            self.cost.estimated_projected_tokens,
            self.cost.token_estimator.as_deref(),
        ) {
            (None, None, None) => {}
            (Some(_), Some(_), Some(estimator)) if !estimator.is_empty() => {}
            _ => return Err(CallableSkeletonValidationError::TokenEstimate),
        }
        Ok(())
    }

    /// Validates that every queryable node stays within the requested root.
    pub fn validate_scope(
        &self,
        root_selector: &str,
    ) -> Result<(), CallableSkeletonValidationError> {
        if root_selector.trim().is_empty() {
            return Err(CallableSkeletonValidationError::EmptyRequiredField);
        }
        let descendant_prefix = format!("{root_selector}/");
        for node in &self.nodes {
            if !node.queryable {
                continue;
            }
            if let Some(selector) = node.selector.as_deref()
                && selector != root_selector
                && !selector.starts_with(&descendant_prefix)
            {
                return Err(CallableSkeletonValidationError::ChildSelectorContext(
                    node.node_id.clone(),
                ));
            }
            if let Some(selector_ref) = node.selector_ref.as_deref()
                && selector_ref != "$root"
                && !selector_ref.starts_with("$root/")
            {
                return Err(CallableSkeletonValidationError::ChildSelectorContext(
                    node.node_id.clone(),
                ));
            }
        }
        Ok(())
    }

    fn validate_child_selector(
        &self,
        node: &CallableSkeletonNodeV1,
        selector: &str,
    ) -> Result<(), CallableSkeletonValidationError> {
        if selector.is_empty() {
            return Err(CallableSkeletonValidationError::ChildSelectorContext(
                node.node_id.clone(),
            ));
        }
        Ok(())
    }

    fn validate_child_selector_ref(
        &self,
        node: &CallableSkeletonNodeV1,
        selector_ref: &str,
    ) -> Result<(), CallableSkeletonValidationError> {
        if selector_ref == "$root" {
            return Ok(());
        }
        let Some(path) = selector_ref.strip_prefix("$root/segment/") else {
            return Err(CallableSkeletonValidationError::ChildSelectorContext(
                node.node_id.clone(),
            ));
        };
        let mut components = path.split('/');
        while let Some(kind) = components.next() {
            let Some(identity) = components.next() else {
                return Err(CallableSkeletonValidationError::ChildSelectorContext(
                    node.node_id.clone(),
                ));
            };
            if kind.is_empty() || identity.is_empty() {
                return Err(CallableSkeletonValidationError::ChildSelectorContext(
                    node.node_id.clone(),
                ));
            }
        }
        Ok(())
    }

    /// Encodes a validated payload as standard Base64 JSON bytes.
    pub fn encode_payload_base64(&self) -> Result<String, CallableSkeletonEncodingError> {
        self.validate()
            .map_err(CallableSkeletonEncodingError::Validation)?;
        let payload = serde_json::to_vec(self).map_err(CallableSkeletonEncodingError::Json)?;
        Ok(STANDARD.encode(payload))
    }
}

/// Typed failures produced while validating a callable skeleton projection.
#[derive(Debug)]
pub enum CallableSkeletonValidationError {
    SchemaId,
    SchemaVersion,
    ProjectionKind,
    EmptyRequiredField,
    RootSelector(ExactStructuralSelectorValidationError),
    EvidenceContext(ProjectionEvidenceContextValidationError),
    RootLanguage,
    EmptyNodes,
    DuplicateOrEmptyNodeId(String),
    MissingRootNode,
    MissingChildSelector(String),
    AmbiguousChildSelector(String),
    UnexpectedChildSelector(String),
    ChildSelectorContext(String),
    InvalidRelation,
    CostAccounting,
    TokenEstimate,
    AlreadyReferenced,
    CostEncoding,
}

impl fmt::Display for CallableSkeletonValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CallableSkeletonValidationError {}

/// Typed failures produced while encoding a callable skeleton payload.
#[derive(Debug)]
pub enum CallableSkeletonEncodingError {
    Validation(CallableSkeletonValidationError),
    Json(serde_json::Error),
}

impl fmt::Display for CallableSkeletonEncodingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CallableSkeletonEncodingError {}

#[cfg(test)]
#[path = "../tests/unit/callable_skeleton_projection/validation.rs"]
mod tests;
