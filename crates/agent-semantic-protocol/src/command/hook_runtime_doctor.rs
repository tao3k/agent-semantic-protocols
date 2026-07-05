use super::{
    codex_enforcement_report, codex_project_plugin_hooks_present, display_path,
    ensure_supported_client, flag_value, project_root_arg, protocol_binary_on_path,
};
use agent_semantic_hook::{
    DecisionKind, HOOK_PROTOCOL_ID, HookClassificationRequest, ROOT_BLOCK_BEGIN, ROOT_BLOCK_END,
    ReasonKind, RuntimeProviderHealthStatus, classify_hook_with_config,
    codex_user_trust_state_status, default_activation_path, default_claude_settings_path,
    default_client_config_path, load_client_config_for_project, load_or_sync_activation,
    runtime_profiles_for_runtime,
};
use std::{collections::BTreeMap, fs, path::PathBuf};

pub(super) fn run_doctor(args: &[String]) -> Result<(), String> {
    let client = flag_value(args, "--client").unwrap_or("codex");
    ensure_supported_client(client)?;
    let project_root = project_root_arg(args)?;
    let activation_path = flag_value(args, "--activation")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_activation_path(&project_root));
    let runtime = load_or_sync_activation(&activation_path, &project_root)?;
    let runtime_profiles = runtime_profiles_for_runtime(&project_root, &runtime);
    let config_path = if client == "claude" {
        default_claude_settings_path(&project_root.to_string_lossy())
    } else {
        project_root.join(".codex").join("config.toml")
    };
    let config = fs::read_to_string(&config_path).unwrap_or_default();
    let client_config_path = default_client_config_path(&project_root.to_string_lossy());
    let hook_config = if client_config_path.is_file() {
        Some(
            load_client_config_for_project(&client_config_path, &project_root).map_err(
                |error| {
                    format!(
                        "invalid client hook config {}: {error}",
                        display_path(&project_root, &client_config_path)
                    )
                },
            )?,
        )
    } else {
        None
    };
    let client_config_status = if hook_config.is_some() {
        "ok"
    } else {
        "missing"
    };
    let legacy_root_hook = if client == "claude" {
        config.contains("asp hook") && config.contains("--client claude")
    } else {
        config.contains(ROOT_BLOCK_BEGIN) && config.contains(ROOT_BLOCK_END)
    };
    let project_plugin_hook =
        client == "codex" && codex_project_plugin_hooks_present(&project_root);
    let root_hook = legacy_root_hook || project_plugin_hook;
    let hook_mode = hook_mode_label(client, legacy_root_hook, project_plugin_hook);
    let hook_binary_path = protocol_binary_on_path();
    let hook_binary = hook_binary_path.is_some();
    let hook_binary_path = hook_binary_path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "missing".to_string());
    let enforcement = if client == "codex" {
        Some(codex_enforcement_report(
            &project_root,
            root_hook,
            hook_binary,
        ))
    } else {
        None
    };
    let (classifier_probe, classifier_reason) = if client == "codex" {
        if let Some(hook_config) = hook_config.as_ref() {
            let probe_payload = serde_json::json!({
                "tool_name": "functions.exec_command",
                "tool_input": {
                    "cmd": "sed -n '1,120p' src/lib.rs"
                }
            });
            let decision = classify_hook_with_config(HookClassificationRequest {
                registry: &runtime,
                config: hook_config,
                platform: client,
                event: "PreToolUse",
                payload: &probe_payload,
            });
            (
                decision_kind_label(decision.decision),
                reason_kind_label(decision.reason_kind),
            )
        } else {
            ("unavailable", "client-config-missing")
        }
    } else {
        ("not-applicable", "non-codex-client")
    };
    let trust_status = if client == "codex" {
        codex_user_trust_state_status(&config_path).ok()
    } else {
        None
    };
    let plugin_only_hook = project_plugin_hook && !legacy_root_hook;
    let trust = trust_status.as_ref().is_some_and(|status| {
        if plugin_only_hook {
            status.project_trusted
        } else {
            status.trusted
        }
    });
    let project_trust = trust_status
        .as_ref()
        .is_some_and(|status| status.project_trusted);
    let hook_state_trust = if plugin_only_hook {
        project_trust
    } else {
        trust_status
            .as_ref()
            .is_some_and(|status| status.hook_state_trusted)
    };
    let trust_missing_count = trust_status
        .as_ref()
        .map(|status| {
            if plugin_only_hook {
                0
            } else {
                status.missing_events.len()
            }
        })
        .unwrap_or(0);
    let trust_stale_count = trust_status
        .as_ref()
        .map(|status| {
            if plugin_only_hook {
                0
            } else {
                status.stale_events.len()
            }
        })
        .unwrap_or(0);
    let trust_config = trust_status
        .as_ref()
        .map(|status| status.trust_config_path.display().to_string())
        .unwrap_or_else(|| "unavailable".to_string());
    println!(
        "[agent-doctor] status=ok client={client} providers={} activation={} activationRuntime=derived config={} clientConfig={} clientConfigStatus={} hook={} hookMode={} pluginHook={} trust={} projectTrust={} hookStateTrust={} trustMissing={} trustStale={} trustConfig={} binary={} binaryPath={} classifierProbe={} classifierReason={} enforcement={} enforcementProbe={} enforcementReason={} protocol={}",
        runtime.providers.len(),
        display_path(&project_root, &activation_path),
        config_path.is_file(),
        display_path(&project_root, &client_config_path),
        client_config_status,
        root_hook,
        hook_mode,
        project_plugin_hook,
        trust,
        project_trust,
        hook_state_trust,
        trust_missing_count,
        trust_stale_count,
        trust_config,
        hook_binary,
        hook_binary_path,
        classifier_probe,
        classifier_reason,
        enforcement
            .as_ref()
            .map(|report| report.status)
            .unwrap_or("not-applicable"),
        enforcement
            .as_ref()
            .map(|report| report.probe)
            .unwrap_or("not-applicable"),
        enforcement
            .as_ref()
            .map(|report| report.reason)
            .unwrap_or("non-codex-client"),
        HOOK_PROTOCOL_ID,
    );
    if let Some(report) = enforcement.as_ref()
        && let Some(detail) = report.detail.as_ref()
    {
        println!(
            "|enforcement status={} probe={} reason={} exitSuccess={} deny={} sentinel={} hookEvent={}",
            report.status,
            report.probe,
            report.reason,
            detail.status_success,
            detail.saw_deny,
            detail.saw_sentinel,
            detail.saw_hook_event,
        );
    }
    if client == "codex" && root_hook {
        println!(
            "|codex-app projectConfig={} hookMode={} pluginHook={} projectTrust={} hookStateTrust={} reloadHint=restart-open-codex-app-thread-after-install",
            display_path(&project_root, &config_path),
            hook_mode,
            project_plugin_hook,
            project_trust,
            hook_state_trust,
        );
    }
    if let Some(status) = trust_status.as_ref()
        && !status.project_trusted
    {
        println!("|trust project=untrusted reason=project-not-trusted");
    }
    if let Some(status) = trust_status.as_ref()
        && !plugin_only_hook
        && !status.missing_events.is_empty()
    {
        println!("|trust missing={}", status.missing_events.join(","));
    }
    if let Some(status) = trust_status.as_ref()
        && !plugin_only_hook
        && !status.stale_events.is_empty()
    {
        println!("|trust stale={}", status.stale_events.join(","));
    }
    let runtime_profile_by_provider_key = runtime_profiles
        .providers
        .iter()
        .map(|profile| {
            (
                (
                    &profile.manifest_id,
                    &profile.language_id,
                    &profile.provider_id,
                    &profile.binary,
                ),
                profile,
            )
        })
        .collect::<BTreeMap<_, _>>();
    for provider in &runtime.providers {
        let runtime_profile = runtime_profile_by_provider_key
            .get(&(
                &provider.manifest_id,
                &provider.language_id,
                &provider.provider_id,
                &provider.binary,
            ))
            .copied();
        let runtime_profile_status = runtime_profile
            .map(|profile| runtime_profile_status_label(profile.health.status))
            .unwrap_or("missing");
        let resolved_binary = runtime_profile
            .and_then(|profile| profile.resolved_binary.as_deref())
            .unwrap_or("missing");
        println!(
            "|provider language={} provider={} binary={} execution={} runtimeStatus={} resolvedBinary={} roots={} extensions={}",
            provider.language_id,
            provider.provider_id,
            provider.binary,
            provider.execution.as_str(),
            runtime_profile_status,
            resolved_binary,
            provider.source_roots.join(","),
            provider.source_extensions.join(","),
        );
    }
    Ok(())
}

