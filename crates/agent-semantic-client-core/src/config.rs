//! User and project configuration model for `agent-semantic-client`.

use serde::{Deserialize, Serialize};

/// Backend mode selected outside the agent prompt command.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackendMode {
    Local,
    Cloud,
    Hybrid,
}

/// Upload/privacy mode for local and cloud client routes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PrivacyMode {
    None,
    SemanticIndex,
    SourceIndex,
}

/// Minimal client configuration shared by local and future cloud routes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientConfig {
    pub backend_mode: BackendMode,
    pub privacy_mode: PrivacyMode,
    pub enabled_providers: Vec<String>,
}

impl ClientConfig {
    /// Return the local-only default configuration.
    #[must_use]
    pub fn local_default() -> Self {
        Self {
            backend_mode: BackendMode::Local,
            privacy_mode: PrivacyMode::None,
            enabled_providers: vec![
                "rust".to_string(),
                "typescript".to_string(),
                "python".to_string(),
            ],
        }
    }
}
