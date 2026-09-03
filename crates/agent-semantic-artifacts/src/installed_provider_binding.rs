//! Immutable V1 binding between a binary provider catalog and installed artifacts.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const INSTALLED_PROVIDER_BINDING_SCHEMA_ID: &str =
    "agent.semantic-protocols.installed-provider-binding";
pub const INSTALLED_PROVIDER_BINDING_SCHEMA_VERSION: &str = "1";
pub const RUNTIME_PROVIDER_EXECUTION_BINDING_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-provider-execution-binding";
pub const RUNTIME_PROVIDER_EXECUTION_BINDING_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstalledProviderArtifactIdentity {
    pub language_id: String,
    pub provider_id: String,
    pub artifact_digest: String,
    pub entrypoint_digest: String,
    pub artifact_metadata_digest: String,
    pub execution_command_digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstalledProviderBinding {
    pub schema_id: String,
    pub schema_version: String,
    pub generation: String,
    pub binary_catalog_digest: String,
    pub provider_registration_digest: String,
    pub hook_policy_digest: String,
    pub providers: Vec<InstalledProviderArtifactIdentity>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledProviderBindingInput {
    pub binary_catalog_digest: String,
    pub provider_registration_digest: String,
    pub hook_policy_digest: String,
    pub providers: Vec<InstalledProviderArtifactIdentity>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeProviderExecutionBinding {
    pub schema_id: String,
    pub schema_version: String,
    pub generation: String,
    pub project_id: String,
    pub workspace_id: String,
    pub installed_provider_binding_generation: String,
    pub schema_bundle_digest: String,
    pub workspace_closure_digest: String,
    pub source_snapshot_digest: String,
    pub source_index_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledProviderBindingPublication {
    pub path: PathBuf,
    pub generation: String,
    pub artifact_write: bool,
}

pub fn installed_provider_binding_path(state_home: &Path) -> PathBuf {
    state_home
        .join("runtime")
        .join("installed-provider-binding.v1.json")
}

pub fn load_installed_provider_binding(
    state_home: &Path,
) -> Result<Option<InstalledProviderBinding>, String> {
    let path = installed_provider_binding_path(state_home);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "read installed provider binding {}: {error}",
                path.display()
            ));
        }
    };
    let binding: InstalledProviderBinding = serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "decode installed provider binding {}: {error}",
            path.display()
        )
    })?;
    binding.validate()?;
    Ok(Some(binding))
}

/// Atomically publish one complete V1 binding under the canonical artifact
/// mutation guard. `expected_generation` is the caller's CAS fence.
pub fn publish_installed_provider_binding(
    state_home: &Path,
    input: InstalledProviderBindingInput,
    expected_generation: Option<&str>,
) -> Result<InstalledProviderBindingPublication, String> {
    let binding = InstalledProviderBinding::build(input)?;
    let artifact_root = state_home.join("runtime/artifacts");
    let _guard = crate::runtime_artifact_retention::RuntimeArtifactMutationGuard::try_acquire(
        &artifact_root,
    )?;
    let path = installed_provider_binding_path(state_home);
    let current = load_installed_provider_binding(state_home)?;
    let observed_generation = current.as_ref().map(|value| value.generation.as_str());
    if let Some(expected) = expected_generation
        && observed_generation != Some(expected)
    {
        return Err(format!(
            "installed provider binding publication conflict: expectedGeneration={expected} observedGeneration={}",
            observed_generation.unwrap_or("absent")
        ));
    }
    if observed_generation == Some(binding.generation.as_str()) {
        return Ok(InstalledProviderBindingPublication {
            path,
            generation: binding.generation,
            artifact_write: false,
        });
    }
    let parent = path.parent().ok_or_else(|| {
        format!(
            "installed provider binding path has no parent: {}",
            path.display()
        )
    })?;
    std::fs::create_dir_all(parent).map_err(|error| {
        format!(
            "create installed provider binding root {}: {error}",
            parent.display()
        )
    })?;
    let temporary = parent.join(format!(
        ".installed-provider-binding.{}.tmp",
        binding.generation
    ));
    let bytes = serde_json::to_vec_pretty(&binding)
        .map_err(|error| format!("encode installed provider binding: {error}"))?;
    std::fs::write(&temporary, bytes).map_err(|error| {
        format!(
            "stage installed provider binding {}: {error}",
            temporary.display()
        )
    })?;
    std::fs::rename(&temporary, &path).map_err(|error| {
        format!(
            "publish installed provider binding {}: {error}",
            path.display()
        )
    })?;
    Ok(InstalledProviderBindingPublication {
        path,
        generation: binding.generation,
        artifact_write: true,
    })
}

