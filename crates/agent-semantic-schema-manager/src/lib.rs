use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use agent_semantic_content_identity::exact_selector_merkle::canonical_content_digest;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    pub root_sets: BTreeMap<String, Vec<String>>,
    pub profiles: Vec<LanguageSchemaProfile>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LanguageSchemaProfile {
    pub language_id: String,
    pub package_root: String,
    pub root_sets: Vec<String>,
    pub roots: Vec<String>,
    pub provider_owned: Vec<String>,
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
    pub language_id: String,
    pub profile_digest: String,
    pub bundle_digest: String,
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

    pub async fn verify(
        &self,
        language_ids: &[String],
    ) -> Result<Vec<SchemaBundleReport>, String> {
        let manager = self.clone();
        let language_ids = language_ids.to_vec();
        tokio::task::spawn_blocking(move || manager.verify_blocking(&language_ids))
            .await
            .map_err(|error| format!("schema verification task failed: {error}"))?
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

    fn verify_blocking(
        &self,
        language_ids: &[String],
    ) -> Result<Vec<SchemaBundleReport>, String> {
        let registry = self.load_registry()?;
        self.select_profiles(&registry, language_ids)?
            .into_iter()
            .map(|profile| self.verify_profile(&registry, profile))
            .collect()
    }

    fn load_registry(&self) -> Result<LanguageSchemaProfileRegistry, String> {
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
        let mut languages = BTreeSet::new();
        for profile in &registry.profiles {
            validate_identity("languageId", &profile.language_id)?;
            validate_relative_path("packageRoot", &profile.package_root)?;
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
            for name in profile.roots.iter().chain(&profile.provider_owned) {
                validate_schema_name(name)?;
            }
        }
        Ok(registry)
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

    fn resolve_closure(&self, roots: BTreeSet<String>) -> Result<BTreeMap<String, Vec<u8>>, String> {
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
        let schema_root = self
            .workspace_root
            .join(&profile.package_root)
            .join("schemas");
        fs::create_dir_all(&schema_root)
            .map_err(|error| format!("create schema bundle root {}: {error}", schema_root.display()))?;
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
        let receipt_path = schema_root.join(BUNDLE_RECEIPT_FILE);
        let previous = read_receipt_if_present(&receipt_path)?;
        let expected_names = documents.keys().cloned().collect::<BTreeSet<_>>();
        let provider_owned = profile.provider_owned.iter().cloned().collect::<BTreeSet<_>>();
        let mut changed_count = 0;
        for (name, bytes) in &documents {
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
                .filter(|name| !expected_names.contains(name) && !provider_owned.contains(name))
            {
                let stale_path = schema_root.join(&stale);
                if stale_path.is_file() {
                    fs::remove_file(&stale_path).map_err(|error| {
                        format!("remove stale managed schema {}: {error}", stale_path.display())
                    })?;
                    removed_count += 1;
                }
            }
        }
        let receipt_bytes = serde_json::to_vec_pretty(&receipt)
            .map_err(|error| format!("encode schema bundle receipt: {error}"))?;
        if fs::read(&receipt_path).ok().as_deref() != Some(receipt_bytes.as_slice()) {
            atomic_write(&receipt_path, &receipt_bytes)?;
            changed_count += 1;
        }
        sync_directory(&schema_root)?;
        Ok(report(profile, &receipt, changed_count, removed_count, receipt_path))
    }

    fn verify_profile(
        &self,
        registry: &LanguageSchemaProfileRegistry,
        profile: &LanguageSchemaProfile,
    ) -> Result<SchemaBundleReport, String> {
        let (expected, documents) = self.expected_receipt(registry, profile)?;
        let schema_root = self
            .workspace_root
            .join(&profile.package_root)
            .join("schemas");
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
        let actual = read_receipt_if_present(&receipt_path)?
            .ok_or_else(|| format!("schema bundle receipt is missing: {}", receipt_path.display()))?;
        if actual != expected {
            return Err(format!(
                "schema bundle receipt is stale for {}: expected={} actual={}",
                profile.language_id, expected.bundle_digest, actual.bundle_digest
            ));
        }
        Ok(report(profile, &expected, 0, 0, receipt_path))
    }
}

pub async fn verify_bundle_receipt(
    receipt_path: impl Into<PathBuf>,
) -> Result<LanguageSchemaBundleReceipt, String> {
    let receipt_path = receipt_path.into();
    tokio::task::spawn_blocking(move || verify_bundle_receipt_blocking(&receipt_path))
        .await
        .map_err(|error| format!("schema bundle receipt verification task failed: {error}"))?
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

fn schema_references(value: &Value) -> Vec<&str> {
    let mut references = Vec::new();
    let mut pending = vec![value];
    while let Some(value) = pending.pop() {
        match value {
            Value::Object(object) => {
                for key in ["$ref", "$dynamicRef"] {
                    if let Some(Value::String(reference)) = object.get(key) {
                        references.push(reference.as_str());
                    }
                }
                pending.extend(object.values());
            }
            Value::Array(array) => pending.extend(array),
            _ => {}
        }
    }
    references
}

fn local_schema_name(reference: &str) -> Option<&str> {
    let target = reference.split('#').next().unwrap_or_default();
    if target.is_empty() {
        return None;
    }
    let name = target.rsplit('/').next()?;
    name.ends_with(".schema.json").then_some(name)
}

fn schema_digest(bytes: &[u8]) -> String {
    tagged_content_digest(b"asp.language-schema-file.v1", &[bytes])
}

fn tagged_content_digest(domain: &[u8], parts: &[&[u8]]) -> String {
    format!(
        "blake3-256:{}",
        canonical_content_digest(domain, parts).as_str()
    )
}

fn read_receipt_if_present(path: &Path) -> Result<Option<LanguageSchemaBundleReceipt>, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("read schema bundle receipt {}: {error}", path.display())),
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| format!("decode schema bundle receipt {}: {error}", path.display()))
}

