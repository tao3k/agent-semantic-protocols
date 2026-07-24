use std::collections::BTreeMap;

use serde::Deserialize;

use super::install_provider::WorkspaceBuildSpec;
use super::install_provider_workspace_artifact::WorkspaceArtifactSpec;
use super::install_provider_workspace_materialization::WorkspaceDependencyMaterializationSpec;

const WORKSPACE_INSTALL_SCHEMA_ID: &str = "agent.semantic-protocols.provider-workspace-install";
const WORKSPACE_INSTALL_SCHEMA_VERSION: &str = "1";
const CANONICAL_SCHEMA_AUTHORITY: &str =
    "https://tao3k.github.io/agent-semantic-protocols/schemas/";

const PROVIDER_MANIFEST_SOURCES: [&str; 7] = [
    include_str!(
        "../../../../languages/rust-lang-project-harness/provider/asp-provider-manifest.json"
    ),
    include_str!(
        "../../../../languages/typescript-lang-project-harness/provider/asp-provider-manifest.json"
    ),
    include_str!(
        "../../../../languages/python-lang-project-harness/provider/asp-provider-manifest.json"
    ),
    include_str!(
        "../../../../languages/gerbil-scheme-language-project-harness/provider/asp-provider-manifest.json"
    ),
    include_str!(
        "../../../../languages/JuliaLangProjectHarness.jl/juliac/asp-provider-manifest.json"
    ),
    include_str!("../../../../languages/org/provider/asp-org-provider-manifest.json"),
    include_str!("../../../../languages/org/provider/asp-md-provider-manifest.json"),
];

const WORKSPACE_INSTALL_DESCRIPTOR_SOURCES: [&str; 6] = [
    include_str!(
        "../../../../languages/rust-lang-project-harness/provider/asp-provider-workspace-install.json"
    ),
    include_str!(
        "../../../../languages/typescript-lang-project-harness/provider/asp-provider-workspace-install.json"
    ),
    include_str!(
        "../../../../languages/python-lang-project-harness/provider/asp-provider-workspace-install.json"
    ),
    include_str!(
        "../../../../languages/JuliaLangProjectHarness.jl/juliac/asp-provider-workspace-install.json"
    ),
    include_str!(
        "../../../../languages/gerbil-scheme-language-project-harness/provider/asp-provider-workspace-install.json"
    ),
    include_str!("../../../../languages/orgize/provider/asp-provider-workspace-install.json"),
];

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ProviderWorkspaceInstallDescriptor {
    schema_id: String,
    schema_version: String,
    schema_authority: String,
    pub(super) provider_id: String,
    pub(super) binary: String,
    pub(super) workspace_artifact: WorkspaceArtifactSpec,
    #[serde(default)]
    pub(super) dependency_materialization: Option<WorkspaceDependencyMaterializationSpec>,
    pub(super) workspace_build: WorkspaceBuildSpec,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderManifestIdentity {
    language_id: String,
    provider_id: String,
    binary: String,
}

pub(super) fn workspace_install_descriptor_for_language(
    language_id: &str,
) -> Result<ProviderWorkspaceInstallDescriptor, String> {
    let manifest = provider_manifest_for_language(language_id)?;
    let mut descriptors = BTreeMap::new();
    for source in WORKSPACE_INSTALL_DESCRIPTOR_SOURCES {
        let descriptor: ProviderWorkspaceInstallDescriptor = serde_json::from_str(source)
            .map_err(|error| format!("invalid provider workspace install descriptor: {error}"))?;
        validate_descriptor_identity(&descriptor)?;
        let descriptor_provider_id = descriptor.provider_id.clone();
        if descriptors
            .insert(descriptor_provider_id.clone(), descriptor)
            .is_some()
        {
            return Err(format!(
                "duplicate workspace install descriptor for provider `{descriptor_provider_id}`"
            ));
        }
    }
    let descriptor = descriptors.remove(&manifest.provider_id).ok_or_else(|| {
        format!(
            "missing workspace install descriptor for provider `{}` (language `{language_id}`)",
            manifest.provider_id
        )
    })?;
    if descriptor.binary != manifest.binary {
        return Err(format!(
            "workspace install descriptor binary `{}` does not match provider manifest binary `{}` for provider `{}`",
            descriptor.binary, manifest.binary, manifest.provider_id
        ));
    }
    Ok(descriptor)
}

fn provider_manifest_for_language(language_id: &str) -> Result<ProviderManifestIdentity, String> {
    let mut providers_by_language = BTreeMap::new();
    for source in PROVIDER_MANIFEST_SOURCES {
        let manifest: ProviderManifestIdentity = serde_json::from_str(source)
            .map_err(|error| format!("invalid provider manifest identity: {error}"))?;
        let manifest_language_id = manifest.language_id.clone();
        if providers_by_language
            .insert(manifest_language_id.clone(), manifest)
            .is_some()
        {
            return Err(format!(
                "duplicate provider manifest for language `{manifest_language_id}`"
            ));
        }
    }
    providers_by_language
        .remove(language_id)
        .ok_or_else(|| format!("no provider manifest for language `{language_id}`"))
}

fn validate_descriptor_identity(
    descriptor: &ProviderWorkspaceInstallDescriptor,
) -> Result<(), String> {
    if descriptor.schema_id != WORKSPACE_INSTALL_SCHEMA_ID {
        return Err(format!(
            "workspace install descriptor `{}` has schemaId `{}`; expected `{WORKSPACE_INSTALL_SCHEMA_ID}`",
            descriptor.provider_id, descriptor.schema_id
        ));
    }
    if descriptor.schema_version != WORKSPACE_INSTALL_SCHEMA_VERSION {
        return Err(format!(
            "workspace install descriptor `{}` has schemaVersion `{}`; expected `{WORKSPACE_INSTALL_SCHEMA_VERSION}`",
            descriptor.provider_id, descriptor.schema_version
        ));
    }
    if descriptor.schema_authority != CANONICAL_SCHEMA_AUTHORITY {
        return Err(format!(
            "workspace install descriptor `{}` has schemaAuthority `{}`; expected `{CANONICAL_SCHEMA_AUTHORITY}`",
            descriptor.provider_id, descriptor.schema_authority
        ));
    }
    if descriptor.provider_id.is_empty() {
        return Err("workspace install descriptor providerId must not be empty".to_string());
    }
    if descriptor.binary.is_empty() {
        return Err(format!(
            "workspace install descriptor binary must not be empty for provider `{}`",
            descriptor.provider_id
        ));
    }
    Ok(())
}
