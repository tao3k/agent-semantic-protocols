use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInstallReceipt {
    pub language_id: String,
    pub provider_id: String,
    pub installed_path: PathBuf,
    pub installed_entrypoint_digest: String,
    pub installed_entrypoint_metadata_digest: String,
    pub execution_command_digest: String,
}