/// Converge normal catalog or policy drift onto the requested V1 binding.
/// Publication conflicts are retried against the winner's generation; invalid
/// or incomplete input still fails closed in `build`.
pub fn reconcile_installed_provider_binding(
    state_home: &Path,
    input: InstalledProviderBindingInput,
) -> Result<InstalledProviderBindingPublication, String> {
    for _ in 0..3 {
        let current = load_installed_provider_binding(state_home)?;
        if current
            .as_ref()
            .is_some_and(|binding| !binding.requires_refresh(&input))
        {
            let binding = current.expect("checked installed provider binding");
            return Ok(InstalledProviderBindingPublication {
                path: installed_provider_binding_path(state_home),
                generation: binding.generation,
                artifact_write: false,
            });
        }
        let expected = current.as_ref().map(|binding| binding.generation.as_str());
        match publish_installed_provider_binding(state_home, input.clone(), expected) {
            Ok(publication) => return Ok(publication),
            Err(error) if error.contains("publication conflict") => continue,
            Err(error) => return Err(error),
        }
    }
    Err("installed provider binding publication did not converge after 3 CAS attempts".to_owned())
}

impl InstalledProviderBinding {
    pub fn build(mut input: InstalledProviderBindingInput) -> Result<Self, String> {
        input
            .providers
            .sort_by(|left, right| left.language_id.cmp(&right.language_id));
        let mut binding = Self {
            schema_id: INSTALLED_PROVIDER_BINDING_SCHEMA_ID.to_owned(),
            schema_version: INSTALLED_PROVIDER_BINDING_SCHEMA_VERSION.to_owned(),
            generation: String::new(),
            binary_catalog_digest: input.binary_catalog_digest,
            provider_registration_digest: input.provider_registration_digest,
            hook_policy_digest: input.hook_policy_digest,
            providers: input.providers,
        };
        binding.generation = binding.derived_generation()?;
        binding.validate()?;
        Ok(binding)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != INSTALLED_PROVIDER_BINDING_SCHEMA_ID
            || self.schema_version != INSTALLED_PROVIDER_BINDING_SCHEMA_VERSION
        {
            return Err("installed provider binding schema identity mismatch".to_owned());
        }
        for (field, digest) in [
            ("binaryCatalogDigest", &self.binary_catalog_digest),
            (
                "providerRegistrationDigest",
                &self.provider_registration_digest,
            ),
            ("hookPolicyDigest", &self.hook_policy_digest),
        ] {
            validate_digest(field, digest)?;
        }
        let mut languages = BTreeSet::new();
        let mut providers = BTreeSet::new();
        for provider in &self.providers {
            if provider.language_id.is_empty() || provider.provider_id.is_empty() {
                return Err("installed provider binding identity is empty".to_owned());
            }
            if !languages.insert(provider.language_id.as_str())
                || !providers.insert(provider.provider_id.as_str())
            {
                return Err("installed provider binding identities must be unique".to_owned());
            }
            validate_digest("artifactDigest", &provider.artifact_digest)?;
            validate_digest("entrypointDigest", &provider.entrypoint_digest)?;
            validate_digest("artifactMetadataDigest", &provider.artifact_metadata_digest)?;
            validate_digest("executionCommandDigest", &provider.execution_command_digest)?;
        }
        if self.generation != self.derived_generation()? {
            return Err("installed provider binding generation drift".to_owned());
        }
        Ok(())
    }

