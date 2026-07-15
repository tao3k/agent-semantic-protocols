//! Hook configuration and managed-profile self-repair receipts.

use agent_semantic_hook::{DecisionKind, HookDecision};
use std::path::Path;

pub(super) fn annotate_hook_config_fallback(
    decision: &mut HookDecision,
    config_path: &Path,
    errors: &[String],
    repair_reasons: &[String],
    auto_sync: Option<&str>,
) {
    let error = errors.join("; ");
    let auto_sync_completed = auto_sync.is_some_and(|status| status.starts_with("completed:"));
    decision.fields.insert(
        "hookConfigStatus".to_string(),
        serde_json::Value::String(
            if errors.is_empty() && auto_sync_completed {
                "repaired-by-asp-sync"
            } else {
                "degraded-built-in-fallback"
            }
            .to_string(),
        ),
    );
    decision.fields.insert(
        "hookConfigPath".to_string(),
        serde_json::Value::String(config_path.display().to_string()),
    );
    if !error.is_empty() {
        decision.fields.insert(
            "hookConfigError".to_string(),
            serde_json::Value::String(error.clone()),
        );
    }
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
        serde_json::Value::String("continue-with-built-in-policy".to_string()),
    );
    decision.fields.insert(
        "hookConfigRecoveryCommand".to_string(),
        serde_json::Value::String("asp sync".to_string()),
    );
    if let Some(auto_sync) = auto_sync {
        decision.fields.insert(
            "hookConfigAutoSync".to_string(),
            serde_json::Value::String(auto_sync.to_string()),
        );
    }
    let diagnostic = if errors.is_empty() && auto_sync_completed {
        format!(
            "ASP hook automatically ran `asp sync` and repaired `{}` before continuing.",
            config_path.display()
        )
    } else {
        format!(
            "Semantic hook config repair did not fully succeed; ASP continued with built-in policy so Codex remains operable. Automatic sync receipt: {}. Error: {}",
            auto_sync.unwrap_or("not-run"),
            if error.is_empty() { "none" } else { &error }
        )
    };
    if decision.decision == DecisionKind::Allow {
        decision.message = diagnostic;
    } else {
        decision.message = format!("{}\n{diagnostic}", decision.message.trim());
    }
}

pub(super) fn annotate_target_agent_auto_sync(
    decision: &mut HookDecision,
    target_agent_name: &str,
) {
    match super::super::sync::ensure_codex_agent_configuration(target_agent_name) {
        Ok(None) => {
            decision.fields.insert(
                "targetAgentRegistryStatus".to_string(),
                serde_json::Value::String("ready".to_string()),
            );
        }
        Ok(Some(sync)) => {
            decision.fields.insert(
                "targetAgentRegistryStatus".to_string(),
                serde_json::Value::String("repaired-by-asp-sync".to_string()),
            );
            decision.fields.insert(
                "targetAgentAutoSync".to_string(),
                serde_json::Value::String(format!(
                    "completed:hookConfig={};agentConfigs={};codexAgentRegistry={}",
                    sync.hook_config_status, sync.projected, sync.codex_registry_entries
                )),
            );
            decision.message = format!(
                "{}\nASP hook automatically ran `asp sync` and verified Codex profile `{target_agent_name}` before returning this route.",
                decision.message.trim()
            );
        }
        Err(error) => {
            decision.fields.insert(
                "targetAgentRegistryStatus".to_string(),
                serde_json::Value::String("degraded-built-in-route".to_string()),
            );
            decision.fields.insert(
                "targetAgentAutoSync".to_string(),
                serde_json::Value::String(format!("failed:{error}")),
            );
            decision.message = format!(
                "{}\nASP could not fully project Codex profile `{target_agent_name}`, but the config failure did not block Codex tool use. Auto-sync error: {error}",
                decision.message.trim()
            );
        }
    }
}
