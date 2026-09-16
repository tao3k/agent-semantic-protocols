// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Shared typed values for Live Corpus Search and Query protocol drivers.

use agent_semantic_client_protocol::AspClientExactQueryFailure;
use agent_semantic_client_protocol::ClientFrame;
use agent_semantic_client_protocol::ClientOutcome;

#[derive(Debug, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PublicRouteFailure {
    reason_kind: String,
    message: String,
    #[serde(default)]
    details: Option<serde_json::Value>,
    #[serde(default)]
    terminal: Option<serde_json::Value>,
}

#[derive(Debug, PartialEq)]
pub(crate) enum PublicRouteTerminal {
    Queued(AspClientExactQueryFailure),
    Building(AspClientExactQueryFailure),
    Ready(serde_json::Value),
    Failed(PublicRouteFailure),
    Cancelled,
}

impl PublicRouteTerminal {
    pub(crate) fn require_ready(self, route: &str) -> Result<serde_json::Value, String> {
        match self {
            Self::Ready(payload) => Ok(payload),
            Self::Queued(failure) => Err(format!(
                "Live Corpus public route remained Queued: route={route} reasonKind={} phase={}",
                failure.reason_kind, failure.phase
            )),
            Self::Building(failure) => Err(format!(
                "Live Corpus public route remained Building: route={route} reasonKind={} phase={}",
                failure.reason_kind, failure.phase
            )),
            Self::Failed(failure) => Err(format!(
                "Live Corpus public route failed: route={route} reasonKind={} message={} terminal={:?}",
                failure.reason_kind, failure.message, failure.terminal
            )),
            Self::Cancelled => Err(format!(
                "Live Corpus public route was cancelled: route={route}"
            )),
        }
    }
}

pub(crate) fn typed_terminal(frame: ClientFrame) -> Result<PublicRouteTerminal, String> {
    match frame {
        ClientFrame::Response {
            outcome: ClientOutcome::Ready,
            result: Some(payload),
            error: None,
            ..
        } => Ok(PublicRouteTerminal::Ready(payload.into_value())),
        ClientFrame::Response {
            outcome: ClientOutcome::Cancelled,
            ..
        } => Ok(PublicRouteTerminal::Cancelled),
        ClientFrame::Response {
            outcome: ClientOutcome::Error | ClientOutcome::StaleGeneration,
            error: Some(error),
            ..
        } => {
            let failure = serde_json::from_value::<PublicRouteFailure>(error.clone()).map_err(
                |decode_error| {
                    format!(
                        "decode Live Corpus typed route failure: {decode_error}; payload={error}"
                    )
                },
            )?;
            if let Some(details) = failure.details.as_ref()
                && let Ok(generation_failure) =
                    serde_json::from_value::<AspClientExactQueryFailure>(details.clone())
            {
                return Ok(match generation_failure.reason_kind.as_str() {
                    "runtime-generation-queued" => PublicRouteTerminal::Queued(generation_failure),
                    "runtime-generation-building" => {
                        PublicRouteTerminal::Building(generation_failure)
                    }
                    _ => PublicRouteTerminal::Failed(failure),
                });
            }
            Ok(PublicRouteTerminal::Failed(failure))
        }
        other => Err(format!(
            "Live Corpus public route returned an invalid terminal frame: {other:?}"
        )),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WorkspaceSearchQualificationReceipt {
    pub(crate) operation_id: String,
    pub(crate) source_generation_digest: String,
    pub(crate) provider_catalog_digest: String,
    pub(crate) topology_generation_digest: String,
    pub(crate) selectors: Vec<String>,
    pub(crate) owner_paths: Vec<String>,
    pub(crate) elapsed_micros: u64,
    pub(crate) response_decode_elapsed_micros: u64,
    pub(crate) packet_bytes: usize,
    pub(crate) node_count: usize,
    pub(crate) edge_count: usize,
    pub(crate) frontier_count: usize,
    pub(crate) coverage_certificate_count: usize,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ResidentSearchLatencyBudget {
    pub(crate) p50_micros: u64,
    pub(crate) p99_micros: u64,
    pub(crate) max_micros: u64,
}

pub(crate) fn registered_producer_axis(producer_id: &str) -> Result<&'static str, String> {
    let profile = include_str!("../../../../../schemas/language-schema-profiles.json");
    let profile: serde_json::Value = serde_json::from_str(profile)
        .map_err(|error| format!("decode embedded Search producer profile registry: {error}"))?;
    let profiles = profile
        .get("profiles")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "Search producer profile registry has no profiles".to_owned())?;
    let producer = profiles
        .iter()
        .find(|profile| {
            profile
                .get("languageId")
                .and_then(serde_json::Value::as_str)
                == Some(producer_id)
        })
        .ok_or_else(|| format!("Search producer profile is not registered: {producer_id}"))?;
    let axes = producer
        .get("searchProducerAxes")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("Search producer profile has no registered axis: {producer_id}"))?;
    let language = axes.iter().any(|axis| axis.as_str() == Some("language"));
    let documents = axes.iter().any(|axis| axis.as_str() == Some("document"));
    match (language, documents) {
        (true, false) => Ok("language"),
        (false, true) => Ok("documents"),
        _ => Err(format!(
            "Live Corpus producer must resolve to exactly one language/document axis: {producer_id}"
        )),
    }
}