    pub fn requires_refresh(&self, expected: &InstalledProviderBindingInput) -> bool {
        Self::build(expected.clone())
            .map(|binding| binding.generation != self.generation)
            .unwrap_or(true)
    }

    fn derived_generation(&self) -> Result<String, String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Identity<'a> {
            binary_catalog_digest: &'a str,
            provider_registration_digest: &'a str,
            hook_policy_digest: &'a str,
            providers: &'a [InstalledProviderArtifactIdentity],
        }
        let bytes = serde_json::to_vec(&Identity {
            binary_catalog_digest: &self.binary_catalog_digest,
            provider_registration_digest: &self.provider_registration_digest,
            hook_policy_digest: &self.hook_policy_digest,
            providers: &self.providers,
        })
        .map_err(|error| format!("encode installed provider binding: {error}"))?;
        Ok(format!("blake3-256:{}", blake3::hash(&bytes).to_hex()))
    }
}

/// Re-admit the executable bytes referenced by a verified binding. Absolute
/// paths remain a compatibility projection only; authority comes from the
/// canonical State Home artifact root plus the content identities in `identity`.
pub fn admit_installed_provider_artifact(
    state_home: &Path,
    materialized_path: &Path,
    identity: &InstalledProviderArtifactIdentity,
) -> Result<PathBuf, String> {
    let canonical_artifact = std::fs::canonicalize(materialized_path).map_err(|error| {
        format!(
            "canonicalize installed provider artifact {}: {error}",
            materialized_path.display()
        )
    })?;
    let authority_root =
        if crate::runtime_artifact_catalog::load_runtime_developer_root(state_home)?.is_some() {
            state_home
                .join("runtime/provider-artifacts")
                .join(&identity.provider_id)
                .join("artifacts")
        } else {
            state_home.join("runtime/artifacts/blake3-256")
        };
    let canonical_authority_root = std::fs::canonicalize(&authority_root).map_err(|error| {
        format!(
            "canonicalize provider artifact authority root {}: {error}",
            authority_root.display()
        )
    })?;
    if !canonical_artifact.starts_with(&canonical_authority_root) {
        return Err(format!(
            "installed provider artifact escapes canonical authority: artifact={} authorityRoot={}",
            canonical_artifact.display(),
            canonical_authority_root.display()
        ));
    }
    let content_digest =
        agent_semantic_content_identity::file_content_digest_v1(&canonical_artifact)?;
    let metadata_digest =
        agent_semantic_content_identity::file_artifact_metadata_digest_v1(&canonical_artifact)?;
    if normalize_digest(&content_digest) != identity.entrypoint_digest
        || normalize_digest(&metadata_digest) != identity.artifact_metadata_digest
    {
        return Err(format!(
            "installed provider artifact content drift: providerId={} artifact={}",
            identity.provider_id,
            canonical_artifact.display()
        ));
    }
    Ok(canonical_artifact)
}

fn normalize_digest(digest: &str) -> String {
    if digest.contains(':') {
        digest.to_owned()
    } else {
        format!("blake3-256:{digest}")
    }
}

