// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Canonical schema closure resolution and content-addressed bundle publication.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::responsibility::SchemaFamily;
use crate::responsibility::SchemaReferenceDecision;
use crate::responsibility::SchemaResponsibility;
use crate::responsibility::audit_schema_responsibilities;

use crate::manager_validation::ensure_unique;
use crate::manager_validation::local_schema_name;
use crate::manager_validation::schema_references;
use crate::manager_validation::validate_identity;
use crate::manager_validation::validate_relative_path;
use crate::manager_validation::validate_schema_name;
use crate::receipt::BUNDLE_MEMBERSHIP_FILE;
use crate::receipt::SchemaBundleMembership;
use crate::receipt::read_receipt_if_present;
use crate::receipt::schema_digest;
use crate::receipt::tagged_content_digest;
use crate::receipt::verify_bundle_receipt_blocking;

pub const PROFILE_REGISTRY_SCHEMA_ID: &str =
    "agent.semantic-protocols.language-schema-profile-registry";
pub const BUNDLE_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.language-schema-bundle-receipt";
pub const SCHEMA_VERSION: &str = "1";
pub const DEFAULT_PROFILE_REGISTRY: &str = "schemas/language-schema-profiles.json";
pub const BUNDLE_RECEIPT_FILE: &str = ".asp-schema-manager-receipt.json";

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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LanguageSchemaProfile {
    pub language_id: String,
    pub search_producer_axes: Vec<SearchProducerAxis>,
    pub package_root: String,
    pub bundle_root: String,
    pub root_sets: Vec<String>,
    pub roots: Vec<String>,
    pub provider_owned: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SearchProducerAxis {
    Language,
    Document,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SchemaBundleEntry {
    pub name: String,
    pub digest: String,
}

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaBundleReport {
    pub language_id: String,
    pub schema_count: usize,
    pub changed_count: usize,
    pub removed_count: usize,
    pub receipt_path: PathBuf,
    pub bundle_digest: String,
}

/// One canonical schema document resolved without writing a package-local bundle.
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

#[derive(Clone, Debug)]
pub struct SchemaManager {
    workspace_root: PathBuf,
    registry_path: PathBuf,
}

impl SchemaManager {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        let workspace_root = workspace_root.into();
        Self {
            registry_path: workspace_root.join(DEFAULT_PROFILE_REGISTRY),
            workspace_root,
        }
    }

    /// Returns the registered language profiles from the canonical schema registry.
    ///
    /// Consumers must use this owner instead of maintaining a second language list.
    pub fn registered_language_profiles(&self) -> Result<Vec<LanguageSchemaProfile>, String> {
        Ok(self.load_registry()?.profiles)
    }

    /// Resolves a declaratively registered, Schema Manager-owned wire artifact.
    /// Build scripts consume this path instead of maintaining package-local
    /// protocol declarations.
    pub fn canonical_wire_artifact_path(&self, artifact_id: &str) -> Result<PathBuf, String> {
        validate_identity("wireArtifactId", artifact_id)?;
        let registry = self.load_registry_document()?;
        self.validate_wire_artifacts(&registry)?;
        let name = registry
            .wire_artifacts
            .get(artifact_id)
            .ok_or_else(|| format!("unknown canonical wire artifact: {artifact_id}"))?;
        let path = self.workspace_root.join("schemas").join(name);
        if !path.is_file() {
            return Err(format!(
                "canonical wire artifact is missing: {}",
                path.display()
            ));
        }
        Ok(path)
    }

    pub fn with_registry(
        workspace_root: impl Into<PathBuf>,
        registry_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            registry_path: registry_path.into(),
        }
    }

    pub async fn materialize(
        &self,
        language_ids: &[String],
    ) -> Result<Vec<SchemaBundleReport>, String> {
        let manager = self.clone();
        let language_ids = language_ids.to_vec();
        tokio::task::spawn_blocking(move || manager.materialize_blocking(&language_ids))
            .await
            .map_err(|error| format!("schema materialization task failed: {error}"))?
    }

    pub async fn verify(&self, language_ids: &[String]) -> Result<Vec<SchemaBundleReport>, String> {
        let manager = self.clone();
        let language_ids = language_ids.to_vec();
        tokio::task::spawn_blocking(move || manager.verify_blocking(&language_ids))
            .await
            .map_err(|error| format!("schema verification task failed: {error}"))?
    }

    /// Resolve canonical closures for embedding without materializing or
    /// verifying a mutable package-local bundle.
    pub async fn resolve_bundles(
        &self,
        language_ids: &[String],
    ) -> Result<Vec<ResolvedLanguageSchemaBundle>, String> {
        let manager = self.clone();
        let language_ids = language_ids.to_vec();
        tokio::task::spawn_blocking(move || manager.resolve_bundles_blocking(&language_ids))
            .await
            .map_err(|error| format!("schema bundle resolution task failed: {error}"))?
    }

    pub async fn publish_client_bundle(
        &self,
        language_id: String,
        output_root: impl Into<PathBuf>,
    ) -> Result<SchemaBundleReport, String> {
        let manager = self.clone();
        let output_root = output_root.into();
        tokio::task::spawn_blocking(move || {
            manager.publish_client_bundle_blocking(&language_id, &output_root)
        })
        .await
        .map_err(|error| format!("client schema publication task failed: {error}"))?
    }

    pub async fn responsibilities(&self) -> Result<Vec<SchemaResponsibility>, String> {
        let manager = self.clone();
        tokio::task::spawn_blocking(move || {
            let registry = manager.load_registry()?;
            manager.schema_responsibilities(&registry)
        })
        .await
        .map_err(|error| format!("schema responsibility audit task failed: {error}"))?
    }

    fn materialize_blocking(
        &self,
        language_ids: &[String],
    ) -> Result<Vec<SchemaBundleReport>, String> {
        let registry = self.load_registry()?;
        self.select_profiles(&registry, language_ids)?
            .into_iter()
            .map(|profile| self.materialize_profile(&registry, profile))
            .collect()
    }

    fn verify_blocking(&self, language_ids: &[String]) -> Result<Vec<SchemaBundleReport>, String> {
        let registry = self.load_registry()?;
        self.select_profiles(&registry, language_ids)?
            .into_iter()
            .map(|profile| self.verify_profile(&registry, profile))
            .collect()
    }

    fn resolve_bundles_blocking(
        &self,
        language_ids: &[String],
    ) -> Result<Vec<ResolvedLanguageSchemaBundle>, String> {
        let registry = self.load_registry()?;
        self.select_profiles(&registry, language_ids)?
            .into_iter()
            .map(|profile| {
                let (receipt, documents) = self.expected_receipt(&registry, profile)?;
                let schemas = receipt
                    .schemas
                    .into_iter()
                    .map(|schema| {
                        let bytes = documents.get(&schema.name).cloned().ok_or_else(|| {
                            format!(
                                "resolved schema bundle omitted canonical document: {}",
                                schema.name
                            )
                        })?;
                        Ok(ResolvedSchemaDocument {
                            name: schema.name,
                            digest: schema.digest,
                            bytes,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                Ok(ResolvedLanguageSchemaBundle {
                    language_id: profile.language_id.clone(),
                    root_set_ids: profile.root_sets.clone(),
                    bundle_digest: receipt.bundle_digest,
                    schemas,
                })
            })
            .collect()
    }

    fn publish_client_bundle_blocking(
        &self,
        language_id: &str,
        output_root: &Path,
    ) -> Result<SchemaBundleReport, String> {
        validate_identity("languageId", language_id)?;
        let registry = self.load_registry()?;
        let requested = vec![language_id.to_owned()];
        let profile = self
            .select_profiles(&registry, &requested)?
            .into_iter()
            .next()
            .ok_or_else(|| format!("unknown language schema profile: {language_id}"))?;
        let (receipt, documents) = self.expected_receipt(&registry, profile)?;
        write_bundle(profile, &receipt, &documents, output_root, &BTreeSet::new())
    }

    fn load_registry(&self) -> Result<LanguageSchemaProfileRegistry, String> {
        let registry = self.load_registry_document()?;
        self.schema_responsibilities(&registry)?;
        self.validate_wire_artifacts(&registry)?;
        let mut languages = BTreeSet::new();
        for profile in &registry.profiles {
            validate_identity("languageId", &profile.language_id)?;
            if profile.search_producer_axes.is_empty() {
                return Err(format!(
                    "language schema profile has no Search producer axis: {}",
                    profile.language_id
                ));
            }
            if profile
                .search_producer_axes
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
                != profile.search_producer_axes.len()
            {
                return Err(format!(
                    "language schema profile has duplicate Search producer axes: {}",
                    profile.language_id
                ));
            }
            validate_relative_path("packageRoot", &profile.package_root)?;
            validate_relative_path("bundleRoot", &profile.bundle_root)?;
            if !languages.insert(profile.language_id.as_str()) {
                return Err(format!(
                    "duplicate language schema profile: {}",
                    profile.language_id
                ));
            }
            for root_set in &profile.root_sets {
                if !registry.root_sets.contains_key(root_set) {
                    return Err(format!(
                        "unknown schema root set {root_set} for {}",
                        profile.language_id
                    ));
                }
            }
            ensure_unique("schema root set", &profile.root_sets)?;
            ensure_unique("schema root", &profile.roots)?;
            ensure_unique("provider-owned schema", &profile.provider_owned)?;
            for name in profile.roots.iter().chain(&profile.provider_owned) {
                validate_schema_name(name)?;
            }
        }
        Ok(registry)
    }

    fn load_registry_document(&self) -> Result<LanguageSchemaProfileRegistry, String> {
        let bytes = fs::read(&self.registry_path).map_err(|error| {
            format!(
                "read schema profile registry {}: {error}",
                self.registry_path.display()
            )
        })?;
        let registry: LanguageSchemaProfileRegistry = serde_json::from_slice(&bytes)
            .map_err(|error| format!("decode schema profile registry: {error}"))?;
        if registry.schema_id != PROFILE_REGISTRY_SCHEMA_ID
            || registry.schema_version != SCHEMA_VERSION
        {
            return Err("schema profile registry identity is unsupported".to_owned());
        }
        Ok(registry)
    }

    fn validate_wire_artifacts(
        &self,
        registry: &LanguageSchemaProfileRegistry,
    ) -> Result<(), String> {
        for (artifact_id, name) in &registry.wire_artifacts {
            validate_identity("wireArtifactId", artifact_id)?;
            validate_relative_path("wireArtifact", name)?;
            if !name.ends_with(".proto") || Path::new(name).components().count() != 1 {
                return Err(format!("wire artifact must be a protobuf basename: {name}"));
            }
        }
        Ok(())
    }

    fn schema_responsibilities(
        &self,
        registry: &LanguageSchemaProfileRegistry,
    ) -> Result<Vec<SchemaResponsibility>, String> {
        audit_schema_responsibilities(&self.workspace_root, registry)
    }

    fn select_profiles<'a>(
        &self,
        registry: &'a LanguageSchemaProfileRegistry,
        language_ids: &[String],
    ) -> Result<Vec<&'a LanguageSchemaProfile>, String> {
        if language_ids.is_empty() {
            return Ok(registry.profiles.iter().collect());
        }
        let requested = language_ids.iter().collect::<BTreeSet<_>>();
        let selected = registry
            .profiles
            .iter()
            .filter(|profile| requested.contains(&profile.language_id))
            .collect::<Vec<_>>();
        let selected_ids = selected
            .iter()
            .map(|profile| &profile.language_id)
            .collect::<BTreeSet<_>>();
        let missing = requested.difference(&selected_ids).collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(format!(
                "unknown language schema profile: {}",
                missing
                    .into_iter()
                    .map(|value| value.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        Ok(selected)
    }

    fn profile_roots(
        &self,
        registry: &LanguageSchemaProfileRegistry,
        profile: &LanguageSchemaProfile,
    ) -> Result<BTreeSet<String>, String> {
        let mut roots = profile.roots.iter().cloned().collect::<BTreeSet<_>>();
        for root_set in &profile.root_sets {
            let names = registry
                .root_sets
                .get(root_set)
                .ok_or_else(|| format!("unknown schema root set: {root_set}"))?;
            roots.extend(names.iter().cloned());
        }
        for name in &roots {
            validate_schema_name(name)?;
        }
        Ok(roots)
    }

    fn resolve_closure(
        &self,
        roots: BTreeSet<String>,
    ) -> Result<BTreeMap<String, Vec<u8>>, String> {
        let schema_root = self.workspace_root.join("schemas");
        let mut pending = roots.into_iter().collect::<Vec<_>>();
        let mut documents = BTreeMap::new();
        while let Some(name) = pending.pop() {
            if documents.contains_key(&name) {
                continue;
            }
            validate_schema_name(&name)?;
            let path = schema_root.join(&name);
            let bytes = fs::read(&path)
                .map_err(|error| format!("read canonical schema {}: {error}", path.display()))?;
            let value: Value = serde_json::from_slice(&bytes)
                .map_err(|error| format!("decode canonical schema {name}: {error}"))?;
            for reference in schema_references(&value) {
                if let Some(reference_name) = local_schema_name(reference) {
                    if !documents.contains_key(reference_name) {
                        pending.push(reference_name.to_owned());
                    }
                }
            }
            documents.insert(name, bytes);
        }
        Ok(documents)
    }

    fn expected_receipt(
        &self,
        registry: &LanguageSchemaProfileRegistry,
        profile: &LanguageSchemaProfile,
    ) -> Result<(LanguageSchemaBundleReceipt, BTreeMap<String, Vec<u8>>), String> {
        let roots = self.profile_roots(registry, profile)?;
        let documents = self.resolve_closure(roots)?;
        let schemas = documents
            .iter()
            .map(|(name, bytes)| SchemaBundleEntry {
                name: name.clone(),
                digest: schema_digest(bytes),
            })
            .collect::<Vec<_>>();
        let profile_bytes = serde_json::to_vec(profile)
            .map_err(|error| format!("encode language schema profile: {error}"))?;
        let bundle_bytes = serde_json::to_vec(&schemas)
            .map_err(|error| format!("encode language schema bundle: {error}"))?;
        Ok((
            LanguageSchemaBundleReceipt {
                schema_id: BUNDLE_RECEIPT_SCHEMA_ID.to_owned(),
                schema_version: SCHEMA_VERSION.to_owned(),
                schema_digest: tagged_content_digest(
                    b"asp.language-schema-bundle.v1",
                    &[&bundle_bytes],
                ),
                language_id: profile.language_id.clone(),
                profile_digest: tagged_content_digest(
                    b"asp.language-schema-profile.v1",
                    &[&profile_bytes],
                ),
                bundle_digest: tagged_content_digest(
                    b"asp.language-schema-bundle.v1",
                    &[&bundle_bytes],
                ),
                schemas,
            },
            documents,
        ))
    }

    fn materialize_profile(
        &self,
        registry: &LanguageSchemaProfileRegistry,
        profile: &LanguageSchemaProfile,
    ) -> Result<SchemaBundleReport, String> {
        let (receipt, documents) = self.expected_receipt(registry, profile)?;
        let schema_root = self.workspace_root.join(&profile.bundle_root);
        fs::create_dir_all(&schema_root).map_err(|error| {
            format!(
                "create schema bundle root {}: {error}",
                schema_root.display()
            )
        })?;
        for name in &profile.provider_owned {
            let path = schema_root.join(name);
            if !path.is_file() {
                return Err(format!(
                    "provider-owned schema is missing for {}: {}",
                    profile.language_id,
                    path.display()
                ));
            }
        }
        let provider_owned = profile
            .provider_owned
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        write_bundle(profile, &receipt, &documents, &schema_root, &provider_owned)
    }

    fn verify_profile(
        &self,
        registry: &LanguageSchemaProfileRegistry,
        profile: &LanguageSchemaProfile,
    ) -> Result<SchemaBundleReport, String> {
        let (expected, documents) = self.expected_receipt(registry, profile)?;
        let schema_root = self.workspace_root.join(&profile.bundle_root);
        for name in &profile.provider_owned {
            if !schema_root.join(name).is_file() {
                return Err(format!(
                    "provider-owned schema is missing for {}: {name}",
                    profile.language_id
                ));
            }
        }
        for (name, bytes) in &documents {
            let actual = fs::read(schema_root.join(name))
                .map_err(|error| format!("read materialized schema {name}: {error}"))?;
            if &actual != bytes {
                return Err(format!(
                    "materialized schema digest drift for {}: {name}",
                    profile.language_id
                ));
            }
        }
        let receipt_path = schema_root.join(BUNDLE_RECEIPT_FILE);
        let actual = read_receipt_if_present(&receipt_path)?.ok_or_else(|| {
            format!(
                "schema bundle receipt is missing: {}",
                receipt_path.display()
            )
        })?;
        if actual != expected {
            return Err(format!(
                "schema bundle receipt is stale for {}: expected={} actual={}",
                profile.language_id, expected.bundle_digest, actual.bundle_digest
            ));
        }
        Ok(report(profile, &expected, 0, 0, receipt_path))
    }
}

fn write_bundle(
    profile: &LanguageSchemaProfile,
    receipt: &LanguageSchemaBundleReceipt,
    documents: &BTreeMap<String, Vec<u8>>,
    schema_root: &Path,
    protected_names: &BTreeSet<String>,
) -> Result<SchemaBundleReport, String> {
    fs::create_dir_all(schema_root).map_err(|error| {
        format!(
            "create schema bundle root {}: {error}",
            schema_root.display()
        )
    })?;
    let receipt_path = schema_root.join(BUNDLE_RECEIPT_FILE);
    let previous = match read_receipt_if_present(&receipt_path) {
        Ok(previous) => previous,
        // A pre-MVP1 receipt is deliberately not accepted as a current
        // receipt, but materialization must be able to replace it atomically.
        Err(_) => None,
    };
    let expected_names = documents.keys().cloned().collect::<BTreeSet<_>>();
    let mut changed_count = 0;
    for (name, bytes) in documents {
        let target = schema_root.join(name);
        if fs::read(&target).ok().as_deref() == Some(bytes.as_slice()) {
            continue;
        }
        atomic_write(&target, bytes)?;
        changed_count += 1;
    }
    let mut removed_count = 0;
    if let Some(previous) = previous {
        for stale in previous
            .schemas
            .into_iter()
            .map(|entry| entry.name)
            .filter(|name| !expected_names.contains(name) && !protected_names.contains(name))
        {
            let stale_path = schema_root.join(&stale);
            if stale_path.is_file() {
                fs::remove_file(&stale_path).map_err(|error| {
                    format!(
                        "remove stale managed schema {}: {error}",
                        stale_path.display()
                    )
                })?;
                removed_count += 1;
            }
        }
    }
    let receipt_bytes = serde_json::to_vec_pretty(receipt)
        .map_err(|error| format!("encode schema bundle receipt: {error}"))?;
    if fs::read(&receipt_path).ok().as_deref() != Some(receipt_bytes.as_slice()) {
        atomic_write(&receipt_path, &receipt_bytes)?;
        changed_count += 1;
    }
    let membership = SchemaBundleMembership {
        language_id: receipt.language_id.clone(),
        profile_digest: receipt.profile_digest.clone(),
        bundle_digest: receipt.bundle_digest.clone(),
        schemas: receipt.schemas.clone(),
    };
    let membership_path = schema_root.join(BUNDLE_MEMBERSHIP_FILE);
    let membership_bytes = serde_json::to_vec_pretty(&membership)
        .map_err(|error| format!("encode schema bundle membership: {error}"))?;
    if fs::read(&membership_path).ok().as_deref() != Some(membership_bytes.as_slice()) {
        atomic_write(&membership_path, &membership_bytes)?;
        changed_count += 1;
    }
    sync_directory(schema_root)?;
    Ok(report(
        profile,
        receipt,
        changed_count,
        removed_count,
        receipt_path,
    ))
}

pub async fn verify_bundle_receipt(
    receipt_path: impl Into<PathBuf>,
) -> Result<LanguageSchemaBundleReceipt, String> {
    let receipt_path = receipt_path.into();
    tokio::task::spawn_blocking(move || verify_bundle_receipt_blocking(&receipt_path))
        .await
        .map_err(|error| format!("schema bundle receipt verification task failed: {error}"))?
}

pub fn load_verified_bundle_receipt(
    receipt_path: impl AsRef<Path>,
) -> Result<LanguageSchemaBundleReceipt, String> {
    verify_bundle_receipt_blocking(receipt_path.as_ref())
}

fn report(
    profile: &LanguageSchemaProfile,
    receipt: &LanguageSchemaBundleReceipt,
    changed_count: usize,
    removed_count: usize,
    receipt_path: PathBuf,
) -> SchemaBundleReport {
    SchemaBundleReport {
        language_id: profile.language_id.clone(),
        schema_count: receipt.schemas.len(),
        changed_count,
        removed_count,
        receipt_path,
        bundle_digest: receipt.bundle_digest.clone(),
    }
}

fn atomic_write(target: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = target
        .parent()
        .ok_or_else(|| format!("schema target has no parent: {}", target.display()))?;
    let mut staged = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| format!("stage schema bundle file in {}: {error}", parent.display()))?;
    staged
        .write_all(bytes)
        .and_then(|()| staged.as_file().sync_all())
        .map_err(|error| format!("write staged schema bundle file: {error}"))?;
    let staged_path = staged.into_temp_path();
    fs::rename(&staged_path, target)
        .map_err(|error| format!("atomically publish schema {}: {error}", target.display()))?;
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), String> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("sync schema bundle directory {}: {error}", path.display()))
}
