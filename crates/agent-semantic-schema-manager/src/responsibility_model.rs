// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Pure V1 Schema responsibility values shared by registry and audit owners.

use serde::Deserialize;
use serde::Serialize;

/// Contractual owner family for a canonical Schema namespace.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SchemaFamily {
    pub family_id: String,
    pub owner: String,
    pub rationale: String,
    #[serde(default)]
    pub priority: i64,
    #[serde(default)]
    pub parent_family_id: Option<String>,
    pub namespace: SchemaFamilyNamespace,
    #[serde(default)]
    pub membership_overrides: SchemaFamilyMembershipOverrides,
    #[serde(default)]
    pub definition_schema_path: Option<String>,
    #[serde(default)]
    pub definition_visibility: Option<String>,
}

/// Filename and Schema identifier selectors owned by one family.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SchemaFamilyNamespace {
    #[serde(default)]
    pub filename_prefixes: Vec<String>,
    #[serde(default)]
    pub schema_identifier_prefixes: Vec<String>,
}

/// Explicit family membership exceptions for schemas predating a namespace.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SchemaFamilyMembershipOverrides {
    #[serde(default)]
    pub include_schema_paths: Vec<String>,
    #[serde(default)]
    pub exclude_schema_paths: Vec<String>,
}

/// Fully resolved responsibility for one canonical Schema.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaResponsibility {
    pub name: String,
    pub schema_id: String,
    pub family_id: String,
    pub owner: String,
    pub purpose: String,
}

/// Accepted decision for structurally equal shapes owned by different families.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SchemaReferenceDecision {
    pub fingerprint: String,
    pub occurrence_set_digest: String,
    pub family_ids: Vec<String>,
    pub decision: String,
    pub review_state: String,
    pub owner: String,
    pub rationale: String,
}
