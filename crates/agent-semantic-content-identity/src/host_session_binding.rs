// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Typed Host session lineage used to bind Runtime observations without inference.

use serde::Deserialize;
use serde::Serialize;

use crate::HostPlatformV1;
use crate::HostSessionIdV1;

/// Schema identifier for a typed Host session-lineage binding.
pub const HOST_SESSION_SCHEMA_ID: &str = "asp.host-session-binding";
/// Schema version for Host session-lineage bindings.
pub const HOST_SESSION_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Explicit root, parent, and current Host session identities.
pub struct HostSessionBinding {
    pub schema_id: String,
    pub schema_version: String,
    pub platform: HostPlatformV1,
    pub root_session_id: HostSessionIdV1,
    pub parent_session_id: HostSessionIdV1,
    pub current_session_id: HostSessionIdV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Deterministic validation error for a Host session binding.
pub enum HostSessionBindingError {
    SchemaMismatch,
    MissingField(&'static str),
    ParentEqualsCurrent,
    ConflictingIdentity,
}

impl HostSessionBinding {
    pub fn new(
        platform: impl Into<HostPlatformV1>,
        root_session_id: impl Into<HostSessionIdV1>,
        parent_session_id: impl Into<HostSessionIdV1>,
        current_session_id: impl Into<HostSessionIdV1>,
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
            ("platform", self.platform.as_str()),
            ("rootSessionId", self.root_session_id.as_str()),
            ("parentSessionId", self.parent_session_id.as_str()),
            ("currentSessionId", self.current_session_id.as_str()),
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
