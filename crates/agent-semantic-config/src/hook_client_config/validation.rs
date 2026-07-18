//! Validation rules for hook client config files.

use std::collections::HashSet;

use super::model::{
    CLIENT_HOOK_CONFIG_SCHEMA_ID, CLIENT_HOOK_CONFIG_SCHEMA_VERSION, HOOK_PROTOCOL_ID,
    HOOK_PROTOCOL_VERSION, HookClientAgentOrgArtifactsArchiveWarningConfig,
    HookClientAgentOrgArtifactsConfig, HookClientAgentSessionGuideConfig,
    HookClientAspCommandIntentPolicyConfig, HookClientConfigFile, HookClientRecoveryPromptConfig,
    HookClientResidentAgentConfig, HookClientRuleConfig, HookClientRuleMatchConfig,
    HookClientRuleRouteConfig,
};

pub(super) fn validate_config(config: &HookClientConfigFile) -> Result<(), String> {
    validate_protocol(config)?;
    validate_optional_non_empty(
        "contractFingerprint",
        config.contract_fingerprint.as_deref(),
    )?;
    validate_agent_org_artifacts(config.agent_org_artifacts.as_ref())?;
    validate_recovery_prompt(&config.recovery_prompt)?;
    validate_agent_session_guide(&config.agent_session_guide)?;
    validate_agent_session_messages(&config.agent_session_messages)?;
    validate_resident_agents(&config.agents.resident_agents)?;
    validate_execution_lanes(&config.execution_lanes, &config.agents.resident_agents)?;
    validate_asp_command_intent_policy(&config.asp_command_intent_policy)?;
    validate_unique_rule_ids(&config.rules)?;
    validate_rule_schema_shape(&config.rules)
}

