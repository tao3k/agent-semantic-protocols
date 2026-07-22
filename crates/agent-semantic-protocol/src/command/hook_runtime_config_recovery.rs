//! Hook configuration and managed-profile self-repair receipts.

use agent_semantic_hook::HookDecision;
use std::path::Path;

pub(super) fn annotate_hook_config_repair(
    decision: &mut HookDecision,
    config_path: &Path,
    repair_reasons: &[String],
    auto_refresh: &str,
) {
    let auto_refresh_completed = auto_refresh.starts_with("completed:");
    let embedded_current = auto_refresh.starts_with("embedded-current:");
    decision.fields.insert(
        "hookConfigStatus".to_string(),
        serde_json::Value::String(
            if auto_refresh_completed {
                "refreshed-by-hook"
            } else if embedded_current {
                "active-from-embedded-authority"
            } else {
                "verified-after-failed-refresh-attempt"
            }
            .to_string(),
        ),
    );
    decision.fields.insert(
        "hookConfigPath".to_string(),
        serde_json::Value::String(config_path.display().to_string()),
    );
    if !repair_reasons.is_empty() {
        decision.fields.insert(
            "hookConfigRepairReasons".to_string(),
            serde_json::Value::Array(
                repair_reasons
                    .iter()
                    .cloned()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }
    decision.fields.insert(
        "hookConfigFailurePolicy".to_string(),
        serde_json::Value::String("fail-closed".to_string()),
    );
    decision.fields.insert(
        "hookConfigAutoRefresh".to_string(),
        serde_json::Value::String(auto_refresh.to_string()),
    );
    decision.fields.insert(
        "hookConfigPersistenceStatus".to_string(),
        serde_json::Value::String(
            if auto_refresh_completed {
                "atomically-persisted"
            } else if embedded_current {
                "deferred-read-only-sandbox"
            } else {
                "refresh-not-confirmed"
            }
            .to_string(),
        ),
    );
    let diagnostic = if auto_refresh_completed {
        format!(
            "ASP hook atomically refreshed `{}` before continuing.",
            config_path.display()
        )
    } else if embedded_current {
        format!(
            "ASP hook activated the binary-owned current config in memory because `{}` is not writable in this sandbox; classification continued from embedded authority.",
            config_path.display()
        )
    } else {
        format!(
            "Hook refresh did not report completion, but `{}` passed the required matcher and resident contracts on reload. Automatic refresh receipt: {}",
            config_path.display(),
            auto_refresh
        )
    };
    if decision.message.trim().is_empty() {
        decision.message = diagnostic;
    } else {
        decision.message = format!("{}\n{diagnostic}", decision.message.trim());
    }
}
