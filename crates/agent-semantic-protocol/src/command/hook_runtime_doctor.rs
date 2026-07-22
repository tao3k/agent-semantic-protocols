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
    let client_config_status = if client_config_path.is_file() {
        "ok"
    } else {
        "missing"
    };
    let hook_config =
        load_client_config_for_project(&client_config_path, &project_root).map_err(|error| {
            format!(
                "invalid effective client hook config {}: {error}",
                display_path(&project_root, &client_config_path)
            )
        })?;
    let binary_contract_fingerprint = agent_semantic_config::hook_client_contract_fingerprint();
    let configured_contract_fingerprint = hook_config.contract_fingerprint();
    let config_contract_status = match configured_contract_fingerprint {
        Some(configured) if configured == binary_contract_fingerprint => "match",
        Some(_) => "mismatch",
        None => "missing",
    };
    let legacy_root_hook = if client == "claude" {
        config.contains("asp hook") && config.contains("--client claude")
    } else {
        config.contains(ROOT_BLOCK_BEGIN) && config.contains(ROOT_BLOCK_END)
    };
    let project_plugin_hook =
        client == "codex" && codex_project_plugin_hooks_present(&project_root);
    let global_plugin_hook = client == "codex" && codex_global_plugin_hooks_present();
    let plugin_hook = project_plugin_hook || global_plugin_hook;
    let root_hook = legacy_root_hook || plugin_hook;
    let hook_mode = hook_mode_label(
        client,
        legacy_root_hook,
        project_plugin_hook,
        global_plugin_hook,
    );
    let hook_binary_path = protocol_binary_on_path();
    let active_contract_fingerprint = hook_binary_path.as_ref().and_then(|path| {
        let output = std::process::Command::new(path)
            .arg("--contract-fingerprint")
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let fingerprint = String::from_utf8(output.stdout).ok()?;
        let fingerprint = fingerprint.trim();
        (!fingerprint.is_empty()).then(|| fingerprint.to_string())
    });
    let binary_contract_status = match active_contract_fingerprint.as_deref() {
        Some(active) if active == binary_contract_fingerprint => "match",
        Some(_) => "mismatch",
        None => "unavailable",
    };
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
        let probe_payload = serde_json::json!({
            "tool_name": "functions.exec_command",
            "tool_input": {
                "cmd": "sed -n '1,120p' src/lib.rs"
            }
        });
        let decision = classify_hook_with_config(HookClassificationRequest {
            registry: &runtime,
            config: &hook_config,
            platform: client,
            event: "PreToolUse",
            payload: &probe_payload,
        });
        (
            decision_kind_label(decision.decision),
            reason_kind_label(decision.reason_kind),
        )
    } else {
        ("not-applicable", "non-codex-client")
    };
    let trust_status = if client == "codex" {
        codex_user_trust_state_status(&config_path).ok()
    } else {
        None
    };
    let plugin_trust_status = if client == "codex" && plugin_hook {
        let plugin_hooks_json_path = if global_plugin_hook {
            codex_global_plugin_hooks_json_path()
        } else {
            codex_project_plugin_hooks_json_path(&project_root)
        };
        plugin_hooks_json_path.ok().and_then(|path| {
            agent_semantic_hook::codex_user_plugin_trust_state_status(
                &path,
                &codex_plugin_hook_key_source(),
            )
            .ok()
        })
    } else {
        None
    };
    let plugin_only_hook = plugin_hook && !legacy_root_hook;
    let project_trust = trust_status
        .as_ref()
        .is_some_and(|status| status.project_trusted);
    let plugin_hook_state_trust = plugin_trust_status
        .as_ref()
        .is_some_and(|status| status.hook_state_trusted);
    let trust = trust_status.as_ref().is_some_and(|status| {
        if plugin_only_hook {
            status.project_trusted && plugin_hook_state_trust
        } else {
            status.trusted
        }
    });
    let hook_state_trust = if plugin_only_hook {
        plugin_hook_state_trust
    } else {
        trust_status
            .as_ref()
            .is_some_and(|status| status.hook_state_trusted)
    };
    let trust_missing_count = trust_status
        .as_ref()
        .map(|status| {
            if plugin_only_hook {
                plugin_trust_status
                    .as_ref()
                    .map(|status| status.missing_events.len())
                    .unwrap_or(0)
            } else {
                status.missing_events.len()
            }
        })
        .unwrap_or(0);
    let trust_stale_count = trust_status
        .as_ref()
        .map(|status| {
            if plugin_only_hook {
                plugin_trust_status
                    .as_ref()
                    .map(|status| status.stale_events.len())
                    .unwrap_or(0)
            } else {
                status.stale_events.len()
            }
        })
        .unwrap_or(0);
    let trust_config = if plugin_only_hook {
        plugin_trust_status
            .as_ref()
            .map(|status| status.trust_config_path.display().to_string())
    } else {
        trust_status
            .as_ref()
            .map(|status| status.trust_config_path.display().to_string())
    }
    .unwrap_or_else(|| "unavailable".to_string());
    let enforcement_status = enforcement
        .as_ref()
        .map(|report| report.status)
        .unwrap_or("not-applicable");
    let doctor_status = if config_contract_status != "match"
        || binary_contract_status != "match"
        || (client == "codex" && enforcement_status != "ok")
    {
        "warning"
    } else {
        "ok"
    };
    let background_thread_hook = if client == "codex" {
        "host-surface-unproven"
    } else {
        "not-applicable"
    };
    println!(
        "[agent-doctor] status={doctor_status} client={client} providers={} activation={} activationRuntime=derived config={} clientConfig={} clientConfigStatus={} configContractStatus={} configuredContractFingerprint={} hook={} hookMode={} pluginHook={} trust={} projectTrust={} hookStateTrust={} trustMissing={} trustStale={} trustConfig={} binary={} binaryPath={} binaryContractStatus={} binaryContractFingerprint={} activeContractFingerprint={} classifierProbe={} classifierReason={} enforcement={} enforcementProbe={} enforcementReason={} backgroundThreadHook={} protocol={}",
        runtime.providers.len(),
        display_path(&project_root, &activation_path),
        config_path.is_file(),
        display_path(&project_root, &client_config_path),
        client_config_status,
        config_contract_status,
        configured_contract_fingerprint.unwrap_or("missing"),
        root_hook,
        hook_mode,
        plugin_hook,
        trust,
        project_trust,
        hook_state_trust,
        trust_missing_count,
        trust_stale_count,
        trust_config,
        hook_binary,
        hook_binary_path,
        binary_contract_status,
        binary_contract_fingerprint,
        active_contract_fingerprint
            .as_deref()
            .unwrap_or("unavailable"),
        classifier_probe,
        classifier_reason,
        enforcement_status,
        enforcement
            .as_ref()
            .map(|report| report.probe)
            .unwrap_or("not-applicable"),
        enforcement
            .as_ref()
            .map(|report| report.reason)
            .unwrap_or("non-codex-client"),
        background_thread_hook,
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
            "|codex-app projectConfig={} hookMode={} pluginHook={} projectTrust={} hookStateTrust={} backgroundThreadHook={} hostSurface=codex_app.create_thread verificationHint=native-thread-required reloadHint=restart-native-codex-thread-after-plugin-install",
            display_path(&project_root, &config_path),
            hook_mode,
            plugin_hook,
            project_trust,
            hook_state_trust,
            background_thread_hook,
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
    if args.iter().any(|arg| arg == "--strict-contract")
        && (config_contract_status != "match" || binary_contract_status != "match")
    {
        return Err(format!(
            "hook contract freshness gate failed: config={config_contract_status} activeBinary={binary_contract_status}"
        ));
    }
    Ok(())
}