fn hook_mode_label(
    client: &str,
    legacy_root_hook: bool,
    project_plugin_hook: bool,
) -> &'static str {
    if client != "codex" {
        return if legacy_root_hook {
            "client-config"
        } else {
            "missing"
        };
    }
    match (legacy_root_hook, project_plugin_hook) {
        (true, true) => "mixed",
        (true, false) => "project-config",
        (false, true) => "codex-plugin",
        (false, false) => "missing",
    }
}

fn decision_kind_label(kind: DecisionKind) -> &'static str {
    match kind {
        DecisionKind::Allow => "allow",
        DecisionKind::Block => "block",
        DecisionKind::Deny => "deny",
    }
}

fn reason_kind_label(kind: ReasonKind) -> &'static str {
    match kind {
        ReasonKind::None => "none",
        ReasonKind::DirectSourceRead => "direct-source-read",
        ReasonKind::BulkSourceDump => "bulk-source-dump",
        ReasonKind::RawBroadSearch => "raw-broad-search",
        ReasonKind::SourceDirectoryEnumeration => "source-directory-enumeration",
        ReasonKind::AgentSearchJson => "agent-search-json",
        ReasonKind::SemanticAstPatchRequired => "semantic-ast-patch-required",
        ReasonKind::ReadOnlySubagentWrite => "read-only-subagent-write",
        ReasonKind::SubagentReceiptRequired => "subagent-receipt-required",
    }
}

fn runtime_profile_status_label(status: RuntimeProviderHealthStatus) -> &'static str {
    match status {
        RuntimeProviderHealthStatus::Available => "available",
        RuntimeProviderHealthStatus::Missing => "missing",
        RuntimeProviderHealthStatus::Unexecutable => "unexecutable",
    }
}
