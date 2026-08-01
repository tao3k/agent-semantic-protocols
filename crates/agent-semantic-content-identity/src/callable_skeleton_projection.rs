use crate::exact_structural_selector::{
    ExactStructuralSelectorV1, ExactStructuralSelectorValidationError,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::fmt;

pub const CALLABLE_SKELETON_PROJECTION_SCHEMA_ID: &str =
    "agent.semantic-protocols.callable-skeleton-projection";
pub const CALLABLE_SKELETON_PROJECTION_SCHEMA_VERSION: &str = "1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CallableDescriptorV1 {
    pub kind: String,
    pub display_name: String,
    pub signature: String,
}

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CallableSkeletonNodeV1 {
    pub node_id: String,
    pub kind: CallableSkeletonNodeKindV1,
    pub label: String,
    pub order: u64,
    pub queryable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exact_selector: Option<ExactStructuralSelectorV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_locator_hint: Option<SourceLocatorHintV1>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub language_facts: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CallableSkeletonRelationV1 {
    pub from_node_id: String,
    pub to_node_id: String,
    pub kind: String,
}

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CallableSkeletonProjectionV1 {
    pub schema_id: String,
    pub schema_version: String,
    pub projection_kind: String,
    pub language_id: String,
    pub provider_id: String,
    pub root_selector: ExactStructuralSelectorV1,
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

impl CallableSkeletonProjectionV1 {
    pub const fn projection_mode() -> &'static str {
        "skeleton"
    }

    pub fn validate(&self) -> Result<(), CallableSkeletonValidationError> {
        if self.schema_id != CALLABLE_SKELETON_PROJECTION_SCHEMA_ID {
            return Err(CallableSkeletonValidationError::SchemaId);
        }
        if self.schema_version != CALLABLE_SKELETON_PROJECTION_SCHEMA_VERSION {
            return Err(CallableSkeletonValidationError::SchemaVersion);
        }
        if self.projection_kind != "callable-skeleton" {
            return Err(CallableSkeletonValidationError::ProjectionKind);
        }
        if self.language_id.is_empty()
            || self.provider_id.is_empty()
            || self.root_node_id.is_empty()
            || self.callable.kind.is_empty()
        {
            return Err(CallableSkeletonValidationError::EmptyRequiredField);
        }
        self.root_selector
            .validate()
            .map_err(CallableSkeletonValidationError::RootSelector)?;
        if self.root_selector.language_id != self.language_id {
            return Err(CallableSkeletonValidationError::RootLanguage);
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
            match (node.queryable, node.exact_selector.as_ref()) {
                (true, Some(selector)) => {
                    selector
                        .validate()
                        .map_err(CallableSkeletonValidationError::ChildSelector)?;
                    if !self.root_selector.shares_projection_identity_with(selector) {
                        return Err(CallableSkeletonValidationError::ChildSelectorContext(
                            node.node_id.clone(),
                        ));
                    }
                }
                (true, None) => {
                    return Err(CallableSkeletonValidationError::MissingChildSelector(
                        node.node_id.clone(),
                    ));
                }
                (false, Some(_)) => {
                    return Err(CallableSkeletonValidationError::UnexpectedChildSelector(
                        node.node_id.clone(),
                    ));
                }
                (false, None) => {}
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
        if self.cost.projected_bytes > self.cost.source_bytes
            || self.cost.source_bytes - self.cost.projected_bytes != self.cost.omitted_bytes
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

    pub fn encode_payload_base64(&self) -> Result<String, CallableSkeletonEncodingError> {
        self.validate()
            .map_err(CallableSkeletonEncodingError::Validation)?;
        let payload = serde_json::to_vec(self).map_err(CallableSkeletonEncodingError::Json)?;
        Ok(STANDARD.encode(payload))
    }
}

#[derive(Debug)]
pub enum CallableSkeletonValidationError {
    SchemaId,
    SchemaVersion,
    ProjectionKind,
    EmptyRequiredField,
    RootSelector(ExactStructuralSelectorValidationError),
    ChildSelector(ExactStructuralSelectorValidationError),
    RootLanguage,
    EmptyNodes,
    DuplicateOrEmptyNodeId(String),
    MissingRootNode,
    MissingChildSelector(String),
    UnexpectedChildSelector(String),
    ChildSelectorContext(String),
    InvalidRelation,
    CostAccounting,
    TokenEstimate,
}

impl fmt::Display for CallableSkeletonValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CallableSkeletonValidationError {}

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
