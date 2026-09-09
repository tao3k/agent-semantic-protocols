// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Pre-admission client measurements that Runtime may enrich after admission.

pub const RUNTIME_SEARCH_CLIENT_TIMING_WITNESS_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-search-client-timing-witness";
pub const RUNTIME_SEARCH_CLIENT_TIMING_WITNESS_SCHEMA_VERSION: &str = "1";
pub const RUNTIME_SEARCH_CLIENT_TIMING_PHASES: [&str; 3] =
    ["launcher", "client-frame-encode", "ipc-connect"];

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeSearchClientTimingPhase {
    pub name: String,
    pub elapsed_micros: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeSearchClientTimingWitness {
    pub schema_id: String,
    pub schema_version: String,
    pub session_id: String,
    pub request_id: String,
    pub phases: [RuntimeSearchClientTimingPhase; 3],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSearchClientTimingError {
    reason_kind: &'static str,
}

impl RuntimeSearchClientTimingError {
    pub fn reason_kind(&self) -> &str {
        self.reason_kind
    }
}

impl RuntimeSearchClientTimingWitness {
    pub fn new(
        session_id: impl Into<String>,
        request_id: impl Into<String>,
        elapsed_micros: [u64; 3],
    ) -> Result<Self, RuntimeSearchClientTimingError> {
        let witness = Self {
            schema_id: RUNTIME_SEARCH_CLIENT_TIMING_WITNESS_SCHEMA_ID.into(),
            schema_version: RUNTIME_SEARCH_CLIENT_TIMING_WITNESS_SCHEMA_VERSION.into(),
            session_id: session_id.into(),
            request_id: request_id.into(),
            phases: std::array::from_fn(|index| RuntimeSearchClientTimingPhase {
                name: RUNTIME_SEARCH_CLIENT_TIMING_PHASES[index].into(),
                elapsed_micros: elapsed_micros[index],
            }),
        };
        witness.validate()?;
        Ok(witness)
    }

    pub fn validate(&self) -> Result<(), RuntimeSearchClientTimingError> {
        if self.schema_id != RUNTIME_SEARCH_CLIENT_TIMING_WITNESS_SCHEMA_ID
            || self.schema_version != RUNTIME_SEARCH_CLIENT_TIMING_WITNESS_SCHEMA_VERSION
            || self.session_id.trim().is_empty()
            || self.request_id.trim().is_empty()
        {
            return Err(RuntimeSearchClientTimingError {
                reason_kind: "runtime-search-client-timing-identity-mismatch",
            });
        }
        if self
            .phases
            .iter()
            .zip(RUNTIME_SEARCH_CLIENT_TIMING_PHASES)
            .any(|(phase, expected)| phase.name != expected)
        {
            return Err(RuntimeSearchClientTimingError {
                reason_kind: "runtime-search-client-timing-phase-order",
            });
        }
        Ok(())
    }

    pub fn admit_for_request(
        &self,
        session_id: &str,
        request_id: &str,
    ) -> Result<(), RuntimeSearchClientTimingError> {
        self.validate()?;
        if self.session_id != session_id || self.request_id != request_id {
            return Err(RuntimeSearchClientTimingError {
                reason_kind: "runtime-search-client-timing-identity-mismatch",
            });
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/unit/client_timing.rs"]
mod tests;
