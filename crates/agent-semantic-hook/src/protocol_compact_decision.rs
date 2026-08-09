//! Compact immutable decision shard encoding for the one-shot Hook reader.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    DecisionKind, DecisionRoute, DecisionRouteKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID,
    HOOK_DECISION_SCHEMA_VERSION, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision,
    ReasonKind, StdinMode,
};

#[derive(Serialize)]
struct CompactSubjectRef<'a> {
    tool_name: &'a Option<String>,
    command: &'a Option<String>,
    paths: &'a [String],
}

#[derive(Serialize)]
struct CompactRouteRef<'a> {
    language_id: &'a agent_semantic_config::LanguageId,
    provider_id: &'a agent_semantic_config::ProviderId,
    binary: &'a str,
    kind: &'a DecisionRouteKind,
    argv: &'a [String],
    stdin_mode: &'a Option<StdinMode>,
}

#[derive(Serialize)]
struct CompactDecisionRef<'a> {
    platform: &'a str,
    event: &'a str,
    decision: &'a DecisionKind,
    reason_kind: &'a ReasonKind,
    language_ids: &'a [agent_semantic_config::LanguageId],
    subject: CompactSubjectRef<'a>,
    routes: Vec<CompactRouteRef<'a>>,
    message: &'a str,
    fields_json: &'a [u8],
}

#[derive(Deserialize)]
struct CompactSubject {
    tool_name: Option<String>,
    command: Option<String>,
    paths: Vec<String>,
}

#[derive(Deserialize)]
struct CompactRoute {
    language_id: agent_semantic_config::LanguageId,
    provider_id: agent_semantic_config::ProviderId,
    binary: String,
    kind: DecisionRouteKind,
    argv: Vec<String>,
    stdin_mode: Option<StdinMode>,
}

#[derive(Deserialize)]
struct CompactDecision {
    platform: String,
    event: String,
    decision: DecisionKind,
    reason_kind: ReasonKind,
    language_ids: Vec<agent_semantic_config::LanguageId>,
    subject: CompactSubject,
    routes: Vec<CompactRoute>,
    message: String,
    fields_json: Vec<u8>,
}

pub(super) fn encode(decision: &HookDecision) -> Result<Vec<u8>, String> {
    let fields_json = serde_json::to_vec(&decision.fields)
        .map_err(|error| format!("encode compact Hook decision fields: {error}"))?;
    postcard::to_allocvec(&CompactDecisionRef {
        platform: &decision.platform,
        event: &decision.event,
        decision: &decision.decision,
        reason_kind: &decision.reason_kind,
        language_ids: &decision.language_ids,
        subject: CompactSubjectRef {
            tool_name: &decision.subject.tool_name,
            command: &decision.subject.command,
            paths: &decision.subject.paths,
        },
        routes: decision.routes.iter().map(compact_route_ref).collect(),
        message: &decision.message,
        fields_json: &fields_json,
    })
    .map_err(|error| format!("encode compact Hook decision: {error}"))
}

fn compact_route_ref(route: &DecisionRoute) -> CompactRouteRef<'_> {
    CompactRouteRef {
        language_id: &route.language_id,
        provider_id: &route.provider_id,
        binary: &route.binary,
        kind: &route.kind,
        argv: &route.argv,
        stdin_mode: &route.stdin_mode,
    }
}

pub(super) fn decode(bytes: &[u8]) -> Result<HookDecision, String> {
    let compact = postcard::from_bytes::<CompactDecision>(bytes)
        .map_err(|error| format!("decode compact Hook decision: {error}"))?;
    let fields = serde_json::from_slice::<BTreeMap<String, Value>>(&compact.fields_json)
        .map_err(|error| format!("decode compact Hook decision fields: {error}"))?;
    Ok(HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: compact.platform,
        event: compact.event,
        decision: compact.decision,
        reason_kind: compact.reason_kind,
        language_ids: compact.language_ids,
        subject: DecisionSubject {
            tool_name: compact.subject.tool_name,
            command: compact.subject.command,
            paths: compact.subject.paths,
        },
        routes: compact
            .routes
            .into_iter()
            .map(DecisionRoute::from)
            .collect(),
        message: compact.message,
        fields,
    })
}

impl From<CompactRoute> for DecisionRoute {
    fn from(route: CompactRoute) -> Self {
        Self {
            language_id: route.language_id,
            provider_id: route.provider_id,
            binary: route.binary,
            kind: route.kind,
            argv: route.argv,
            stdin_mode: route.stdin_mode,
        }
    }
}

pub(super) fn replace_template_marker(
    decision: &mut HookDecision,
    marker: &str,
    source_path: &str,
) -> bool {
    let mut replaced = replace_marker(&mut decision.platform, marker, source_path);
    replaced |= replace_marker(&mut decision.event, marker, source_path);
    replaced |= replace_marker(&mut decision.message, marker, source_path);
    replaced |= decision
        .subject
        .tool_name
        .as_mut()
        .is_some_and(|value| replace_marker(value, marker, source_path));
    replaced |= decision
        .subject
        .command
        .as_mut()
        .is_some_and(|value| replace_marker(value, marker, source_path));
    replaced |= decision
        .subject
        .paths
        .iter_mut()
        .fold(false, |seen, value| {
            replace_marker(value, marker, source_path) | seen
        });
    replaced |= decision.routes.iter_mut().fold(false, |seen, route| {
        let route_replaced = replace_marker(&mut route.binary, marker, source_path)
            | route.argv.iter_mut().fold(false, |seen, value| {
                replace_marker(value, marker, source_path) | seen
            });
        route_replaced | seen
    });
    replaced
        | decision.fields.values_mut().fold(false, |seen, value| {
            replace_value_marker(value, marker, source_path) | seen
        })
}

fn replace_marker(value: &mut String, marker: &str, replacement: &str) -> bool {
    if !value.contains(marker) {
        return false;
    }
    *value = value.replace(marker, replacement);
    true
}

fn replace_value_marker(value: &mut Value, marker: &str, replacement: &str) -> bool {
    match value {
        Value::String(text) => replace_marker(text, marker, replacement),
        Value::Array(values) => values.iter_mut().fold(false, |seen, value| {
            replace_value_marker(value, marker, replacement) | seen
        }),
        Value::Object(values) => values.values_mut().fold(false, |seen, value| {
            replace_value_marker(value, marker, replacement) | seen
        }),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}
