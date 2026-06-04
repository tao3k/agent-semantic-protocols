//! Request model passed from `agent-semantic-client` to execution backends.

use std::path::PathBuf;

use crate::types::LanguageId;
use serde::{Deserialize, Serialize};

/// Shared agent-facing method routed by the client.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientMethod {
    Guide,
    Providers,
    Doctor,
    CacheStatus,
    CacheImport,
    CacheInvalidate,
    Search,
    Query,
    Check,
}

/// Client request sent to a local, cache, or future cloud backend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientRequest {
    pub method: ClientMethod,
    pub language_id: Option<LanguageId>,
    pub forwarded_args: Vec<String>,
    pub project_root: PathBuf,
}

impl ClientRequest {
    /// Create a request for a method and project root.
    #[must_use]
    pub fn new(method: ClientMethod, project_root: impl Into<PathBuf>) -> Self {
        Self {
            method,
            language_id: None,
            forwarded_args: Vec::new(),
            project_root: project_root.into(),
        }
    }

    /// Attach an explicit language id to the request.
    #[must_use]
    pub fn with_language(mut self, language_id: impl Into<LanguageId>) -> Self {
        self.language_id = Some(language_id.into());
        self
    }

    /// Attach provider-native forwarded arguments.
    #[must_use]
    pub fn with_forwarded_args(mut self, forwarded_args: Vec<String>) -> Self {
        self.forwarded_args = forwarded_args;
        self
    }
}
