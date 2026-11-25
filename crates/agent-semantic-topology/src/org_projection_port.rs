// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Pure-Rust Org document projection port for Marlin-native convergence.
//!
//! Marlin (`marlin-agent-context` / `orgize-fold`) owns Org parsing once in its
//! host runtime and feeds typed [`OrgDocumentProjection`] and
//! [`OrgOverlayDocumentProjection`] values into ASP's search and topology
//! engine without embedding a second Orgize/Gambit FFI runtime.

use std::fmt;

use serde::{Deserialize, Serialize};

/// One key/value property entry projected from an Org property drawer.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OrgPropertyEntry {
    pub key: String,
    pub value: String,
}

impl OrgPropertyEntry {
    #[must_use]
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

/// One headline section projected from an Org document tree.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OrgSectionProjection {
    pub level: u16,
    pub parent_index: Option<usize>,
    pub properties: Vec<OrgPropertyEntry>,
}

impl OrgSectionProjection {
    #[must_use]
    pub fn new(level: u16, parent_index: Option<usize>, properties: Vec<OrgPropertyEntry>) -> Self {
        Self {
            level,
            parent_index,
            properties,
        }
    }
}

/// Parser-neutral Org document projection accepted by ASP topology loaders.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OrgDocumentProjection {
    pub source_digest: String,
    pub document_properties: Vec<OrgPropertyEntry>,
    pub sections: Vec<OrgSectionProjection>,
}

impl OrgDocumentProjection {
    #[must_use]
    pub fn new(
        source_digest: impl Into<String>,
        document_properties: Vec<OrgPropertyEntry>,
        sections: Vec<OrgSectionProjection>,
    ) -> Self {
        Self {
            source_digest: source_digest.into(),
            document_properties,
            sections,
        }
    }

    #[must_use]
    pub fn digest_for_source(source: &str) -> String {
        format!("blake3-256:{}", blake3::hash(source.as_bytes()).to_hex())
    }

    /// Return true when `section_index` is strictly nested under `ancestor_index`.
    #[must_use]
    pub fn is_descendant_of(&self, section_index: usize, ancestor_index: usize) -> bool {
        let mut current = self
            .sections
            .get(section_index)
            .and_then(|section| section.parent_index);
        let mut hops = 0usize;
        while let Some(parent) = current {
            if parent == ancestor_index {
                return true;
            }
            hops += 1;
            if hops > self.sections.len() {
                return false;
            }
            current = self
                .sections
                .get(parent)
                .and_then(|section| section.parent_index);
        }
        false
    }
}

/// Parser-neutral Org overlay inspection result fed by Marlin's Org parser.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OrgOverlayDocumentProjection {
    pub org_source: String,
    pub org_ast_digest: String,
    pub has_headline: bool,
}

impl OrgOverlayDocumentProjection {
    #[must_use]
    pub fn new(
        org_source: impl Into<String>,
        org_ast_digest: impl Into<String>,
        has_headline: bool,
    ) -> Self {
        Self {
            org_source: org_source.into(),
            org_ast_digest: org_ast_digest.into(),
            has_headline,
        }
    }
}

/// Deterministic error returned by an [`OrgDocumentProjectionPort`] adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrgProjectionPortError {
    pub reason_kind: &'static str,
    pub message: String,
}

impl OrgProjectionPortError {
    #[must_use]
    pub fn new(reason_kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            reason_kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for OrgProjectionPortError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.reason_kind, self.message)
    }
}

impl std::error::Error for OrgProjectionPortError {}

/// Host-supplied Org parsing port implemented by Marlin (`marlin-agent-context`).
pub trait OrgDocumentProjectionPort: Send + Sync {
    /// Project an Org document into document-level and section-level property drawers.
    fn project_document(
        &self,
        source: &str,
    ) -> Result<OrgDocumentProjection, OrgProjectionPortError>;

    /// Inspect an Agent-authored Org topology overlay for headlines and AST digest.
    fn inspect_overlay(
        &self,
        source: &str,
    ) -> Result<OrgOverlayDocumentProjection, OrgProjectionPortError>;
}
