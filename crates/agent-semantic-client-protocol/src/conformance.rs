// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Executable request/response cases for validating a Runtime client implementation.

use serde::Deserialize;
use serde::Serialize;

use crate::ClientFrame;
use crate::ClientOutcome;
use crate::ClientReasonKind;

/// Versioned collection of client protocol conformance cases.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientConformanceSuite {
    /// Optional JSON Schema document URI.
    #[serde(rename = "$schema", default)]
    pub schema: Option<String>,
    /// Suite schema identity.
    pub schema_id: String,
    /// Suite schema version.
    pub schema_version: String,
    /// Protocol identity exercised by the suite.
    pub protocol_id: String,
    /// Protocol version exercised by the suite.
    pub protocol_version: String,
    /// Ordered executable cases.
    pub cases: Vec<ClientConformanceCase>,
}

/// One request and its required terminal outcome.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientConformanceCase {
    /// Stable case identity.
    pub case_id: String,
    /// Request frame submitted to the implementation.
    pub request: ClientFrame,
    /// Expected terminal outcome.
    pub expected_outcome: ClientOutcome,
    /// Expected typed reason when the outcome is not accepted.
    #[serde(default)]
    pub expected_reason_kind: Option<ClientReasonKind>,
}

/// Observed terminal result for one conformance case.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientConformanceReceipt {
    /// Stable case identity.
    pub case_id: String,
    /// Observed terminal outcome.
    pub outcome: ClientOutcome,
    /// Observed typed reason, when present.
    pub reason_kind: Option<ClientReasonKind>,
}

/// Typed admission failure returned by a conformance executor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientAdmissionError {
    /// Stable machine-readable failure reason.
    pub reason_kind: &'static str,
    /// Human-readable diagnostic.
    pub message: String,
}
