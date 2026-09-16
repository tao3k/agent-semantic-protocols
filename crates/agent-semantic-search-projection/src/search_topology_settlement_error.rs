// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Typed failures for Search topology settlement admission and rendering.

use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchTopologySettlementError {
    reason_kind: &'static str,
    message: String,
}

impl SearchTopologySettlementError {
    pub fn reason_kind(&self) -> &'static str {
        self.reason_kind
    }
}

impl fmt::Display for SearchTopologySettlementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.reason_kind, self.message)
    }
}

impl std::error::Error for SearchTopologySettlementError {}

pub(super) fn error(
    reason_kind: &'static str,
    message: impl Into<String>,
) -> SearchTopologySettlementError {
    SearchTopologySettlementError {
        reason_kind,
        message: message.into(),
    }
}

pub(super) fn invalid<T>(
    reason_kind: &'static str,
    message: impl Into<String>,
) -> Result<T, SearchTopologySettlementError> {
    Err(error(reason_kind, message))
}
