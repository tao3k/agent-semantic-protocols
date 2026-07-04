use serde_json::{Value, json};

use crate::protocol::{DecisionKind, HookDecision};

pub(crate) fn deny_replay_key(decision: &HookDecision) -> Option<String> {
    if decision.decision != DecisionKind::Deny {
        return None;
    }
    let reason = serde_json::to_value(decision.reason_kind).ok()?;
    let mut language_ids = decision.language_ids.clone();
    language_ids.sort();
    language_ids.dedup();
    if matches!(
        reason.as_str(),
        Some(
            "bulk-source-dump"
                | "direct-source-read"
                | "raw-broad-search"
                | "source-directory-enumeration"
        )
    ) {
        let key = json!({
            "platform": decision.platform,
            "replayFamily": "source-access-recovery",
            "cwd": decision.fields.get("cwd").cloned().unwrap_or(Value::Null),
            "sessionId": decision.fields.get("sessionId").cloned().unwrap_or(Value::Null),
            "transcriptPath": decision.fields.get("transcriptPath").cloned().unwrap_or(Value::Null),
        });
        return serde_json::to_string(&key).ok();
    }
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
        if let Some(message) = render_compact_source_access_template(
            decision,
            "sourceAccessCompactRepeatedMessage",
            &reason,
            recovery_ref,
        ) {
            return message;
        }
        return format!(
            "ASP denied source access again (`{reason}`). Use the active recovery lane; do not retry raw source tools.\nrecoveryRef={recovery_ref}"
        );
    }

    if decision
        .fields
        .get("subagentContext")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        if let Some(message) = render_compact_source_access_template(
            decision,
            "sourceAccessCompactSubagentMessage",
            &reason,
            recovery_ref,
        ) {
            return message;
        }
        return format!(
            "ASP denied source access (`{reason}`) inside asp-explore. Use ASP query/search routes and return compact `[asp-search-subagent]` evidence.\nrecoveryRef={recovery_ref}"
        );
    }

    if let Some(message) = render_compact_source_access_template(
        decision,
        "sourceAccessCompactMessage",
        &reason,
        recovery_ref,
    ) {
        return message;
    }
    format!(
        "ASP denied source access (`{reason}`). Use asp-explore for ASP search/query; start and register it once if no asp-explore session is registered.\nrecoveryRef={recovery_ref}"
    )
}

fn render_compact_source_access_template(
    decision: &HookDecision,
    field: &str,
    reason: &str,
    recovery_ref: &str,
) -> Option<String> {
    let template = decision.fields.get(field)?.as_str()?;
    let resident_child_name = decision
        .fields
        .get("residentChildName")
        .and_then(Value::as_str)
        .unwrap_or("asp-explore");
    Some(
        template
            .replace("{{reason}}", reason)
            .replace("{{recoveryRef}}", recovery_ref)
            .replace("{{residentChildName}}", resident_child_name)
            .trim()
            .to_string(),
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
        "Follow the previous recovery route instead of retrying raw source tools.".to_string(),
        String::new(),
        "## Stop".to_string(),
        "Do not retry `Read`, `cat`, `sed`, `rg`, or source-dump commands on the matched source. The hook has already denied this lane."
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