fn hook_mode_label(
    client: &str,
    legacy_root_hook: bool,
    project_plugin_hook: bool,
    global_plugin_hook: bool,
) -> &'static str {
    if client != "codex" {
        return if legacy_root_hook {
            "client-config"
        } else {
            "missing"
        };
    }
    match (legacy_root_hook, project_plugin_hook, global_plugin_hook) {
        (true, true, _) | (true, _, true) => "mixed",
        (true, false, false) => "project-config",
        (false, true, true) => "codex-plugin-project+global",
        (false, true, false) => "codex-plugin-project",
        (false, false, true) => "codex-plugin-global",
        (false, false, false) => "missing",
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
        ReasonKind::ActivationUnavailable => "activation-unavailable",
        ReasonKind::DirectSourceRead => "direct-source-read",
        ReasonKind::BulkSourceDump => "bulk-source-dump",
        ReasonKind::RawBroadSearch => "raw-broad-search",
        ReasonKind::AspReasoningRouted => "asp-reasoning-routed",
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
use super::hook_runtime_codex_plugin::codex_project_plugin_hooks_json_path;
use super::hook_runtime_codex_plugin_identity::{
    codex_global_plugin_hooks_json_path, codex_plugin_hook_key_source,
};
use crate::command::hook_runtime::hook_runtime_codex_plugin::codex_global_plugin_hooks_present;
