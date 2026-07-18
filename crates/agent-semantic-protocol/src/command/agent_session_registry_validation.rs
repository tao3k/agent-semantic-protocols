use agent_semantic_runtime::{
    AgentSessionValidationReport as SessionValidationReport, CodexRolloutSessionMetadata,
    codex_rollout_session_metadata,
};

use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};

enum RolloutMetadataLookup {
    Historical,
    Registration,
}

impl RolloutMetadataLookup {
    fn missing_reason(&self, session_id: &str) -> String {
        match self {
            Self::Historical => {
                format!("Codex rollout metadata not found for child session `{session_id}`")
            }
            Self::Registration => {
                format!("Codex rollout metadata not found for child session `{session_id}`")
            }
        }
    }
}

struct ExpectedAgentProfile {
    config_path: PathBuf,
    role: String,
    model: String,
    reasoning_effort: Option<String>,
    sandbox: String,
}

#[derive(Default)]
struct CodexModelSwitchConfig {
    primary_model: Option<String>,
    agent_models: BTreeMap<String, String>,
}

pub(crate) fn validate_session_profile(
    session_id: &str,
    root_session_id: &str,
    name: &str,
    role: &str,
    reference_unix: i64,
) -> Result<SessionValidationReport, String> {
    validate_session_profile_with_rollout_lookup(
        session_id,
        root_session_id,
        name,
        role,
        reference_unix,
        RolloutMetadataLookup::Historical,
    )
}

pub(super) fn validate_recent_session_profile(
    session_id: &str,
    root_session_id: &str,
    name: &str,
    role: &str,
    _reference_unix: i64,
) -> Result<SessionValidationReport, String> {
    validate_session_profile_with_rollout_lookup(
        session_id,
        root_session_id,
        name,
        role,
        _reference_unix,
        RolloutMetadataLookup::Registration,
    )
}

