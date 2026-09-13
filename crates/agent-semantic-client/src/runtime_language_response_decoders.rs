// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Typed response decoders for Runtime language-client methods.

use agent_semantic_client_protocol::{ClientFrame, SchemaBundleResponse};

pub(crate) fn decode_graph_evaluation_response(
    frame: ClientFrame,
) -> Result<agent_semantic_search_projection::ResidentGraphEvaluationResultV1, String> {
    match frame {
        ClientFrame::Response {
            outcome: agent_semantic_client_protocol::ClientOutcome::Ready,
            result: Some(result),
            error: None,
            ..
        } => agent_semantic_search_projection::ResidentGraphEvaluationResultV1::from_value(result)
            .map_err(|error| format!("decode resident graph evaluation response: {error}")),
        ClientFrame::Response { outcome, error, .. } => Err(format!(
            "graph evaluation dispatch failed: outcome={outcome:?} error={}",
            error.unwrap_or(serde_json::Value::Null)
        )),
        frame => Err(format!(
            "graph evaluation dispatch returned a non-response frame: {frame:?}"
        )),
    }
}

pub(crate) fn decode_schema_bundle_response(
    frame: ClientFrame,
) -> Result<SchemaBundleResponse, String> {
    match frame {
        ClientFrame::Response {
            outcome: agent_semantic_client_protocol::ClientOutcome::Ready,
            result: Some(result),
            error: None,
            ..
        } => {
            let response: SchemaBundleResponse = serde_json::from_value(result)
                .map_err(|error| format!("decode schema bundle response: {error}"))?;
            response.validate()?;
            Ok(response)
        }
        ClientFrame::Response { outcome, error, .. } => Err(format!(
            "schema bundle dispatch failed: outcome={outcome:?} error={}",
            error.unwrap_or(serde_json::Value::Null)
        )),
        frame => Err(format!(
            "schema bundle dispatch returned a non-response frame: {frame:?}"
        )),
    }
}
