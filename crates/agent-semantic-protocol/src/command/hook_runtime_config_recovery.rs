//! Hook configuration ownership, loading, and managed-profile self-repair.

use super::hook_runtime_agent_session::{
    AspSessionPolicy, load_asp_session_policy, load_asp_session_policy_overlay,
    load_embedded_asp_session_policy,
};
use agent_semantic_hook::{
    ClientHookConfig, HookDecision, load_client_config_for_project,
    load_client_config_overlay_for_project, load_embedded_client_config_for_project,
};
use std::path::Path;

pub(super) struct HookRuntimeConfigLoad {
    pub(super) hook_config: ClientHookConfig,
    pub(super) asp_session_policy: AspSessionPolicy,
    pub(super) repair_reasons: Vec<String>,
    pub(super) auto_refresh: Option<String>,
}

pub(super) fn load_hook_runtime_config(
    config_path: &Path,
    project_root: &Path,
) -> Result<HookRuntimeConfigLoad, String> {
    let declared_contract_fingerprint =
        agent_semantic_config::load_hook_client_config_declared_contract_fingerprint(config_path);
    let use_config_overlay = declared_contract_fingerprint
        .as_ref()
        .is_ok_and(|fingerprint| fingerprint.is_none());
    let mut hook_config_result = if use_config_overlay {
        load_client_config_overlay_for_project(config_path, project_root)
    } else {
        load_client_config_for_project(config_path, project_root)
    };
    let mut asp_session_policy_result = if use_config_overlay {
        load_asp_session_policy_overlay(config_path, project_root)
    } else {
        load_asp_session_policy(config_path, project_root)
    };
    let mut repair_reasons = unique_config_errors(&hook_config_result, &asp_session_policy_result);
    let expected_contract_fingerprint = agent_semantic_config::hook_client_contract_fingerprint();
    let matcher_contract_needs_refresh = declared_contract_fingerprint
        .as_ref()
        .ok()
        .and_then(Option::as_deref)
        .is_some_and(|configured| configured != expected_contract_fingerprint.as_str());
    if matcher_contract_needs_refresh {
        repair_reasons.push(format!(
            "hook matcher config fingerprint must equal {expected_contract_fingerprint}"
        ));
    }
    let needs_auto_refresh = hook_config_result.is_err()
        || asp_session_policy_result.is_err()
        || matcher_contract_needs_refresh;
    let auto_refresh = if needs_auto_refresh {
        Some(refresh_hook_runtime_config(
            config_path,
            project_root,
            &mut hook_config_result,
            &mut asp_session_policy_result,
        ))
    } else {
        None
    };
    let refresh_receipt = auto_refresh.as_deref().unwrap_or("not-required");
    let hook_config = hook_config_result.map_err(|error| {
        format!(
            "hook matcher config freshness gate failed for {}: {error}; automatic refresh receipt: {refresh_receipt}",
            config_path.display(),
        )
    })?;
    validate_strict_managed_fingerprint(
        &hook_config,
        use_config_overlay,
        &expected_contract_fingerprint,
        config_path,
        refresh_receipt,
    )?;
    let asp_session_policy = asp_session_policy_result.map_err(|error| {
        format!(
            "hook resident config freshness gate failed for {}: {error}; automatic refresh receipt: {refresh_receipt}",
            config_path.display(),
        )
    })?;
    Ok(HookRuntimeConfigLoad {
        hook_config,
        asp_session_policy,
        repair_reasons,
        auto_refresh,
    })
}

fn unique_config_errors(
    hook_config_result: &Result<ClientHookConfig, String>,
    asp_session_policy_result: &Result<AspSessionPolicy, String>,
) -> Vec<String> {
    let mut errors = hook_config_result
        .as_ref()
        .err()
        .cloned()
        .into_iter()
        .collect::<Vec<_>>();
    if let Some(error) = asp_session_policy_result.as_ref().err()
        && !errors.contains(error)
    {
        errors.push(error.clone());
    }
    errors
}

fn refresh_hook_runtime_config(
    config_path: &Path,
    project_root: &Path,
    hook_config_result: &mut Result<ClientHookConfig, String>,
    asp_session_policy_result: &mut Result<AspSessionPolicy, String>,
) -> String {
    match super::super::managed_hook_config::materialize(config_path) {
        Ok(status) => {
            *hook_config_result = load_client_config_for_project(config_path, project_root);
            *asp_session_policy_result = load_asp_session_policy(config_path, project_root);
            format!("completed:{}", status.as_str())
        }
        Err(error) => {
            *hook_config_result = load_embedded_client_config_for_project(project_root);
            *asp_session_policy_result = load_embedded_asp_session_policy(project_root);
            format!("embedded-current:persistence-failed:{error}")
        }
    }
}

fn validate_strict_managed_fingerprint(
    hook_config: &ClientHookConfig,
    use_config_overlay: bool,
    expected_contract_fingerprint: &str,
    config_path: &Path,
    refresh_receipt: &str,
) -> Result<(), String> {
    if use_config_overlay {
        return Ok(());
    }
    match hook_config.contract_fingerprint() {
        Some(configured) if configured == expected_contract_fingerprint => Ok(()),
        Some(configured) => Err(format!(
            "hook matcher config freshness gate failed for {}: configured fingerprint {configured} does not match binary fingerprint {expected_contract_fingerprint}; automatic refresh receipt: {refresh_receipt}",
            config_path.display()
        )),
        None => Err(format!(
            "hook matcher config freshness gate failed for {}: contract fingerprint is missing; automatic refresh receipt: {refresh_receipt}",
            config_path.display()
        )),
    }
}

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
