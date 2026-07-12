//! Shared release metadata for provider installation.

use serde::Deserialize;

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum WorkspaceInstallMode {
    #[default]
    Copy,
    Symlink,
}

impl WorkspaceInstallMode {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Copy => "copy",
            Self::Symlink => "symlink",
        }
    }
}

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
    pub(super) workspace_binary: Option<String>,
    pub(super) workspace_install: WorkspaceInstallMode,
    pub(super) supported_targets: Vec<String>,
}
