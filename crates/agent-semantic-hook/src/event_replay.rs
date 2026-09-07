// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use serde_json::Value;
use serde_json::json;

use crate::protocol::DecisionKind;
use crate::protocol::HookDecision;

pub(crate) fn deny_replay_key(decision: &HookDecision) -> Option<String> {
    if decision.decision != DecisionKind::Deny {
        return None;
    }
    let reason = serde_json::to_value(decision.reason_kind).ok()?;
    let mut language_ids = decision.language_ids.clone();
    language_ids.sort();
    language_ids.dedup();
    let routes = decision
        .routes
        .iter()
        .map(|route| {
            json!({
                "languageId": route.language_id,
                "providerId": route.provider_id,
                "kind": route.kind,
                "argv": route.argv,
            })
        })
        .collect::<Vec<_>>();
    if is_source_access_replay_reason(reason.as_str()) {
        let key = json!({
            "platform": decision.platform,
            "replayFamily": "source-access-recovery",
            "reasonKind": reason,
            "languageIds": language_ids,
            "routes": routes,
            "subject": decision.subject,
            "cwd": decision.fields.get("cwd").cloned().unwrap_or(Value::Null),
            "sessionId": decision.fields.get("sessionId").cloned().unwrap_or(Value::Null),
            "transcriptPath": decision.fields.get("transcriptPath").cloned().unwrap_or(Value::Null),
        });
        return serde_json::to_string(&key).ok();
    }
    let subject = if routes.is_empty() {
        serde_json::to_value(&decision.subject).unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    let key = json!({
        "platform": decision.platform,
        "reasonKind": reason,
        "languageIds": language_ids,
        "operationIntent": decision.fields.get("operationIntent").cloned().unwrap_or(Value::Null),
        "toolSurface": decision.fields.get("toolSurface").cloned().unwrap_or(Value::Null),
        "sessionId": decision.fields.get("sessionId").cloned().unwrap_or(Value::Null),
        "transcriptPath": decision.fields.get("transcriptPath").cloned().unwrap_or(Value::Null),
        "routes": routes,
        "subject": subject,
    });
    serde_json::to_string(&key).ok()
}

fn is_source_access_replay_reason(reason: Option<&str>) -> bool {
    matches!(
        reason,
        Some(
            "bulk-source-dump"
                | "registered-source-route-required"
                | "structured-source-read"
                | "raw-broad-search"
                | "source-directory-enumeration"
        )
    )
}

fn structured_source_read_repeated_message(
    reason: &str,
    configured_message: &str,
    recovery_ref: &str,
) -> Option<String> {
    (reason == "structured-source-read")
        .then(|| format!("{configured_message}\nrecoveryRef={recovery_ref}"))
}

#[cfg(test)]
#[path = "../tests/unit/event_replay.rs"]
mod event_replay_tests;

pub(crate) fn recovery_ref_for_replay_key(replay_key: &str) -> String {
    let prefix = if is_source_access_replay_key(replay_key) {
        "source-access"
    } else {
        "hook-deny"
    };
    format!("{prefix}:{}", replay_key_hash(replay_key))
}

pub(crate) fn is_source_access_replay_key(replay_key: &str) -> bool {
    serde_json::from_str::<Value>(replay_key)
        .ok()
        .and_then(|value| {
            value
                .get("replayFamily")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .as_deref()
        == Some("source-access-recovery")
}

pub(crate) fn compact_source_access_deny_message(
    decision: &HookDecision,
    recovery_ref: &str,
) -> String {
    let reason = replay_reason_label(decision);
    if decision.fields.get("denyReplay").and_then(Value::as_str) == Some("repeated") {
        if let Some(message) =
            structured_source_read_repeated_message(&reason, &decision.message, recovery_ref)
        {
            return message;
        }
        return format!(
            "ASP denied source access again (`{reason}`). Use `collaboration.spawn_agent` with the Config-resolved `agent_type`, then verify the canonical child with `collaboration.list_agents`.\nrecoveryRef={recovery_ref}"
        );
    }

    if decision
        .fields
        .get("subagentContext")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return format!(
            "ASP denied source access (`{reason}`) inside a delegated Agent. Use ASP Search Playbook and return Search success as exactly one Org/GQL source block with `:profile search-evidence.v1 :eval never`; do not return prose, source bodies, snippets, a flat receipt, command grammar, or a prescribed next action.\nrecoveryRef={recovery_ref}"
        );
    }

    format!(
        "ASP denied source access (`{reason}`). Use `collaboration.spawn_agent` with the Config-resolved `agent_type`, then verify the canonical child with `collaboration.list_agents`.\nrecoveryRef={recovery_ref}"
    )
}

pub(crate) fn should_compact_source_access_deny_message(decision: &HookDecision) -> bool {
    !decision.fields.contains_key("configRuleId")
        && (decision.message.starts_with("ASP hook blocked `")
            || decision.message.starts_with("ASP denied `"))
}

pub(crate) fn repeated_deny_message(decision: &HookDecision) -> String {
    let reason = replay_reason_label(decision);
    [
        format!("ASP hook already denied `{reason}` on this source-access lane."),
        "See @.agents/skills/agent-semantic-protocols/SKILL.md for the active ASP agent workflow."
            .to_string(),
        String::new(),
        "## ASP Hook Recovery".to_string(),
        "Use `collaboration.spawn_agent` with the Config-resolved `agent_type`, then verify the canonical child with `collaboration.list_agents`.".to_string(),
        String::new(),
        "## Stop".to_string(),
        "Do not switch to another evidence channel. The hook has already denied this lane."
            .to_string(),
    ]
    .join("\n")
}

fn replay_reason_label(decision: &HookDecision) -> String {
    serde_json::to_value(decision.reason_kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "source-access".to_string())
}

fn replay_key_hash(replay_key: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in replay_key.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