fn validate_session_profile_with_rollout_lookup(
    session_id: &str,
    root_session_id: &str,
    name: &str,
    role: &str,
    _reference_unix: i64,
    rollout_lookup: RolloutMetadataLookup,
) -> Result<SessionValidationReport, String> {
    let Some(agent_kind) = validated_agent_kind(name, role) else {
        return Ok(SessionValidationReport {
            status: "skipped".to_string(),
            reason: "session role does not require Codex rollout profile validation".to_string(),
            config_path: None,
            rollout_path: None,
            expected_root_session_id: None,
            actual_root_session_id: None,
            expected_parent_thread_id: None,
            actual_parent_thread_id: None,
            expected_agent_path: None,
            actual_agent_path: None,
            expected_role: None,
            actual_role: None,
            expected_model: None,
            actual_model: None,
            expected_reasoning_effort: None,
            actual_reasoning_effort: None,
            expected_sandbox: None,
            actual_sandbox: None,
        });
    };
    let expected = match load_expected_agent_profile(agent_kind) {
        Ok(expected) => expected,
        Err(error) => {
            return Ok(SessionValidationReport {
                status: "failed".to_string(),
                reason: error,
                config_path: None,
                rollout_path: None,
                expected_root_session_id: Some(root_session_id.to_string()),
                actual_root_session_id: None,
                expected_parent_thread_id: Some(root_session_id.to_string()),
                actual_parent_thread_id: None,
                expected_agent_path: None,
                actual_agent_path: None,
                expected_role: Some(agent_kind.default_role().to_string()),
                actual_role: None,
                expected_model: None,
                actual_model: None,
                expected_reasoning_effort: None,
                actual_reasoning_effort: None,
                expected_sandbox: None,
                actual_sandbox: None,
            });
        }
    };
    let metadata = match rollout_lookup {
        RolloutMetadataLookup::Historical | RolloutMetadataLookup::Registration => {
            codex_rollout_session_metadata(session_id)?
        }
    };
    let Some(metadata) = metadata else {
        return Ok(SessionValidationReport {
            status: "failed".to_string(),
            reason: rollout_lookup.missing_reason(session_id),
            config_path: Some(expected.config_path.display().to_string()),
            rollout_path: None,
            expected_root_session_id: Some(root_session_id.to_string()),
            actual_root_session_id: None,
            expected_parent_thread_id: Some(root_session_id.to_string()),
            actual_parent_thread_id: None,
            expected_agent_path: Some(normalized_path_string(&expected.config_path)),
            actual_agent_path: None,
            expected_role: Some(expected.role),
            actual_role: None,
            expected_model: Some(expected.model),
            actual_model: None,
            expected_reasoning_effort: expected.reasoning_effort,
            actual_reasoning_effort: None,
            expected_sandbox: Some(expected.sandbox),
            actual_sandbox: None,
        });
    };
    let actual_model = metadata
        .model
        .clone()
        .or(metadata.collaboration_model.clone());
    let actual_reasoning_effort = metadata.reasoning_effort.clone();
    let expected_agent_path = normalized_path_string(&expected.config_path);
    let actual_agent_path = metadata
        .agent_path
        .as_deref()
        .map(|path| normalized_path_string(Path::new(path)));
    let mut failures = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut pass_reason = None;
    if metadata.thread_source.as_deref() != Some("subagent") {
        failures.push(format!(
            "threadSource expected subagent got {}",
            metadata.thread_source.as_deref().unwrap_or("<missing>")
        ));
    }
    if metadata.root_session_id.as_deref() != Some(root_session_id) {
        failures.push(format!(
            "rootSessionId expected {root_session_id} got {}",
            metadata.root_session_id.as_deref().unwrap_or("<missing>")
        ));
    }
    if metadata.parent_thread_id.as_deref() != Some(root_session_id) {
        failures.push(format!(
            "parentThreadId expected {root_session_id} got {}",
            metadata.parent_thread_id.as_deref().unwrap_or("<missing>")
        ));
    }
    if metadata.spawn_depth != Some(1) {
        failures.push(format!(
            "spawnDepth expected 1 for resident child got {}",
            metadata
                .spawn_depth
                .map(|depth| depth.to_string())
                .unwrap_or_else(|| "<missing>".to_string())
        ));
    }
    let canonical_agent_path_matches = actual_agent_path
        .as_deref()
        .is_some_and(|path| canonical_host_agent_identity_path_matches(path, &expected.role));
    let exact_role_matches = metadata.agent_role.as_deref().is_some_and(|role| {
        normalize_agent_identity(role) == normalize_agent_identity(&expected.role)
    });
    match metadata.agent_role.as_deref() {
        Some(_) if exact_role_matches => {}
        Some("default") if canonical_agent_path_matches => {
            let role_fallback_reason = format!(
                "agentRole default accepted only because canonical host agent path proves {}",
                expected.role,
            );
            pass_reason = Some(match pass_reason.take() {
                Some(existing) => format!("{existing}; {role_fallback_reason}"),
                None => role_fallback_reason,
            });
        }
        Some(actual_role) => {
            failures.push(format!(
                "agentRole expected {} got {}",
                expected.role, actual_role
            ));
        }
        None if canonical_agent_path_matches => {
            let role_fallback_reason = format!(
                "agentRole missing accepted only because parent-owned canonical host agent path proves {}",
                expected.role,
            );
            pass_reason = Some(match pass_reason.take() {
                Some(existing) => format!("{existing}; {role_fallback_reason}"),
                None => role_fallback_reason,
            });
        }
        None => {
            failures.push(format!(
                "agentRole expected {} got <missing>",
                expected.role
            ));
        }
    }
    match actual_agent_path.as_deref() {
        Some(actual_agent_path)
            if actual_agent_path == expected_agent_path || canonical_agent_path_matches => {}
        Some(actual_agent_path) => {
            failures.push(format!(
                "agentPath expected {} got {}",
                expected_agent_path, actual_agent_path
            ));
        }
        None if exact_role_matches => {
            let missing_agent_path_reason = format!(
                "agentPath missing in rollout; validating against expected config {} by role/model/root/parent",
                expected_agent_path
            );
            pass_reason = Some(match pass_reason.take() {
                Some(existing) => format!("{existing}; {missing_agent_path_reason}"),
                None => missing_agent_path_reason,
            });
        }
        None => failures.push(format!(
            "agentPath missing and agentRole does not prove expected managed profile {}",
            expected.role
        )),
    }
    match actual_model.as_deref() {
        Some(actual_model) if actual_model == expected.model => {}
        Some(actual_model) => {
            let runtime_observation = format!(
                "child-session runtime model drift observed: host observed {actual_model}, configured managed-child profile expects {}. identity remains adoptable for same-child repair; requiredAction=main-agent-followup-existing-child-with-natural-language-runtime-switch; runtimeEvidence=fresh-host-observation",
                expected.model
            );
            warnings.push(runtime_observation);
        }
        None => {
            let runtime_observation = format!(
                "child-session runtime model observation is missing; configured managed-child profile expects {}. identity remains adoptable for same-child repair; requiredAction=main-agent-followup-existing-child-with-natural-language-runtime-switch; runtimeEvidence=fresh-host-observation",
                expected.model
            );
            warnings.push(runtime_observation);
        }
    }
    if let Some(expected_reasoning_effort) = expected.reasoning_effort.as_deref() {
        match actual_reasoning_effort.as_deref() {
            Some(actual_reasoning_effort)
                if actual_reasoning_effort == expected_reasoning_effort => {}
            Some(actual_reasoning_effort) => {
                let runtime_observation = format!(
                    "child-session runtime reasoning drift observed: host observed {actual_reasoning_effort}, configured managed-child profile expects {expected_reasoning_effort}. identity remains adoptable for same-child repair; requiredAction=main-agent-followup-existing-child-with-natural-language-runtime-switch; runtimeEvidence=fresh-host-observation"
                );
                pass_reason = Some(match pass_reason.take() {
                    Some(existing) => format!("{existing}; {runtime_observation}"),
                    None => runtime_observation,
                });
            }
            None => {
                let runtime_observation = format!(
                    "child-session runtime reasoning observation is missing; configured managed-child profile expects {expected_reasoning_effort}. identity remains adoptable for same-child repair; requiredAction=main-agent-followup-existing-child-with-natural-language-runtime-switch; runtimeEvidence=fresh-host-observation"
                );
                pass_reason = Some(match pass_reason.take() {
                    Some(existing) => format!("{existing}; {runtime_observation}"),
                    None => runtime_observation,
                });
            }
        }
    }
    if let Some(reason) =
        sandbox_policy_mismatch_reason(&expected.sandbox, metadata.sandbox_policy.as_deref())
    {
        warnings.push(format!(
            "{reason}; sandboxVerificationStatus=host-inherited-drift-warning; sandboxPolicy=warning-only-host-inherited; sandboxAffectsReady=false"
        ));
    }
    Ok(SessionValidationReport {
        status: if !failures.is_empty() {
            "failed".to_string()
        } else if !warnings.is_empty() {
            "warning".to_string()
        } else {
            "passed".to_string()
        },
        reason: if !failures.is_empty() {
            failures.join("; ")
        } else if !warnings.is_empty() {
            warnings.join("; ")
        } else if let Some(pass_reason) = pass_reason {
            pass_reason
        } else {
            format!(
                "Codex rollout metadata matches {} profile",
                agent_kind.default_role()
            )
        },
        config_path: Some(expected.config_path.display().to_string()),
        rollout_path: Some(metadata.rollout_path.display().to_string()),
        expected_root_session_id: Some(root_session_id.to_string()),
        actual_root_session_id: metadata.root_session_id,
        expected_parent_thread_id: Some(root_session_id.to_string()),
        actual_parent_thread_id: metadata.parent_thread_id,
        expected_agent_path: Some(expected_agent_path),
        actual_agent_path,
        expected_role: Some(expected.role),
        actual_role: metadata.agent_role,
        expected_model: Some(expected.model),
        actual_model,
        expected_reasoning_effort: expected.reasoning_effort,
        actual_reasoning_effort,
        expected_sandbox: Some(expected.sandbox),
        actual_sandbox: metadata.sandbox_policy,
    })
}