impl RuntimeProviderExecutionBinding {
    pub fn build(
        project_id: String,
        workspace_id: String,
        installed_provider_binding_generation: String,
        schema_bundle_digest: String,
        workspace_closure_digest: String,
        source_snapshot_digest: String,
        source_index_digest: String,
    ) -> Result<Self, String> {
        let mut binding = Self {
            schema_id: RUNTIME_PROVIDER_EXECUTION_BINDING_SCHEMA_ID.to_owned(),
            schema_version: RUNTIME_PROVIDER_EXECUTION_BINDING_SCHEMA_VERSION.to_owned(),
            generation: String::new(),
            project_id,
            workspace_id,
            installed_provider_binding_generation,
            schema_bundle_digest,
            workspace_closure_digest,
            source_snapshot_digest,
            source_index_digest,
        };
        binding.generation = binding.derived_generation()?;
        binding.validate()?;
        Ok(binding)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != RUNTIME_PROVIDER_EXECUTION_BINDING_SCHEMA_ID
            || self.schema_version != RUNTIME_PROVIDER_EXECUTION_BINDING_SCHEMA_VERSION
        {
            return Err("runtime provider execution binding schema identity mismatch".to_owned());
        }
        validate_project_id(&self.project_id)?;
        validate_workspace_id(&self.workspace_id)?;
        for (field, digest) in [
            (
                "installedProviderBindingGeneration",
                &self.installed_provider_binding_generation,
            ),
            ("schemaBundleDigest", &self.schema_bundle_digest),
            ("workspaceClosureDigest", &self.workspace_closure_digest),
            ("sourceSnapshotDigest", &self.source_snapshot_digest),
            ("sourceIndexDigest", &self.source_index_digest),
        ] {
            validate_digest(field, digest)?;
        }
        if self.generation != self.derived_generation()? {
            return Err("runtime provider execution binding generation drift".to_owned());
        }
        Ok(())
    }

    fn derived_generation(&self) -> Result<String, String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Identity<'a> {
            project_id: &'a str,
            workspace_id: &'a str,
            installed_provider_binding_generation: &'a str,
            schema_bundle_digest: &'a str,
            workspace_closure_digest: &'a str,
            source_snapshot_digest: &'a str,
            source_index_digest: &'a str,
        }
        let bytes = serde_json::to_vec(&Identity {
            project_id: &self.project_id,
            workspace_id: &self.workspace_id,
            installed_provider_binding_generation: &self.installed_provider_binding_generation,
            schema_bundle_digest: &self.schema_bundle_digest,
            workspace_closure_digest: &self.workspace_closure_digest,
            source_snapshot_digest: &self.source_snapshot_digest,
            source_index_digest: &self.source_index_digest,
        })
        .map_err(|error| format!("encode runtime provider execution binding: {error}"))?;
        Ok(format!("blake3-256:{}", blake3::hash(&bytes).to_hex()))
    }
}

fn validate_project_id(project_id: &str) -> Result<(), String> {
    if project_id.strip_prefix("repo-").is_none_or(str::is_empty) {
        return Err(
            "runtime provider execution binding projectId is not a canonical RepoId".to_owned(),
        );
    }
    Ok(())
}

