use serde::{Deserialize, Serialize};

pub const HOST_SESSION_SCHEMA_ID: &str = "asp.host-session-binding";
pub const HOST_SESSION_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostSessionBinding {
    pub schema_id: String,
    pub schema_version: String,
    pub platform: String,
    pub root_session_id: String,
    pub parent_session_id: String,
    pub current_session_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostSessionBindingError {
    SchemaMismatch,
    MissingField(&'static str),
    ParentEqualsCurrent,
    ConflictingIdentity,
}

impl HostSessionBinding {
    pub fn new(
        platform: impl Into<String>,
        root_session_id: impl Into<String>,
        parent_session_id: impl Into<String>,
        current_session_id: impl Into<String>,
    ) -> Result<Self, HostSessionBindingError> {
        let binding = Self {
            schema_id: HOST_SESSION_SCHEMA_ID.into(),
            schema_version: HOST_SESSION_SCHEMA_VERSION.into(),
            platform: platform.into(),
            root_session_id: root_session_id.into(),
            parent_session_id: parent_session_id.into(),
            current_session_id: current_session_id.into(),
        };
        binding.validate()?;
        Ok(binding)
    }

    pub fn validate(&self) -> Result<(), HostSessionBindingError> {
        if self.schema_id != HOST_SESSION_SCHEMA_ID
            || self.schema_version != HOST_SESSION_SCHEMA_VERSION
        {
            return Err(HostSessionBindingError::SchemaMismatch);
        }
        for (name, value) in [
            ("platform", &self.platform),
            ("rootSessionId", &self.root_session_id),
            ("parentSessionId", &self.parent_session_id),
            ("currentSessionId", &self.current_session_id),
        ] {
            if value.is_empty() {
                return Err(HostSessionBindingError::MissingField(name));
            }
        }
        if self.parent_session_id == self.current_session_id {
            return Err(HostSessionBindingError::ParentEqualsCurrent);
        }
        Ok(())
    }

    pub fn from_environment<F>(mut read: F) -> Result<Self, HostSessionBindingError>
    where
        F: FnMut(&str) -> Option<String>,
    {
        let platform =
            read("ASP_HOST_PLATFORM").ok_or(HostSessionBindingError::MissingField("platform"))?;
        let root = read("ASP_ROOT_SESSION_ID")
            .ok_or(HostSessionBindingError::MissingField("rootSessionId"))?;
        let parent = read("CODEX_SESSION_ID")
            .ok_or(HostSessionBindingError::MissingField("parentSessionId"))?;
        let current = read("CODEX_THREAD_ID")
            .ok_or(HostSessionBindingError::MissingField("currentSessionId"))?;
        Self::new(platform, root, parent, current)
    }
}