#[derive(Clone, Copy)]
enum ValidatedAgentKind {
    AspExplore,
    AspTesting,
}

impl ValidatedAgentKind {
    fn config_file_name(self) -> &'static str {
        match self {
            Self::AspExplore => "asp-explorer.toml",
            Self::AspTesting => "asp-testing.toml",
        }
    }

    fn canonical_codex_config_file_name(self) -> &'static str {
        match self {
            Self::AspExplore => "asp-explorer_codex.toml",
            Self::AspTesting => "asp-testing_codex.toml",
        }
    }

    fn default_role(self) -> &'static str {
        match self {
            Self::AspExplore => "asp_explorer",
            Self::AspTesting => "asp_testing",
        }
    }

    fn agent_config_key(self) -> &'static str {
        match self {
            Self::AspExplore => "asp_explorer",
            Self::AspTesting => "asp_testing",
        }
    }
}

fn validated_agent_kind(name: &str, role: &str) -> Option<ValidatedAgentKind> {
    [name, role]
        .into_iter()
        .find_map(|value| match normalize_agent_identity(value).as_str() {
            "asp_explore" | "asp_explorer" => Some(ValidatedAgentKind::AspExplore),
            "asp_testing" => Some(ValidatedAgentKind::AspTesting),
            _ => None,
        })
}

