// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Typed failures shared by topology generation inputs and projections.

use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTopologyGenerationBuildError {
    reason_kind: &'static str,
    message: String,
}

impl ProjectTopologyGenerationBuildError {
    pub const fn reason_kind(&self) -> &'static str {
        self.reason_kind
    }
}

impl fmt::Display for ProjectTopologyGenerationBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.reason_kind, self.message)
    }
}

impl std::error::Error for ProjectTopologyGenerationBuildError {}

impl From<crate::ProjectTopologyClosureError> for ProjectTopologyGenerationBuildError {
    fn from(cause: crate::ProjectTopologyClosureError) -> Self {
        error(cause.reason_kind(), cause.to_string())
    }
}

pub(crate) fn error(
    reason_kind: &'static str,
    message: impl Into<String>,
) -> ProjectTopologyGenerationBuildError {
    ProjectTopologyGenerationBuildError {
        reason_kind,
        message: message.into(),
    }
}

pub(crate) fn validate_identifier(
    value: &str,
    name: &str,
) -> Result<(), ProjectTopologyGenerationBuildError> {
    if value.is_empty()
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_'
        })
        || !value.as_bytes()[0].is_ascii_lowercase()
    {
        return Err(error(
            "topology-generation-identity-invalid",
            format!("{name} is not canonical"),
        ));
    }
    Ok(())
}

pub(crate) fn require_digest(
    digest: &str,
    name: &str,
) -> Result<(), ProjectTopologyGenerationBuildError> {
    if digest.strip_prefix("blake3-256:").is_some_and(|suffix| {
        suffix.len() == 64
            && suffix
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    }) {
        Ok(())
    } else {
        Err(error(
            "topology-generation-digest-invalid",
            format!("{name} must be a canonical blake3-256 digest"),
        ))
    }
}
