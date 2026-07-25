use crate::types::{ClientDbArtifactEvent, ClientDbArtifactEventStorageColumns};
use agent_semantic_client_core::LanguageId;

/// Validating construction state for a graph-turbo artifact event.
#[derive(Debug, Default)]
pub struct ClientDbArtifactEventBuilder {
    artifact_path: Option<String>,
    event_ordinal: Option<u32>,
    timestamp_ms: Option<i64>,
    kind: Option<String>,
    language: Option<LanguageId>,
    method: Option<String>,
    target: Option<String>,
    query: Option<String>,
    project_root: Option<String>,
    project_root_arg: Option<String>,
    bytes: Option<u64>,
}

impl ClientDbArtifactEventBuilder {
    #[must_use]
    pub fn artifact_path(mut self, value: impl Into<String>) -> Self {
        self.artifact_path = Some(value.into());
        self
    }

    #[must_use]
    pub fn event_ordinal(mut self, value: u32) -> Self {
        self.event_ordinal = Some(value);
        self
    }

    #[must_use]
    pub fn timestamp_ms(mut self, value: i64) -> Self {
        self.timestamp_ms = Some(value);
        self
    }

    #[must_use]
    pub fn kind(mut self, value: impl Into<String>) -> Self {
        self.kind = Some(value.into());
        self
    }

    #[must_use]
    pub fn language(mut self, value: LanguageId) -> Self {
        self.language = Some(value);
        self
    }

    #[must_use]
    pub fn method(mut self, value: impl Into<String>) -> Self {
        self.method = Some(value.into());
        self
    }

    #[must_use]
    pub fn target(mut self, value: impl Into<String>) -> Self {
        self.target = Some(value.into());
        self
    }

    #[must_use]
    pub fn query(mut self, value: impl Into<String>) -> Self {
        self.query = Some(value.into());
        self
    }

    #[must_use]
    pub fn project_root(mut self, value: impl Into<String>) -> Self {
        self.project_root = Some(value.into());
        self
    }

    #[must_use]
    pub fn project_root_arg(mut self, value: impl Into<String>) -> Self {
        self.project_root_arg = Some(value.into());
        self
    }

    #[must_use]
    pub fn bytes(mut self, value: u64) -> Self {
        self.bytes = Some(value);
        self
    }

    pub fn build(self) -> Result<ClientDbArtifactEvent, String> {
        let required = |value: Option<String>, field: &str| {
            value.ok_or_else(|| format!("artifact event builder requires `{field}`"))
        };
        let event_ordinal = self
            .event_ordinal
            .ok_or_else(|| "artifact event builder requires `eventOrdinal`".to_string())?;
        let timestamp_ms = self
            .timestamp_ms
            .ok_or_else(|| "artifact event builder requires `timestampMs`".to_string())?;
        let language = self
            .language
            .ok_or_else(|| "artifact event builder requires `language`".to_string())?;
        let bytes = self
            .bytes
            .ok_or_else(|| "artifact event builder requires `bytes`".to_string())?;
        let bytes = i64::try_from(bytes)
            .map_err(|_| "artifact event byte count exceeds the i64 storage domain".to_string())?;
        ClientDbArtifactEvent::from_storage_columns(ClientDbArtifactEventStorageColumns {
            artifact_path: required(self.artifact_path, "artifactPath")?,
            event_ordinal: i64::from(event_ordinal),
            timestamp_ms,
            kind: required(self.kind, "kind")?,
            language: language.into_string(),
            method: required(self.method, "method")?,
            target: required(self.target, "target")?,
            query: required(self.query, "query")?,
            project_root: required(self.project_root, "projectRoot")?,
            project_root_arg: required(self.project_root_arg, "projectRootArg")?,
            bytes,
        })
    }
}

impl ClientDbArtifactEvent {
    #[must_use]
    pub fn builder() -> ClientDbArtifactEventBuilder {
        ClientDbArtifactEventBuilder::default()
    }
}