pub(crate) fn rollout_metadata_matches_managed_agent_profile(
    name: &str,
    role: &str,
    metadata: &CodexRolloutSessionMetadata,
) -> bool {
    let Some(kind) = validated_agent_kind(name, role) else {
        return false;
    };
    let expected = load_expected_agent_profile(kind).ok();
    let expected_role = expected
        .as_ref()
        .map(|profile| profile.role.as_str())
        .unwrap_or_else(|| kind.default_role());
    let expected_identity = normalize_agent_identity(expected_role);
    let configured_role_matches = metadata.agent_role.as_deref().is_some_and(|agent_role| {
        let actual_identity = normalize_agent_identity(agent_role);
        actual_identity == expected_identity
            || actual_identity == normalize_agent_identity(kind.default_role())
    });
    let canonical_agent_path_matches = metadata
        .agent_path
        .as_deref()
        .is_some_and(|path| canonical_host_agent_identity_path_matches(path, expected_role));
    let direct_child = metadata.thread_source.as_deref() == Some("subagent")
        && metadata.spawn_depth == Some(1)
        && metadata.root_session_id.is_some()
        && metadata.root_session_id == metadata.parent_thread_id;
    direct_child && (configured_role_matches || canonical_agent_path_matches)
}

fn normalize_agent_identity(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn canonical_host_agent_identity_path_matches(path: &str, expected_role: &str) -> bool {
    path.rsplit(['/', '\\'])
        .find(|segment| !segment.is_empty())
        .is_some_and(|segment| {
            let profile_stem = segment.strip_suffix(".toml").unwrap_or(segment);
            let profile_identity = normalize_agent_identity(profile_stem);
            let profile_identity = profile_identity
                .strip_suffix("_codex")
                .unwrap_or(&profile_identity);
            profile_identity == normalize_agent_identity(expected_role)
        })
}

fn sandbox_policy_mismatch_reason(expected: &str, actual: Option<&str>) -> Option<String> {
    if actual == Some(expected) {
        return None;
    }
    Some(format!(
        "sandbox expected {} got {}",
        expected,
        actual.unwrap_or("<missing>")
    ))
}

fn load_expected_agent_profile(kind: ValidatedAgentKind) -> Result<ExpectedAgentProfile, String> {
    let host_config_path = codex_home().join("agents").join(kind.config_file_name());
    let canonical_config_path = asp_agent_canonical_config_path(kind)?;
    let config_path = if canonical_config_path.exists() {
        canonical_config_path
    } else {
        host_config_path
    };
    let text = fs::read_to_string(&config_path)
        .map_err(|error| format!("failed to read {}: {error}", config_path.display()))?;
    let value = toml::from_str::<toml::Value>(&text)
        .map_err(|error| format!("failed to parse {}: {error}", config_path.display()))?;
    let role = toml_string(&value, "name").unwrap_or_else(|| kind.default_role().to_string());
    let model = toml_string(&value, "model")
        .ok_or_else(|| format!("{} missing `model`", config_path.display()))?;
    let model_switch = load_codex_model_switch_config()?;
    let primary_model = model_switch
        .agent_model(kind)
        .cloned()
        .or(model_switch.primary_model)
        .unwrap_or(model);
    let sandbox = toml_string(&value, "sandbox_mode")
        .ok_or_else(|| format!("{} missing `sandbox_mode`", config_path.display()))?;
    Ok(ExpectedAgentProfile {
        config_path,
        role,
        model: primary_model,
        reasoning_effort: toml_string(&value, "model_reasoning_effort"),
        sandbox,
    })
}

pub(crate) fn expected_model_for_session_profile(
    name: &str,
    role: &str,
) -> Result<Option<String>, String> {
    let Some(agent_kind) = validated_agent_kind(name, role) else {
        return Ok(None);
    };
    Ok(Some(load_expected_agent_profile(agent_kind)?.model))
}

pub(crate) fn expected_reasoning_effort_for_session_profile(
    name: &str,
    role: &str,
) -> Result<Option<String>, String> {
    let Some(agent_kind) = validated_agent_kind(name, role) else {
        return Ok(None);
    };
    Ok(load_expected_agent_profile(agent_kind)?.reasoning_effort)
}

fn codex_model_switch_config(value: &toml::Value) -> CodexModelSwitchConfig {
    let models = value
        .get("platform")
        .and_then(|value| value.get("codex"))
        .and_then(|value| value.get("models"));
    CodexModelSwitchConfig {
        primary_model: models.and_then(|models| {
            toml_string(models, "primary").or_else(|| toml_string(models, "primaryModel"))
        }),
        agent_models: codex_agent_model_overrides(value),
    }
}

impl CodexModelSwitchConfig {
    fn agent_model(&self, kind: ValidatedAgentKind) -> Option<&String> {
        self.agent_models.get(kind.agent_config_key())
    }
}

fn codex_agent_model_overrides(value: &toml::Value) -> BTreeMap<String, String> {
    value
        .get("agents")
        .and_then(toml::Value::as_table)
        .map(|agents| {
            agents
                .iter()
                .filter_map(|(key, agent)| {
                    let model = toml_string(agent, "model")?;
                    Some((key.to_string(), model))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn asp_agent_canonical_config_path(kind: ValidatedAgentKind) -> Result<PathBuf, String> {
    Ok(agent_semantic_runtime::state_core::resolve_state_home()?
        .join("agents")
        .join(kind.canonical_codex_config_file_name()))
}

fn asp_agents_config_path() -> Result<PathBuf, String> {
    Ok(agent_semantic_runtime::state_core::resolve_state_home()?
        .join("agents")
        .join("config.toml"))
}

fn load_codex_model_switch_config() -> Result<CodexModelSwitchConfig, String> {
    let config_path = asp_agents_config_path()?;
    if !config_path.exists() {
        return Ok(CodexModelSwitchConfig::default());
    }
    let text = fs::read_to_string(&config_path)
        .map_err(|error| format!("failed to read {}: {error}", config_path.display()))?;
    let value = toml::from_str::<toml::Value>(&text)
        .map_err(|error| format!("failed to parse {}: {error}", config_path.display()))?;
    Ok(codex_model_switch_config(&value))
}

fn codex_home() -> PathBuf {
    env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex")))
        .unwrap_or_else(|| PathBuf::from(".codex"))
}

fn toml_string(value: &toml::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(toml::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

fn normalized_path_string(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string()
}