fn validate_execution_lanes(
    lanes: &super::model::HookClientExecutionLanesConfig,
    resident_agents: &[HookClientResidentAgentConfig],
) -> Result<(), String> {
    let mut command_prefix_owners = std::collections::BTreeMap::new();
    for (lane_name, lane) in &lanes.lanes {
        validate_non_empty("executionLanes lane name", lane_name)?;
        let prefix = format!("executionLanes.{lane_name}");
        validate_optional_non_empty(
            &format!("{prefix}.receiptKind"),
            Some(lane.receipt_kind.as_str()),
        )?;
        if lane.enabled && lane.command_prefixes.is_empty() {
            return Err(format!(
                "{prefix}.commandPrefixes must not be empty when enabled"
            ));
        }
        if lane.transport == super::model::HookClientExecutionTransport::ResidentAgent {
            validate_non_empty(&format!("{prefix}.residentName"), &lane.resident_name)?;
            if !resident_agents
                .iter()
                .any(|agent| agent.enabled && agent.name == lane.resident_name)
            {
                return Err(format!(
                    "{prefix}.residentName `{}` must name an enabled agents.residentAgents entry",
                    lane.resident_name
                ));
            }
        }
        validate_non_empty_values(
            &format!("{prefix}.commandPrefixes[]"),
            &lane.command_prefixes,
        )?;
        validate_unique_values(
            &format!("{prefix}.commandPrefixes[]"),
            &lane.command_prefixes,
        )?;
        if lane.enabled {
            for command_prefix in &lane.command_prefixes {
                let normalized = command_prefix
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                if let Some(previous_lane) =
                    command_prefix_owners.insert(normalized.clone(), lane_name.as_str())
                {
                    return Err(format!(
                        "executionLanes.{lane_name}.commandPrefixes[] `{normalized}` duplicates enabled lane `{previous_lane}`"
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validate_asp_command_intent_policy(
    policy: &HookClientAspCommandIntentPolicyConfig,
) -> Result<(), String> {
    for (label, values) in [
        (
            "aspCommandIntentPolicy.controlPlane.rootCommands[]",
            &policy.control_plane.root_commands,
        ),
        (
            "aspCommandIntentPolicy.reasoning.rootCommands[]",
            &policy.reasoning.root_commands,
        ),
        (
            "aspCommandIntentPolicy.reasoning.searchRoutes[]",
            &policy.reasoning.search_routes,
        ),
        (
            "aspCommandIntentPolicy.reasoning.queryFlags[]",
            &policy.reasoning.query_flags,
        ),
        (
            "aspCommandIntentPolicy.exactEvidence.queryProjectionFlags[]",
            &policy.exact_evidence.query_projection_flags,
        ),
        (
            "aspCommandIntentPolicy.exactEvidence.queryProjectionViews[]",
            &policy.exact_evidence.query_projection_views,
        ),
        (
            "aspCommandIntentPolicy.exactEvidence.selectorKinds[]",
            &policy.exact_evidence.selector_kinds,
        ),
        (
            "aspCommandIntentPolicy.directReadFallback.fromHookValues[]",
            &policy.direct_read_fallback.from_hook_values,
        ),
    ] {
        if values.iter().any(|value| value.trim().is_empty()) {
            return Err(format!("{label} must not contain empty values"));
        }
    }
    Ok(())
}

fn validate_recovery_prompt(config: &HookClientRecoveryPromptConfig) -> Result<(), String> {
    validate_optional_non_empty("recoveryPrompt.template", config.template.as_deref())?;
    validate_optional_non_empty(
        "recoveryPrompt.codexAgentFlow",
        config.codex_agent_flow.as_deref(),
    )?;
    validate_optional_non_empty(
        "recoveryPrompt.claudeAgentFlow",
        config.claude_agent_flow.as_deref(),
    )?;
    validate_optional_non_empty(
        "recoveryPrompt.defaultAgentFlow",
        config.default_agent_flow.as_deref(),
    )
}

fn validate_agent_session_guide(config: &HookClientAgentSessionGuideConfig) -> Result<(), String> {
    validate_optional_non_empty("agentSessionGuide.register", config.register.as_deref())?;
    validate_optional_non_empty("agentSessionGuide.list", config.list.as_deref())?;
    validate_optional_non_empty("agentSessionGuide.show", config.show.as_deref())?;
    validate_optional_non_empty("agentSessionGuide.reuse", config.reuse.as_deref())
}

fn validate_agent_session_messages(
    config: &super::model::HookClientAgentSessionMessagesConfig,
) -> Result<(), String> {
    validate_optional_non_empty(
        "agentSessionMessages.sourceAccessCompactSubagent",
        config.source_access_compact_subagent.as_deref(),
    )?;
    reject_legacy_flat_subagent_receipt_message(
        "agentSessionMessages.sourceAccessCompactSubagent",
        config.source_access_compact_subagent.as_deref(),
    )
}

fn reject_legacy_flat_subagent_receipt_message(
    field: &str,
    message: Option<&str>,
) -> Result<(), String> {
    let Some(message) = message else {
        return Ok(());
    };
    let normalized = message.to_ascii_lowercase();
    let mentions_legacy_owner_read_next = normalized.contains("owner/read/next");
    let mentions_legacy_selector_only_evidence = normalized.contains("selector-only")
        && normalized.contains("[asp-search-subagent]")
        && normalized.contains("evidence");
    if mentions_legacy_owner_read_next || mentions_legacy_selector_only_evidence {
        Err(format!(
            "{field} uses the legacy flat subagent receipt contract; refresh hooks/config.toml so ASP search children return schema/intent/route/state/evidence/next graph-route receipts"
        ))
    } else {
        Ok(())
    }
}

fn validate_resident_agents(configs: &[HookClientResidentAgentConfig]) -> Result<(), String> {
    for config in configs {
        validate_resident_agent(config)?;
    }
    Ok(())
}

fn validate_resident_agent(config: &HookClientResidentAgentConfig) -> Result<(), String> {
    validate_optional_non_empty("agents.residentAgents[].name", Some(config.name.as_str()))?;
    validate_optional_non_empty("agents.residentAgents[].role", Some(config.role.as_str()))?;
    if !config.codex_agent_name.is_empty() {
        validate_optional_non_empty(
            "agents.residentAgents[].codexAgentName",
            Some(config.codex_agent_name.as_str()),
        )?;
    }
    validate_optional_non_empty(
        "agents.residentAgents[].lifecycle",
        Some(config.lifecycle.as_str()),
    )?;
    validate_non_empty_values("agents.residentAgents[].roles[]", &config.roles)?;
    validate_unique_values("agents.residentAgents[].roles[]", &config.roles)?;
    for role in &config.roles {
        validate_binary_name("agents.residentAgents[].roles[]", role)?;
    }
    validate_non_empty_values("agents.residentAgents[].permissions[]", &config.permissions)?;
    validate_unique_values("agents.residentAgents[].permissions[]", &config.permissions)?;
    for permission in &config.permissions {
        validate_session_permission("agents.residentAgents[].permissions[]", permission)?;
    }
    for prefix in &config.main_allowed_asp_command_prefixes {
        validate_optional_non_empty(
            "agents.residentAgents[].mainAllowedAspCommandPrefixes[]",
            Some(prefix.as_str()),
        )?;
    }
    for prefix in &config.command_prefixes {
        validate_optional_non_empty(
            "agents.residentAgents[].commandPrefixes[]",
            Some(prefix.as_str()),
        )?;
    }
    Ok(())
}

fn validate_protocol(config: &HookClientConfigFile) -> Result<(), String> {
    expect_optional_field(
        "schemaId",
        config.schema_id.as_deref(),
        CLIENT_HOOK_CONFIG_SCHEMA_ID,
    )?;
    expect_optional_field(
        "schemaVersion",
        config.schema_version.as_deref(),
        CLIENT_HOOK_CONFIG_SCHEMA_VERSION,
    )?;
    expect_optional_field(
        "protocolId",
        config.protocol_id.as_deref(),
        HOOK_PROTOCOL_ID,
    )?;
    expect_optional_field(
        "protocolVersion",
        config.protocol_version.as_deref(),
        HOOK_PROTOCOL_VERSION,
    )?;
    Ok(())
}

fn validate_unique_rule_ids(rules: &[HookClientRuleConfig]) -> Result<(), String> {
    let mut seen = HashSet::new();
    for rule in rules {
        if !seen.insert(rule.id.as_str()) {
            return Err(format!("duplicate client hook rule id `{}`", rule.id));
        }
    }
    Ok(())
}

fn validate_rule_schema_shape(rules: &[HookClientRuleConfig]) -> Result<(), String> {
    for rule in rules {
        validate_identifier("rules[].id", &rule.id)?;
        validate_optional_non_empty("rules[].message", rule.message.as_deref())?;
        validate_optional_event(rule.event.as_deref())?;
        validate_optional_platform(rule.platform.as_deref())?;
        validate_unique_values("rules[].languageIds", &rule.language_ids)?;
        validate_identifiers("rules[].languageIds[]", &rule.language_ids)?;
        validate_match_schema_shape(&rule.match_config)?;
        if rule.decision_materializer.is_some() && !rule.routes.is_empty() {
            return Err(format!(
                "hook rule `{}` cannot combine decisionMaterializer with static routes",
                rule.id
            ));
        }
        for route in &rule.routes {
            validate_route_schema_shape(route)?;
        }
    }
    Ok(())
}

fn validate_match_schema_shape(match_config: &HookClientRuleMatchConfig) -> Result<(), String> {
    validate_optional_non_empty("rules[].match.tool", match_config.tool.as_deref())?;
    validate_non_empty_values("rules[].match.toolAny[]", &match_config.tool_any)?;
    validate_non_empty_values("rules[].match.commandAny[]", &match_config.command_any)?;
    validate_argv_prefix_patterns("rules[].match.argvPrefixAny", &match_config.argv_prefix_any)?;
    validate_non_empty_values(
        "rules[].match.commandContainsAny[]",
        &match_config.command_contains_any,
    )?;
    validate_non_empty_values("rules[].match.pathAny[]", &match_config.path_any)?;
    validate_non_empty_values("rules[].match.pathGlobAny[]", &match_config.path_glob_any)?;
    validate_non_empty_values(
        "rules[].match.argvSourceAny[]",
        &match_config.argv_source_any,
    )?;
    validate_non_empty_values(
        "rules[].match.argvSourceGlobAny[]",
        &match_config.argv_source_glob_any,
    )?;
    validate_non_empty_values(
        "rules[].match.argvSourceExcludeFlagAny[]",
        &match_config.argv_source_exclude_flag_any,
    )?;
    Ok(())
}

fn validate_route_schema_shape(route: &HookClientRuleRouteConfig) -> Result<(), String> {
    validate_identifier("rules[].routes[].providerId", &route.provider_id)?;
    if let Some(language_id) = &route.language_id {
        validate_identifier("rules[].routes[].languageId", language_id)?;
    }
    if let Some(binary) = &route.binary {
        validate_binary_name("rules[].routes[].binary", binary)?;
    }
    if route.argv.is_empty() {
        return Err("rules[].routes[].argv must contain at least one item".to_string());
    }
    Ok(())
}

fn validate_agent_org_artifacts(
    config: Option<&HookClientAgentOrgArtifactsConfig>,
) -> Result<(), String> {
    let Some(config) = config else {
        return Ok(());
    };
    if config.inactive_after_minutes == 0 {
        return Err("agentOrgArtifacts.inactiveAfterMinutes must be greater than 0".to_string());
    }
    validate_non_empty("agentOrgArtifacts.artifactsPath", &config.artifacts_path)?;
    validate_non_empty("agentOrgArtifacts.entrySkillPath", &config.entry_skill_path)?;
    validate_agent_org_artifacts_archive_warning(&config.archive_warning)?;
    Ok(())
}

fn validate_agent_org_artifacts_archive_warning(
    config: &HookClientAgentOrgArtifactsArchiveWarningConfig,
) -> Result<(), String> {
    if config.active_org_file_threshold == 0 {
        return Err(
            "agentOrgArtifacts.archiveWarning.activeOrgFileThreshold must be greater than 0"
                .to_string(),
        );
    }
    if config.max_reported_files == 0 {
        return Err(
            "agentOrgArtifacts.archiveWarning.maxReportedFiles must be greater than 0".to_string(),
        );
    }
    validate_non_empty(
        "agentOrgArtifacts.archiveWarning.archivesDir",
        &config.archives_dir,
    )?;
    Ok(())
}

fn validate_identifiers(field: &str, values: &[String]) -> Result<(), String> {
    for value in values {
        validate_identifier(field, value)?;
    }
    Ok(())
}

fn validate_identifier(field: &str, value: &str) -> Result<(), String> {
    let mut bytes = value.bytes();
    if !matches!(bytes.next(), Some(b'a'..=b'z')) {
        return Err(format!("invalid {field} `{value}`"));
    }
    if bytes.all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-')) {
        Ok(())
    } else {
        Err(format!("invalid {field} `{value}`"))
    }
}

fn validate_optional_non_empty(field: &str, value: Option<&str>) -> Result<(), String> {
    if matches!(value, Some("")) {
        Err(format!("{field} must not be empty"))
    } else {
        Ok(())
    }
}

fn validate_non_empty_values(field: &str, values: &[String]) -> Result<(), String> {
    for value in values {
        if value.is_empty() {
            return Err(format!("{field} must not be empty"));
        }
    }
    Ok(())
}

fn validate_argv_prefix_patterns(field: &str, patterns: &[Vec<String>]) -> Result<(), String> {
    for (index, pattern) in patterns.iter().enumerate() {
        if pattern.is_empty() {
            return Err(format!("{field}[{index}] must not be empty"));
        }
        validate_non_empty_values(&format!("{field}[{index}][]"), pattern)?;
    }
    Ok(())
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        Err(format!("{field} must not be empty"))
    } else {
        Ok(())
    }
}

fn validate_unique_values(field: &str, values: &[String]) -> Result<(), String> {
    let mut seen = HashSet::new();
    for value in values {
        if !seen.insert(value.as_str()) {
            return Err(format!("duplicate {field} `{value}`"));
        }
    }
    Ok(())
}

fn validate_optional_event(value: Option<&str>) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    match value {
        "pre-tool" | "permission-request" | "post-tool" | "user-prompt" | "session-start"
        | "subagent-start" | "subagent-stop" | "stop" => Ok(()),
        _ => Err(format!("unsupported event `{value}`")),
    }
}

fn validate_optional_platform(value: Option<&str>) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    match value {
        "codex" | "claude" | "unknown" => Ok(()),
        _ => Err(format!("unsupported platform `{value}`")),
    }
}

fn validate_binary_name(field: &str, value: &str) -> Result<(), String> {
    if !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
    {
        Ok(())
    } else {
        Err(format!("invalid {field} `{value}`"))
    }
}

fn validate_session_permission(field: &str, value: &str) -> Result<(), String> {
    match value {
        "read-only" | "workspace-write" | "danger-full-access" => Ok(()),
        _ => Err(format!(
            "invalid {field} `{value}`; expected one of read-only, workspace-write, danger-full-access"
        )),
    }
}

fn expect_optional_field(field: &str, actual: Option<&str>, expected: &str) -> Result<(), String> {
    if actual.is_some_and(|actual| actual != expected) {
        return Err(format!("expected {field}={expected}"));
    }
    Ok(())
}