fn verify_bundle_receipt_blocking(
    receipt_path: &Path,
) -> Result<LanguageSchemaBundleReceipt, String> {
    let receipt = read_receipt_if_present(receipt_path)?
        .ok_or_else(|| format!("schema bundle receipt is missing: {}", receipt_path.display()))?;
    if receipt.schema_id != BUNDLE_RECEIPT_SCHEMA_ID || receipt.schema_version != SCHEMA_VERSION {
        return Err("schema bundle receipt identity is unsupported".to_owned());
    }
    validate_identity("languageId", &receipt.language_id)?;
    let schema_root = receipt_path.parent().ok_or_else(|| {
        format!(
            "schema bundle receipt has no schema root: {}",
            receipt_path.display()
        )
    })?;
    let mut previous_name: Option<&str> = None;
    for entry in &receipt.schemas {
        validate_schema_name(&entry.name)?;
        if previous_name.is_some_and(|previous| previous >= entry.name.as_str()) {
            return Err("schema bundle receipt entries must be sorted and unique".to_owned());
        }
        previous_name = Some(&entry.name);
        let bytes = fs::read(schema_root.join(&entry.name)).map_err(|error| {
            format!("read receipt-owned schema {}: {error}", entry.name)
        })?;
        let actual = schema_digest(&bytes);
        if actual != entry.digest {
            return Err(format!(
                "schema bundle receipt digest mismatch for {}: expected={} actual={actual}",
                entry.name, entry.digest
            ));
        }
    }
    let bundle_bytes = serde_json::to_vec(&receipt.schemas)
        .map_err(|error| format!("encode receipt schema entries: {error}"))?;
    let actual_bundle = tagged_content_digest(b"asp.language-schema-bundle.v1", &[&bundle_bytes]);
    if actual_bundle != receipt.bundle_digest {
        return Err(format!(
            "schema bundle receipt bundle digest mismatch: expected={} actual={actual_bundle}",
            receipt.bundle_digest
        ));
    }
    Ok(receipt)
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

fn validate_identity(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(format!("{field} must be a lowercase semantic identity"));
    }
    Ok(())
}

fn validate_schema_name(name: &str) -> Result<(), String> {
    if !name.ends_with(".schema.json") || Path::new(name).components().count() != 1 {
        return Err(format!("schema name must be a basename ending in .schema.json: {name}"));
    }
    Ok(())
}

fn validate_relative_path(field: &str, path: &str) -> Result<(), String> {
    let path = Path::new(path);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("{field} must be a normalized relative path"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
