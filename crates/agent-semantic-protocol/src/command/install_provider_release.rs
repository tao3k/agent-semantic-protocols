//! Shared release metadata for provider installation.

#[derive(Clone, Debug)]
pub(super) struct ProviderReleaseSpec {
    pub(super) language_id: String,
    pub(super) provider_id: String,
    pub(super) repo: String,
    pub(super) release_version: String,
    pub(super) download_base_url: String,
    pub(super) binary: String,
    pub(super) archive_prefix: String,
    pub(super) archive_binary: String,
    pub(super) require_native_binary: bool,
    pub(super) workspace_artifact:
        Option<super::install_provider_workspace_artifact::WorkspaceArtifactSpec>,
    pub(super) workspace_build: Option<super::install_provider::WorkspaceBuildSpec>,
    pub(super) supported_targets: Vec<String>,
}
