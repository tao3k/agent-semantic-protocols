// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Pure V1 registry and bundle value types shared by Schema Manager owners.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

use crate::responsibility_model::SchemaFamily;
use crate::responsibility_model::SchemaReferenceDecision;

/// Canonical language profile registry schema identity.
pub const PROFILE_REGISTRY_SCHEMA_ID: &str =
    "agent.semantic-protocols.language-schema-profile-registry";
/// Public language bundle receipt schema identity.
pub const BUNDLE_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.language-schema-bundle-receipt";
/// Stable Schema Manager contract version.
pub const SCHEMA_VERSION: &str = "1";
/// Workspace-relative canonical language profile registry.
pub const DEFAULT_PROFILE_REGISTRY: &str = "schemas/language-schema-profiles.json";
/// Public receipt filename emitted beside a projected bundle.
pub const BUNDLE_RECEIPT_FILE: &str = ".asp-schema-manager-receipt.json";

/// Canonical registry describing language roots and responsibility families.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LanguageSchemaProfileRegistry {
    #[serde(rename = "$schema")]
    pub schema: String,
    pub schema_id: String,
    pub schema_version: String,
    pub families: Vec<SchemaFamily>,
    pub reference_decisions: Vec<SchemaReferenceDecision>,
    pub wire_artifacts: BTreeMap<String, String>,
    pub root_sets: BTreeMap<String, Vec<String>>,
    pub profiles: Vec<LanguageSchemaProfile>,
}

/// One language's canonical Schema closure and thin bootstrap projection.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LanguageSchemaProfile {
    pub language_id: String,
    pub search_producer_axes: Vec<SearchProducerAxis>,
    pub package_root: String,
    pub bundle_root: String,
    pub root_sets: Vec<String>,
    pub roots: Vec<String>,
    #[serde(default)]
    pub bootstrap: Vec<String>,
    pub provider_owned: Vec<String>,
}

/// Search producer namespace supported by a language profile.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SearchProducerAxis {
    Language,
    Document,
}

/// Content-bound identity of one document in a resolved bundle.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SchemaBundleEntry {
    pub name: String,
    pub digest: String,
}

/// Public V1 receipt plus manager-private membership fields restored on load.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LanguageSchemaBundleReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub schema_digest: String,
    #[serde(skip)]
    pub language_id: String,
    #[serde(skip)]
    pub profile_digest: String,
    #[serde(skip)]
    pub bundle_digest: String,
    #[serde(skip)]
    pub schemas: Vec<SchemaBundleEntry>,
}

/// Materialization or verification summary for one language bundle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaBundleReport {
    pub language_id: String,
    pub schema_count: usize,
    pub changed_count: usize,
    pub removed_count: usize,
    pub receipt_path: PathBuf,
    pub bundle_digest: String,
}

/// One canonical schema document resolved without package-local replication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedSchemaDocument {
    pub name: String,
    pub digest: String,
    pub bytes: Vec<u8>,
}

/// Immutable language bundle input for build-time consumers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedLanguageSchemaBundle {
    pub language_id: String,
    pub root_set_ids: Vec<String>,
    pub bundle_digest: String,
    pub schemas: Vec<ResolvedSchemaDocument>,
}
