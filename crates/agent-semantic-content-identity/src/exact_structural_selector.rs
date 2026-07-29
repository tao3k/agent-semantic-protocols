use serde::{Deserialize, Serialize};
use std::fmt;

pub const EXACT_STRUCTURAL_SELECTOR_SCHEMA_ID: &str = "asp.exact-structural-selector.v1";
pub const EXACT_STRUCTURAL_SELECTOR_SCHEMA_VERSION: &str = "1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanonicalItemSelector {
    pub schema_id: String,
    pub schema_version: String,
    pub language_id: String,
    pub kind: String,
    pub symbol: String,
    pub scopes: Vec<CanonicalItemSelectorScopeV1>,
    pub structural_selector: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanonicalItemSelectorScopeV1 {
    pub relation: String,
    pub kind: String,
    pub symbol: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExactStructuralSelectorSegmentV1 {
    pub relation: String,
    pub kind: String,
    pub identity: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExactStructuralSelectorV1 {
    pub schema_id: String,
    pub schema_version: String,
    pub language_id: String,
    pub owner_path: String,
    pub selector: String,
    pub generation_identity_digest: String,
    pub parser_identity_digest: String,
    pub query_pack_digest: String,
    pub root_item_selector: CanonicalItemSelector,
    pub segments: Vec<ExactStructuralSelectorSegmentV1>,
}

impl ExactStructuralSelectorV1 {
    pub fn validate(&self) -> Result<(), ExactStructuralSelectorValidationError> {
        if self.schema_id != EXACT_STRUCTURAL_SELECTOR_SCHEMA_ID {
            return Err(ExactStructuralSelectorValidationError::SchemaId);
        }
        if self.schema_version != EXACT_STRUCTURAL_SELECTOR_SCHEMA_VERSION {
            return Err(ExactStructuralSelectorValidationError::SchemaVersion);
        }
        if self.language_id.is_empty() {
            return Err(ExactStructuralSelectorValidationError::EmptyField(
                "languageId",
            ));
        }
        if self.owner_path.is_empty() || self.owner_path.starts_with('/') {
            return Err(ExactStructuralSelectorValidationError::OwnerPath);
        }
        if self.selector.is_empty() {
            return Err(ExactStructuralSelectorValidationError::EmptyField(
                "selector",
            ));
        }
        validate_digest("generationIdentityDigest", &self.generation_identity_digest)?;
        validate_digest("parserIdentityDigest", &self.parser_identity_digest)?;
        validate_digest("queryPackDigest", &self.query_pack_digest)?;
        self.validate_root_item()?;
        for (index, segment) in self.segments.iter().enumerate() {
            if segment.relation.is_empty() || segment.kind.is_empty() {
                return Err(ExactStructuralSelectorValidationError::EmptySegmentField { index });
            }
            if segment.identity.is_empty() || looks_like_source_location(&segment.identity) {
                return Err(
                    ExactStructuralSelectorValidationError::InvalidSegmentIdentity { index },
                );
            }
        }
        Ok(())
    }

    fn validate_root_item(&self) -> Result<(), ExactStructuralSelectorValidationError> {
        let root = &self.root_item_selector;
        if root.schema_id != "asp.canonical-item-selector.v1" || root.schema_version != "1" {
            return Err(ExactStructuralSelectorValidationError::RootItemContract);
        }
        if root.language_id != self.language_id {
            return Err(ExactStructuralSelectorValidationError::RootLanguage);
        }
        if root.kind.is_empty() || root.symbol.is_empty() || root.structural_selector.is_empty() {
            return Err(ExactStructuralSelectorValidationError::RootItemContract);
        }
        if root.scopes.iter().any(|scope| {
            scope.relation.is_empty() || scope.kind.is_empty() || scope.symbol.is_empty()
        }) {
            return Err(ExactStructuralSelectorValidationError::RootItemContract);
        }
        Ok(())
    }

    pub fn shares_projection_identity_with(&self, other: &Self) -> bool {
        self.language_id == other.language_id
            && self.owner_path == other.owner_path
            && self.generation_identity_digest == other.generation_identity_digest
            && self.parser_identity_digest == other.parser_identity_digest
            && self.query_pack_digest == other.query_pack_digest
            && self.root_item_selector == other.root_item_selector
    }
}

fn validate_digest(
    field: &'static str,
    value: &str,
) -> Result<(), ExactStructuralSelectorValidationError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(ExactStructuralSelectorValidationError::Digest { field })
    }
}

fn looks_like_source_location(identity: &str) -> bool {
    let normalized = identity.to_ascii_lowercase();
    normalized.bytes().all(|byte| byte.is_ascii_digit())
        || ["line:", "lines:", "byte:", "bytes:", "offset:"]
            .iter()
            .any(|prefix| normalized.starts_with(prefix))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExactStructuralSelectorValidationError {
    SchemaId,
    SchemaVersion,
    EmptyField(&'static str),
    OwnerPath,
    Digest { field: &'static str },
    RootItemContract,
    RootLanguage,
    EmptySegmentField { index: usize },
    InvalidSegmentIdentity { index: usize },
}

impl fmt::Display for ExactStructuralSelectorValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ExactStructuralSelectorValidationError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn selector() -> ExactStructuralSelectorV1 {
        ExactStructuralSelectorV1 {
            schema_id: EXACT_STRUCTURAL_SELECTOR_SCHEMA_ID.to_owned(),
            schema_version: EXACT_STRUCTURAL_SELECTOR_SCHEMA_VERSION.to_owned(),
            language_id: "rust".to_owned(),
            owner_path: "crates/example/src/lib.rs".to_owned(),
            selector: "rust://crates/example/src/lib.rs#node/function/run/identity=root".to_owned(),
            generation_identity_digest: "a".repeat(64),
            parser_identity_digest: "b".repeat(64),
            query_pack_digest: "c".repeat(64),
            root_item_selector: CanonicalItemSelector {
                schema_id: "asp.canonical-item-selector.v1".to_owned(),
                schema_version: "1".to_owned(),
                language_id: "rust".to_owned(),
                kind: "function".to_owned(),
                symbol: "run".to_owned(),
                scopes: Vec::new(),
                structural_selector: "rust://crates/example/src/lib.rs#item/function/run"
                    .to_owned(),
            },
            segments: Vec::new(),
        }
    }

    #[test]
    fn validates_generation_bound_root_selector() {
        selector().validate().expect("selector should be valid");
    }

    #[test]
    fn rejects_line_based_segment_identity() {
        let mut value = selector();
        value.segments.push(ExactStructuralSelectorSegmentV1 {
            relation: "contains".to_owned(),
            kind: "arm".to_owned(),
            identity: "line:42".to_owned(),
            label: None,
        });
        assert_eq!(
            value.validate(),
            Err(ExactStructuralSelectorValidationError::InvalidSegmentIdentity { index: 0 })
        );
    }
}
