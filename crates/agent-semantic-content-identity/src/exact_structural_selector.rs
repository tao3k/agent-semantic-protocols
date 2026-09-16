// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Provider-neutral exact structural-selector wire identities and validation.

use serde::Deserialize;
use serde::Serialize;
use std::fmt;

/// Schema identifier for exact structural selectors.
pub const EXACT_STRUCTURAL_SELECTOR_SCHEMA_ID: &str = "asp.exact-structural-selector.v1";
/// Schema version for exact structural selectors.
pub const EXACT_STRUCTURAL_SELECTOR_SCHEMA_VERSION: &str = "1";

/// Provider DTO for the canonical root item embedded in an exact selector.
///
/// Typed catalog boundary: item kinds remain provider-owned syntax vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExactCanonicalItemSelectorV1 {
    pub schema_id: String,
    pub schema_version: String,
    pub language_id: String,
    pub kind: String,
    pub symbol: String,
    pub scopes: Vec<CanonicalItemSelectorScopeV1>,
    pub structural_selector: String,
}

/// One lexical scope in an exact canonical item selector.
///
/// Typed catalog boundary: scope item kinds remain provider-owned syntax vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanonicalItemSelectorScopeV1 {
    pub relation: String,
    pub kind: String,
    pub symbol: String,
}

/// One exact descendant segment following the canonical root item.
///
/// Typed catalog boundary: segment kinds remain provider-owned syntax vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExactStructuralSelectorSegmentV1 {
    pub relation: String,
    pub kind: String,
    pub identity: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// Lossless V1 identity parsed from an exact structural-selector request.
/// One parsed exact descendant path segment.
///
/// Typed catalog boundary: segment kinds remain provider-owned syntax vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactStructuralSelectorPathV1 {
    pub selector: String,
    pub root_selector: String,
    pub segments: Vec<ExactStructuralSelectorPathSegmentV1>,
}

/// One parsed exact descendant path segment.
///
/// Typed catalog boundary: segment kinds remain provider-owned syntax vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactStructuralSelectorPathSegmentV1 {
    pub kind: String,
    pub identity: String,
}

impl ExactStructuralSelectorPathV1 {
    /// Parses a canonical root selector followed by exact descendant segments.
    pub fn parse(structural_selector: impl Into<String>) -> Result<Self, String> {
        let selector = structural_selector.into();
        let mut parts = selector.split("/segment/");
        let root = parts.next().unwrap_or_default().to_owned();
        if root.is_empty() || !root.contains("://") || !root.contains('#') {
            return Err(
                "exact descendant structuralSelector must include a canonical root item"
                    .to_string(),
            );
        }
        let mut segments = Vec::new();
        for descendant in parts {
            let (kind, identity) = descendant.split_once('/').ok_or_else(|| {
                "exact descendant structuralSelector segment must include <kind>/<identity>"
                    .to_string()
            })?;
            if kind.is_empty() || identity.is_empty() || identity.contains('/') {
                return Err(
                    "exact descendant structuralSelector segment must be a canonical <kind>/<identity> pair"
                        .to_string(),
                );
            }
            segments.push(ExactStructuralSelectorPathSegmentV1 {
                kind: kind.to_owned(),
                identity: identity.to_owned(),
            });
        }
        Ok(Self {
            selector,
            root_selector: root,
            segments,
        })
    }
}

/// Raw DTO boundary for a complete exact structural-selector receipt.
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
    pub root_item_selector: ExactCanonicalItemSelectorV1,
    pub segments: Vec<ExactStructuralSelectorSegmentV1>,
}

impl ExactStructuralSelectorV1 {
    /// Validates schema identity, content digests, root binding, and descendant segments.
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

    /// Returns whether two selectors bind the same projection identity.
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

/// Typed validation failures for exact structural selectors.
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
#[path = "../tests/unit/exact_structural_selector.rs"]
mod tests;
