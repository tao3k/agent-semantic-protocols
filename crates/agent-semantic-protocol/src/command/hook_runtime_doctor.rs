use super::{
    codex_enforcement_report, codex_project_plugin_hooks_present, display_path,
    ensure_supported_client, flag_value, project_root_arg,
};
use agent_semantic_hook::{
    DecisionKind, HOOK_PROTOCOL_ID, HookClassificationRequest, ROOT_BLOCK_BEGIN, ROOT_BLOCK_END,
    ReasonKind, RuntimeProviderHealthStatus, classify_hook_with_config,
    codex_user_trust_state_status, default_activation_path, default_claude_settings_path,
    default_client_config_path, evaluate_match_policy_conformance, load_client_config_for_project,
    load_or_sync_activation, runtime_profiles_for_runtime,
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
    let hook_binary_probe = crate::command::protocol_binary::protocol_binary_path_probe();
    let hook_binary_path = hook_binary_probe.path.clone();
    let active_contract_fingerprint = hook_binary_path
        .as_ref()
        .and_then(|path| crate::command::protocol_binary_contract_fingerprint(path));
    let binary_contract_status = match active_contract_fingerprint.as_deref() {
        Some(active) if active == binary_contract_fingerprint => "match",
        Some(_) => "mismatch",
        None => "unavailable",
    };
    let hook_shell_probe = crate::command::protocol_binary_in_codex_hook_shell();
    let hook_shell_binary_status = match (
        hook_binary_path
            .as_ref()
            .and_then(|path| crate::command::protocol_binary_artifact_path_digest(path)),
        hook_shell_probe
            .path
            .as_ref()
            .and_then(|path| crate::command::protocol_binary_artifact_path_digest(path)),
    ) {
        (Some(active), Some(host)) if active == host => "match",
        (Some(_), Some(_)) => "mismatch",
        (_, None) if hook_shell_probe.status == "missing" => "missing",
        _ => "unavailable",
    };
    let hook_shell_binary_path = hook_shell_probe
        .path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "missing".to_string());
    let asp_path_status = match hook_shell_binary_status {
        "match" => "current",
        "mismatch" => "stale",
        "missing" => "missing",
        _ => "unavailable",
    };
    let asp_path = hook_shell_binary_path.clone();
    let hook_shell = hook_shell_probe.shell.display().to_string();
    let event_state_path = agent_semantic_runtime::project_state_paths(&project_root)?
        .hook_state_dir
        .join("events.jsonl");
    let (event_state_status, event_state_bytes, event_state_age_ms) =
        match fs::metadata(&event_state_path) {
            Ok(metadata) if metadata.len() == 0 => ("empty", 0, "unavailable".to_string()),
            Ok(metadata) => {
                let age_ms = metadata
                    .modified()
                    .ok()
                    .and_then(|modified| std::time::SystemTime::now().duration_since(modified).ok())
                    .map(|age| age.as_millis());
                let status = if age_ms.is_some_and(|age_ms| age_ms <= 300_000) {
                    "live"
                } else {
                    "present"
                };
                (
                    status,
                    metadata.len(),
                    age_ms
                        .map(|age_ms| age_ms.to_string())
                        .unwrap_or_else(|| "unavailable".to_string()),
                )
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                ("missing", 0, "unavailable".to_string())
            }
            Err(_) => ("unreadable", 0, "unavailable".to_string()),
        };
    let event_state_path = event_state_path.display().to_string();
    let hook_binary = hook_binary_path.is_some();
    let hook_binary_path = hook_binary_probe
        .candidate
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
    let match_policy_rule_count = hook_config.rule_count();
    let (classifier_probe, classifier_reason, classifier_rule_id) = if client == "codex" {
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
            decision
                .fields
                .get("configRuleId")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| "none".to_owned()),
        )
    } else {
        ("not-applicable", "non-codex-client", "none".to_owned())
    };
    let match_policy_report = (client == "codex")
        .then(|| evaluate_match_policy_conformance(&runtime, &hook_config, client));
    let match_policy_probe_count = match_policy_report
        .as_ref()
        .map_or(0, |report| report.case_count);
    let match_policy_covered_rule_count = match_policy_report
        .as_ref()
        .map_or(0, |report| report.covered_rule_ids.len());
    let match_policy_failure_count = match_policy_report
        .as_ref()
        .map_or(0, |report| report.failures.len());
    let match_policy_status = match match_policy_report.as_ref() {
        None => "not-applicable",
        Some(report) if report.is_complete() => "complete",
        Some(_) => "partial",
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
        || hook_binary_probe.status != "found"
        || (client == "codex" && root_hook && hook_shell_binary_status != "match")
        || (client == "codex"
            && root_hook
            && matches!(event_state_status, "missing" | "empty" | "unreadable"))
        || (client == "codex" && enforcement_status != "ok")
        || (client == "codex" && match_policy_status != "complete")
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
        "[agent-doctor] status={doctor_status} client={client} providers={} activation={} activationRuntime=derived config={} clientConfig={} clientConfigStatus={} configContractStatus={} configuredContractFingerprint={} hook={} hookMode={} pluginHook={} trust={} projectTrust={} hookStateTrust={} trustMissing={} trustStale={} trustConfig={} binary={} binaryPath={} binaryPathStatus={} binaryContractStatus={} binaryContractFingerprint={} activeContractFingerprint={} aspPathStatus={} aspPath={} hookShell={} hookShellMode=login hookShellBinaryStatus={} hookShellBinaryPath={} eventState={} eventStatePath={} eventStateBytes={} eventStateAgeMs={} classifierProbe={} classifierReason={} classifierRule={} matchPolicyStatus={} matchPolicyRules={} matchPolicyCases={} matchPolicyCovered={} matchPolicyFailures={} enforcement={} enforcementProbe={} enforcementReason={} backgroundThreadHook={} protocol={}",
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
        hook_binary_probe.status,
        binary_contract_status,
        binary_contract_fingerprint,
        active_contract_fingerprint
            .as_deref()
            .unwrap_or("unavailable"),
        asp_path_status,
        asp_path,
        hook_shell,
        hook_shell_binary_status,
        hook_shell_binary_path,
        event_state_status,
        event_state_path,
        event_state_bytes,
        event_state_age_ms,
        classifier_probe,
        classifier_reason,
        classifier_rule_id,
        match_policy_status,
        match_policy_rule_count,
        match_policy_probe_count,
        match_policy_covered_rule_count,
        match_policy_failure_count,
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
    if let Some(report) = match_policy_report.as_ref() {
        for failure in &report.failures {
            println!("|match-policy failure={failure}");
        }
    }
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
            provider.package_roots.join(","),
            provider.source_extensions.join(","),
        );
    }
    if args.iter().any(|arg| arg == "--strict-contract")
        && (config_contract_status != "match"
            || binary_contract_status != "match"
            || hook_binary_probe.status != "found"
            || (client == "codex" && match_policy_status != "complete")
            || (client == "codex" && root_hook && hook_shell_binary_status != "match"))
    {
        return Err(format!(
            "hook contract freshness gate failed: config={config_contract_status} activeBinary={binary_contract_status} binaryPath={} aspPath={asp_path_status} hookShellBinary={hook_shell_binary_status}",
            hook_binary_probe.status,
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
        ReasonKind::ProviderBinaryDirectExecution => "provider-binary-direct-execution",
        ReasonKind::None => "none",
        ReasonKind::ActivationUnavailable => "activation-unavailable",
        ReasonKind::DirectSourceRead => "direct-source-read",
        ReasonKind::StructuredSourceRead => "structured-source-read",
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
