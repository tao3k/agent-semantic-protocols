//! Provider-owned workspace build and publication contract.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use serde::Deserialize;

pub const PROVIDER_WORKSPACE_INSTALL_SCHEMA_ID: &str =
    "agent.semantic-protocols.provider-workspace-install";
pub const PROVIDER_WORKSPACE_INSTALL_SCHEMA_VERSION: &str = "1";
pub const PROVIDER_WORKSPACE_INSTALL_SCHEMA_FILE: &str =
    "provider-workspace-install.schema.json";
pub const PROVIDER_WORKSPACE_INSTALL_SCHEMA_AUTHORITY: &str =
    "https://tao3k.github.io/agent-semantic-protocols/schemas/";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderWorkspaceInstallDescriptor {
    #[serde(rename = "$schema")]
    pub schema: String,
    pub schema_id: String,
    pub schema_version: String,
    pub schema_authority: String,
    pub language_id: String,
    pub provider_id: String,
    pub binary: String,
    pub provider_registration: String,
    pub schema_bundle_receipt: String,
    pub workspace_artifact: WorkspaceArtifactDescriptor,
    pub dependency_materialization: Option<WorkspaceCommandDescriptor>,
    pub workspace_build: WorkspaceBuildDescriptor,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceArtifactDescriptor {
    pub root: String,
    pub entrypoint: String,
    pub launch: Option<WorkspaceLaunchDescriptor>,
    #[serde(default)]
    pub runtime_dependencies: Vec<WorkspaceRuntimeDependencyDescriptor>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceRuntimeDependencyDescriptor {
    pub source: String,
    pub target: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceLaunchDescriptor {
    pub program: String,
    pub args: Vec<String>,
    pub program_relative_to_artifact: bool,
    pub args_relative_to_artifact: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceBuildDescriptor {
    pub program: String,
    pub args: Vec<String>,
    pub working_directory: String,
    pub source_snapshot_anchors: Vec<String>,
    pub derived_paths: Vec<String>,
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub remove_env: Vec<String>,
    #[serde(default)]
    pub remove_env_prefixes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceCommandDescriptor {
    pub program: String,
    pub args: Vec<String>,
    pub working_directory: String,
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub remove_env: Vec<String>,
    #[serde(default)]
    pub remove_env_prefixes: Vec<String>,
}

impl ProviderWorkspaceInstallDescriptor {
    pub fn validate(&self) -> Result<(), String> {
        let schema_reference = Path::new(&self.schema);
        if schema_reference.is_absolute()
            || schema_reference.as_os_str().is_empty()
            || schema_reference
                .components()
                .any(|component| matches!(component, Component::RootDir | Component::Prefix(_)))
            || schema_reference.file_name().and_then(|name| name.to_str())
                != Some(PROVIDER_WORKSPACE_INSTALL_SCHEMA_FILE)
            || self.schema_id != PROVIDER_WORKSPACE_INSTALL_SCHEMA_ID
            || self.schema_version != PROVIDER_WORKSPACE_INSTALL_SCHEMA_VERSION
            || self.schema_authority != PROVIDER_WORKSPACE_INSTALL_SCHEMA_AUTHORITY
        {
            return Err(
                "provider workspace install must reference the canonical local schema with schema version 1"
                    .to_owned(),
            );
        }
        if self.workspace_build.source_snapshot_anchors.is_empty()
            || self.workspace_build.derived_paths.is_empty()
        {
            return Err(
                "provider workspace build requires source anchors and derived paths".to_owned(),
            );
        }
        validate_environment_removals(
            "workspaceBuild",
            &self.workspace_build.remove_env,
            &self.workspace_build.remove_env_prefixes,
        )?;
        if let Some(materialization) = &self.dependency_materialization {
            validate_environment_removals(
                "dependencyMaterialization",
                &materialization.remove_env,
                &materialization.remove_env_prefixes,
            )?;
        }
        Ok(())
    }

    pub fn validate_registration_identity(
        &self,
        language_id: &str,
        provider_id: &str,
        binary: &str,
    ) -> Result<(), String> {
        if self.language_id == language_id
            && self.provider_id == provider_id
            && self.binary == binary
        {
            return Ok(());
        }
        Err(format!(
            "provider workspace install identity drift: language={} provider={} binary={} expectedLanguage={language_id} expectedProvider={provider_id} expectedBinary={binary}",
            self.language_id, self.provider_id, self.binary
        ))
    }
}

fn validate_environment_removals(
    field: &str,
    names: &[String],
    prefixes: &[String],
) -> Result<(), String> {
    let valid = |value: &str| {
        let mut bytes = value.bytes();
        matches!(bytes.next(), Some(b'A'..=b'Z' | b'a'..=b'z' | b'_'))
            && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    };
    for (suffix, values) in [("removeEnv", names), ("removeEnvPrefixes", prefixes)] {
        let mut unique = BTreeSet::new();
        for value in values {
            if !valid(value) {
                return Err(format!(
                    "{field}.{suffix} contains an invalid environment identity: {value}"
                ));
            }
            if !unique.insert(value) {
                return Err(format!(
                    "{field}.{suffix} contains a duplicate environment identity: {value}"
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_prefix_cannot_be_empty() {
        let error = validate_environment_removals("workspaceBuild", &[], &[String::new()])
            .expect_err("empty prefix must fail closed");
        assert!(error.contains("invalid environment identity"), "{error}");
    }

    #[test]
    fn schema_reference_may_follow_the_receipted_bundle_root() {
        for schema in [
            "../schemas/provider-workspace-install.schema.json",
            "org/schemas/provider-workspace-install.schema.json",
        ] {
            let descriptor: ProviderWorkspaceInstallDescriptor = serde_json::from_value(
                serde_json::json!({
                    "$schema": schema,
                    "schemaId": PROVIDER_WORKSPACE_INSTALL_SCHEMA_ID,
                    "schemaVersion": PROVIDER_WORKSPACE_INSTALL_SCHEMA_VERSION,
                    "schemaAuthority": PROVIDER_WORKSPACE_INSTALL_SCHEMA_AUTHORITY,
                    "languageId": "fixture",
                    "providerId": "asp-fixture",
                    "binary": "asp-fixture",
                    "providerRegistration": "asp-provider-registration.json",
                    "schemaBundleReceipt": "schemas/.asp-schema-manager-receipt.json",
                    "workspaceArtifact": {"root": "build/provider", "entrypoint": "bin/asp-fixture"},
                    "workspaceBuild": {
                        "program": "cargo",
                        "args": ["build"],
                        "workingDirectory": ".",
                        "sourceSnapshotAnchors": ["Cargo.toml"],
                        "derivedPaths": ["build/provider"],
                        "env": {}
                    }
                }),
            )
            .expect("descriptor");
            descriptor.validate().expect("bundle-relative schema");
        }
    }
}