fn validate_workspace_id(workspace_id: &str) -> Result<(), String> {
    if workspace_id
        .strip_prefix("workspace-")
        .is_none_or(str::is_empty)
    {
        return Err(
            "runtime provider execution binding workspaceId is not a canonical WorkspaceId"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_digest(field: &str, digest: &str) -> Result<(), String> {
    let Some((algorithm, value)) = digest.split_once(':') else {
        return Err(format!(
            "installed provider binding {field} is not an integrity reference"
        ));
    };
    if !matches!(algorithm, "blake3-256" | "sha256")
        || value.len() != 64
        || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(format!("installed provider binding {field} is invalid"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: char) -> String {
        format!("blake3-256:{}", byte.to_string().repeat(64))
    }

    fn input() -> InstalledProviderBindingInput {
        InstalledProviderBindingInput {
            binary_catalog_digest: digest('a'),
            provider_registration_digest: digest('b'),
            hook_policy_digest: digest('d'),
            providers: vec![InstalledProviderArtifactIdentity {
                language_id: "rust".to_owned(),
                provider_id: "asp-rust".to_owned(),
                artifact_digest: digest('f'),
                entrypoint_digest: digest('0'),
                artifact_metadata_digest: digest('1'),
                execution_command_digest: digest('2'),
            }],
        }
    }

    #[test]
    fn binding_refreshes_when_binary_catalog_changes() {
        let binding = InstalledProviderBinding::build(input()).expect("binding");
        let mut changed = input();
        changed.binary_catalog_digest = digest('9');
        assert!(binding.requires_refresh(&changed));
    }

    #[test]
    fn binding_rejects_generation_drift() {
        let mut binding = InstalledProviderBinding::build(input()).expect("binding");
        binding.providers[0].artifact_digest = digest('8');
        assert_eq!(
            binding.validate().expect_err("generation drift"),
            "installed provider binding generation drift"
        );
    }

    #[test]
    fn execution_binding_refreshes_for_each_workspace_authority() {
        let binding = RuntimeProviderExecutionBinding::build(
            "repo-project".to_owned(),
            "workspace-checkout".to_owned(),
            digest('a'),
            digest('b'),
            digest('c'),
            digest('d'),
            digest('e'),
        )
        .expect("execution binding");
        for changed in 0..7 {
            let mut inputs = [
                "repo-project".to_owned(),
                "workspace-checkout".to_owned(),
                digest('a'),
                digest('b'),
                digest('c'),
                digest('d'),
                digest('e'),
            ];
            inputs[changed] = match changed {
                0 => "repo-other".to_owned(),
                1 => "workspace-other".to_owned(),
                _ => digest('9'),
            };
            let refreshed = RuntimeProviderExecutionBinding::build(
                inputs[0].clone(),
                inputs[1].clone(),
                inputs[2].clone(),
                inputs[3].clone(),
                inputs[4].clone(),
                inputs[5].clone(),
                inputs[6].clone(),
            )
            .expect("refreshed execution binding");
            assert_ne!(binding.generation, refreshed.generation);
        }
    }

    #[test]
    fn execution_binding_rejects_noncanonical_project_and_workspace_ids() {
        let error = RuntimeProviderExecutionBinding::build(
            "project-from-path".to_owned(),
            "workspace-checkout".to_owned(),
            digest('a'),
            digest('b'),
            digest('c'),
            digest('d'),
            digest('e'),
        )
        .expect_err("projectId must be a RepoId");
        assert!(error.contains("canonical RepoId"), "{error}");

        let error = RuntimeProviderExecutionBinding::build(
            "repo-project".to_owned(),
            "/tmp/checkout".to_owned(),
            digest('a'),
            digest('b'),
            digest('c'),
            digest('d'),
            digest('e'),
        )
        .expect_err("workspaceId must not be a path");
        assert!(error.contains("canonical WorkspaceId"), "{error}");
    }

    #[test]
    fn publication_refreshes_and_enforces_cas() {
        let root = tempfile::tempdir().expect("state home");
        let first = publish_installed_provider_binding(root.path(), input(), None)
            .expect("first publication");
        assert!(first.artifact_write);
        let warm = publish_installed_provider_binding(
            root.path(),
            input(),
            Some(first.generation.as_str()),
        )
        .expect("warm publication");
        assert!(!warm.artifact_write);
        let mut changed = input();
        changed.hook_policy_digest = digest('9');
        let conflict = publish_installed_provider_binding(
            root.path(),
            changed.clone(),
            Some("blake3-256:deadbeef"),
        )
        .expect_err("stale publisher must lose");
        assert!(conflict.contains("publication conflict"), "{conflict}");
        let refreshed = publish_installed_provider_binding(
            root.path(),
            changed,
            Some(first.generation.as_str()),
        )
        .expect("refresh publication");
        assert!(refreshed.artifact_write);
        assert_ne!(refreshed.generation, first.generation);
        assert_eq!(
            load_installed_provider_binding(root.path())
                .expect("load binding")
                .expect("published binding")
                .generation,
            refreshed.generation
        );
    }
}
