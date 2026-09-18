// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Canonical schema closure resolution and content-addressed bundle publication.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use serde_json::Value;

use crate::responsibility::audit_schema_responsibilities;
use crate::responsibility_model::SchemaResponsibility;

use crate::manager_validation::ensure_unique;
use crate::manager_validation::local_schema_name;
use crate::manager_validation::schema_references;
use crate::manager_validation::validate_identity;
use crate::manager_validation::validate_relative_path;
use crate::manager_validation::validate_schema_name;
use crate::publication::report;
use crate::publication::write_bundle;
use crate::publication::write_package_projection;
use crate::receipt::BUNDLE_MEMBERSHIP_FILE;
use crate::receipt::read_public_receipt_blocking;
use crate::receipt::schema_digest;
use crate::receipt::tagged_content_digest;
use crate::receipt::verify_bundle_receipt_blocking;
use crate::registry::BUNDLE_RECEIPT_FILE;
use crate::registry::BUNDLE_RECEIPT_SCHEMA_ID;
use crate::registry::DEFAULT_PROFILE_REGISTRY;
use crate::registry::LanguageSchemaBundleReceipt;
use crate::registry::LanguageSchemaProfile;
use crate::registry::LanguageSchemaProfileRegistry;
use crate::registry::PROFILE_REGISTRY_SCHEMA_ID;
use crate::registry::ResolvedLanguageSchemaBundle;
use crate::registry::ResolvedSchemaDocument;
use crate::registry::SCHEMA_VERSION;
use crate::registry::SchemaBundleEntry;
use crate::registry::SchemaBundleReport;
use crate::task_owner::run_blocking;

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
        run_blocking("schema-materialize", move || {
            manager.materialize_blocking(&language_ids)
        })
        .await
    }

    pub async fn verify(&self, language_ids: &[String]) -> Result<Vec<SchemaBundleReport>, String> {
        let manager = self.clone();
        let language_ids = language_ids.to_vec();
        run_blocking("schema-verify", move || {
            manager.verify_blocking(&language_ids)
        })
        .await
    }

    /// Resolve canonical closures for embedding without materializing or
    /// verifying a mutable package-local bundle.
    pub async fn resolve_bundles(
        &self,
        language_ids: &[String],
    ) -> Result<Vec<ResolvedLanguageSchemaBundle>, String> {
        let manager = self.clone();
        let language_ids = language_ids.to_vec();
        run_blocking("schema-resolve-bundles", move || {
            manager.resolve_bundles_blocking(&language_ids)
        })
        .await
    }

    pub async fn publish_client_bundle(
        &self,
        language_id: String,
        output_root: impl Into<PathBuf>,
    ) -> Result<SchemaBundleReport, String> {
        let manager = self.clone();
        let output_root = output_root.into();
        run_blocking("schema-publish-client-bundle", move || {
            manager.publish_client_bundle_blocking(&language_id, &output_root)
        })
        .await
    }

    pub async fn responsibilities(&self) -> Result<Vec<SchemaResponsibility>, String> {
        let manager = self.clone();
        run_blocking("schema-audit-responsibilities", move || {
            let registry = manager.load_registry()?;
            manager.schema_responsibilities(&registry)
        })
        .await
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
        let package_bundle_root = self.workspace_root.join(&profile.bundle_root);
        if output_root.canonicalize().ok() == package_bundle_root.canonicalize().ok() {
            return Err(format!(
                "portable client bundle output cannot target a Language package: {}",
                output_root.display()
            ));
        }
        let (receipt, documents) = self.expected_receipt(&registry, profile)?;
        write_bundle(profile, &receipt, &documents, output_root, &BTreeSet::new())
    }

    pub(super) fn load_registry(&self) -> Result<LanguageSchemaProfileRegistry, String> {
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
            ensure_unique("bootstrap schema", &profile.bootstrap)?;
            ensure_unique("provider-owned schema", &profile.provider_owned)?;
            for name in profile
                .roots
                .iter()
                .chain(&profile.bootstrap)
                .chain(&profile.provider_owned)
            {
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

    pub(super) fn select_profiles<'a>(
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
                if let Some(reference_name) = local_schema_name(reference)
                    && !documents.contains_key(reference_name)
                {
                    pending.push(reference_name.to_owned());
                }
            }
            documents.insert(name, bytes);
        }
        Ok(documents)
    }

    pub(super) fn expected_receipt(
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
        write_package_projection(profile, &receipt, &documents, &schema_root)
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
        for name in &profile.bootstrap {
            let expected_bytes = documents.get(name).ok_or_else(|| {
                format!(
                    "bootstrap schema is outside canonical closure for {}: {name}",
                    profile.language_id
                )
            })?;
            let actual = fs::read(schema_root.join(name))
                .map_err(|error| format!("read bootstrap schema {name}: {error}"))?;
            if &actual != expected_bytes {
                return Err(format!(
                    "bootstrap schema digest drift for {}: {name}",
                    profile.language_id
                ));
            }
        }
        let receipt_path = schema_root.join(BUNDLE_RECEIPT_FILE);
        let actual = read_public_receipt_blocking(&receipt_path)?;
        if actual.schema_digest != expected.bundle_digest {
            return Err(format!(
                "schema bundle receipt is stale for {}: expected={} actual={}",
                profile.language_id, expected.bundle_digest, actual.schema_digest
            ));
        }
        let membership_path = schema_root.join(BUNDLE_MEMBERSHIP_FILE);
        if membership_path.exists() {
            return Err(format!(
                "package-local schema membership is forbidden for {}: {}",
                profile.language_id,
                membership_path.display()
            ));
        }
        let allowed = profile
            .provider_owned
            .iter()
            .chain(&profile.bootstrap)
            .cloned()
            .collect::<BTreeSet<_>>();
        for entry in fs::read_dir(&schema_root)
            .map_err(|error| format!("read schema root {}: {error}", schema_root.display()))?
        {
            let entry = entry.map_err(|error| format!("read schema root entry: {error}"))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.ends_with(".schema.json") && !allowed.contains(&name) {
                return Err(format!(
                    "shared package-local schema copy is forbidden for {}: {name}",
                    profile.language_id
                ));
            }
        }
        Ok(report(profile, &expected, 0, 0, receipt_path))
    }
}

/// Verify a portable bundle receipt without blocking a Tokio worker thread.
pub async fn verify_bundle_receipt(
    receipt_path: impl Into<PathBuf>,
) -> Result<LanguageSchemaBundleReceipt, String> {
    let receipt_path = receipt_path.into();
    run_blocking("schema-verify-bundle-receipt", move || {
        verify_bundle_receipt_blocking(&receipt_path)
    })
    .await
}

/// Load and verify a portable bundle receipt from a synchronous owner.
pub fn load_verified_bundle_receipt(
    receipt_path: impl AsRef<Path>,
) -> Result<LanguageSchemaBundleReceipt, String> {
    verify_bundle_receipt_blocking(receipt_path.as_ref())
}
