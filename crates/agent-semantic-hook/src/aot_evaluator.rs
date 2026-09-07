// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::borrow::Cow;

use serde::Deserialize;

#[path = "aot_evaluator_parts/matching.rs"]
mod matching;
pub use matching::host_matcher_matches_tool_name;
use matching::{configured_matcher_matches, normalized_agent_eq, rule_conditions_match};
use serde::Serialize;

pub const HOOK_POLICY_BUNDLE_SCHEMA_ID: &str = "agent.semantic-protocols.hook-policy-bundle";
pub const HOOK_POLICY_BUNDLE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledHookPolicyBundle<'a> {
    #[serde(borrow)]
    pub schema_id: &'a str,
    pub schema_version: u32,
    #[serde(borrow)]
    pub generation_digest: &'a str,
    #[serde(default, borrow)]
    pub command_action_patterns: Vec<CompiledCommandActionPattern<'a>>,
    #[serde(default, borrow)]
    pub registered_languages: Vec<&'a str>,
    #[serde(default = "default_agent_calling_pattern", borrow)]
    pub agent_calling_pattern: &'a str,
    #[serde(borrow)]
    pub rules: Vec<CompiledDecisionRule<'a>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledCommandActionPattern<'a> {
    #[serde(borrow)]
    pub action: &'a str,
    #[serde(borrow)]
    pub argv_pattern_any: Vec<Vec<&'a str>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledDecisionRule<'a> {
    #[serde(borrow)]
    pub id: &'a str,
    #[serde(default)]
    pub priority: i64,
    #[serde(borrow)]
    pub matchers: Vec<&'a str>,
    pub wrapped_command: bool,
    #[serde(default)]
    pub actions: Vec<&'a str>,
    #[serde(default, borrow)]
    pub registered_extensions: Vec<&'a str>,
    #[serde(borrow)]
    pub decision: &'a str,
    #[serde(default = "default_operation_intent")]
    pub intent: &'a str,
    #[serde(borrow)]
    pub reason_kind: &'a str,
    #[serde(borrow)]
    pub message: &'a str,
    #[serde(default, borrow)]
    pub profile: Option<&'a str>,
    #[serde(default, borrow)]
    pub language: Option<&'a str>,
    #[serde(default, borrow)]
    pub route: Option<&'a str>,
    #[serde(default, borrow)]
    pub argv_prefix_any: Vec<Vec<&'a str>>,
    #[serde(default, borrow)]
    pub command_contains_any: Vec<&'a str>,
    #[serde(default, borrow)]
    pub argv_token_all: Vec<&'a str>,
    #[serde(default, borrow)]
    pub argv_source_glob_any: Vec<&'a str>,
    #[serde(default, borrow)]
    pub command_any: Vec<&'a str>,
    #[serde(default, borrow)]
    pub process_environment_assignment_any: Vec<&'a str>,
    #[serde(default, borrow)]
    pub path_glob_any: Vec<&'a str>,
    #[serde(default)]
    pub argv_workspace_regular_file: bool,
    #[serde(default)]
    pub argv_structured_document_file: bool,
    #[serde(default, borrow)]
    pub structured_projection: Option<BorrowedStructuredProjection<'a>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BorrowedStructuredProjection<'a> {
    #[serde(borrow)]
    binary: &'a str,
    #[serde(default)]
    optional_subcommand_any: Vec<String>,
    #[serde(default)]
    option_any: Vec<String>,
    #[serde(default)]
    option_value_arity: std::collections::BTreeMap<String, u8>,
    #[serde(default = "default_max_slice_items")]
    max_slice_items: usize,
}

const fn default_max_slice_items() -> usize {
    64
}

const fn default_operation_intent() -> &'static str {
    "host-tool"
}

const fn default_agent_calling_pattern() -> &'static str {
    "@{name}"
}

#[derive(Debug, Deserialize)]
pub struct BorrowedHookPayload<'a> {
    #[serde(borrow)]
    pub tool_name: &'a str,
    #[serde(borrow)]
    pub tool_input: &'a serde_json::value::RawValue,
    #[serde(default, borrow)]
    pub session_id: Option<&'a str>,
    #[serde(default, borrow)]
    pub tool_use_id: Option<&'a str>,
    #[serde(default, borrow)]
    pub cwd: Option<&'a str>,
    /// Host-owned configured Agent identity. Child topology is not
    /// registration authority: a temporary SubAgent never becomes resident
    /// merely because it has a parent session.
    #[serde(
        default,
        alias = "agentRole",
        alias = "agent_type",
        alias = "agentType",
        borrow
    )]
    pub agent_role: Option<&'a str>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AotHookDecision<'a> {
    pub schema_id: &'static str,
    pub schema_version: u32,
    pub decision: &'a str,
    pub permission_decision: &'a str,
    pub reason_kind: &'a str,
    pub config_rule_id: &'a str,
    pub generation_digest: &'a str,
    pub evidence: &'static str,
    pub operation_intent: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<Cow<'a, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<&'a str>,
    pub message: String,
    pub context: String,
    pub access: &'static str,
    pub access_mode: &'static str,
    pub backend: &'a str,
    pub terminal: &'a str,
    pub elapsed_micros: u64,
    pub process_launched: bool,
    pub probe_process_launched: bool,
    pub policy_fast_path: bool,
    pub cleanup_verified: bool,
    pub timeout: bool,
    pub reader_observation_micros: u64,
}

#[derive(Deserialize)]
struct BorrowedShellToolInput<'a> {
    #[serde(default, borrow)]
    command: Option<Cow<'a, str>>,
    #[serde(default, rename = "_aspReaderProbe")]
    reader_probe: Option<BorrowedReaderProbe<'a>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BorrowedReaderProbe<'a> {
    schema_id: &'a str,
    schema_version: u32,
    subject: &'a str,
    access: &'a str,
    access_mode: &'a str,
    backend: &'a str,
    terminal: &'a str,
    elapsed_micros: u128,
    process_launched: bool,
    probe_process_launched: bool,
    timeout: bool,
    policy_fast_path: bool,
    cleanup_verified: bool,
}

struct ConfirmedRead<'a> {
    subject: Cow<'a, str>,
    evidence: &'static str,
    backend: &'a str,
    terminal: &'a str,
    elapsed_micros: u64,
    process_launched: bool,
    probe_process_launched: bool,
    timeout: bool,
    policy_fast_path: bool,
    cleanup_verified: bool,
}

fn confirmed_read_subject<'a>(
    tool_input: &'a str,
    registered_extensions: &[&str],
) -> Result<Option<ConfirmedRead<'a>>, String> {
    let input: BorrowedShellToolInput<'a> = serde_json::from_str(tool_input)
        .map_err(|error| format!("decode Bash tool input for Read classification: {error}"))?;
    if let Some(probe) = input.reader_probe {
        let valid_registered_probe = probe.schema_id
            == "agent.semantic-protocols.reader-probe-observation"
            && probe.schema_version == 1
            && registered_source_operand(probe.subject, registered_extensions);
        if valid_registered_probe
            && probe.access == "read"
            && probe.access_mode == "read-permission"
            && matches!(
                probe.terminal,
                "read-permission-observed"
                    | "reader-behavior-catalog-hit"
                    | "reader-behavior-cache-hit"
            )
            && probe.cleanup_verified
        {
            return Ok(Some(ConfirmedRead {
                subject: Cow::Borrowed(probe.subject),
                evidence: match probe.backend {
                    "hook-policy-bundle-reader-catalog" => "reader-behavior-static-catalog",
                    "state-home-reader-catalog" | "process-memory-reader-catalog" => {
                        "reader-behavior-dynamic-cache"
                    }
                    _ => "reader-probe-read-permission",
                },
                backend: probe.backend,
                terminal: probe.terminal,
                elapsed_micros: u64::try_from(probe.elapsed_micros).unwrap_or(u64::MAX),
                process_launched: probe.process_launched,
                probe_process_launched: probe.probe_process_launched,
                timeout: probe.timeout,
                policy_fast_path: probe.policy_fast_path,
                cleanup_verified: probe.cleanup_verified,
            }));
        }
        if valid_registered_probe
            && probe.access == "unknown"
            && reader_probe_indeterminate_requires_deny(probe.terminal)
        {
            return Ok(Some(ConfirmedRead {
                subject: Cow::Borrowed(probe.subject),
                evidence: "reader-probe-indeterminate-fail-closed",
                backend: probe.backend,
                terminal: probe.terminal,
                elapsed_micros: u64::try_from(probe.elapsed_micros).unwrap_or(u64::MAX),
                process_launched: probe.process_launched,
                probe_process_launched: probe.probe_process_launched,
                timeout: probe.timeout,
                policy_fast_path: probe.policy_fast_path,
                cleanup_verified: probe.cleanup_verified,
            }));
        }
        return Ok(None);
    }
    let Some(command) = input.command.as_deref() else {
        return Ok(None);
    };
    let mut tokens = command.split_whitespace();
    while let Some(token) = tokens.next() {
        let subject = if token == "<" {
            tokens.next()
        } else {
            token.strip_prefix('<').filter(|path| !path.is_empty())
        };
        if let Some(subject) = subject
            && registered_source_operand(subject, registered_extensions)
        {
            return Ok(Some(ConfirmedRead {
                subject: Cow::Owned(subject.to_owned()),
                evidence: "shell-redirection-read",
                backend: "shell-redirection",
                terminal: "policy-fast-path-confirmed-read",
                elapsed_micros: 0,
                process_launched: false,
                probe_process_launched: false,
                timeout: false,
                policy_fast_path: true,
                cleanup_verified: true,
            }));
        }
    }
    Ok(None)
}

fn reader_probe_indeterminate_requires_deny(terminal: &str) -> bool {
    matches!(
        terminal,
        "probe-timeout"
            | "probe-deferred"
            | "reader-behavior-cache-wait-timeout"
            | "cleanup-failed"
            | "readable-wait-failed"
            | "denied-wait-failed"
            | "permission-observation-incomplete"
    ) || terminal.starts_with("probe-candidates-exhausted:readable-wait-failed")
        || terminal.starts_with("probe-candidates-exhausted:denied-wait-failed")
        || terminal.starts_with("probe-candidates-exhausted:permission-observation-incomplete")
}

fn registered_source_operand(subject: &str, registered_extensions: &[&str]) -> bool {
    registered_extensions.iter().any(|extension| {
        subject
            .rsplit_once('.')
            .is_some_and(|(_, suffix)| suffix == *extension)
    })
}

pub struct AotReaderProbeRequest {
    pub command_tokens: Vec<String>,
    pub subject: String,
    pub wrapped_command: bool,
    pub reader_behavior_patterns: Vec<Vec<String>>,
}

pub fn reader_probe_request(
    generation_json: &str,
    payload_json: &str,
    host_matcher: &str,
) -> Result<Option<AotReaderProbeRequest>, String> {
    if host_matcher != "Bash" {
        return Ok(None);
    }
    let generation: CompiledHookPolicyBundle<'_> = serde_json::from_str(generation_json)
        .map_err(|error| format!("decode compiled HookPolicyBundle: {error}"))?;
    let payload: BorrowedHookPayload<'_> = serde_json::from_str(payload_json)
        .map_err(|error| format!("decode Hook payload: {error}"))?;
    if payload.tool_name != "Bash" {
        return Ok(None);
    }
    let input: BorrowedShellToolInput<'_> = serde_json::from_str(payload.tool_input.get())
        .map_err(|error| format!("decode Bash tool input for Reader probe admission: {error}"))?;
    if input.reader_probe.is_some() {
        return Ok(None);
    }
    let Some(command) = input.command.as_deref() else {
        return Ok(None);
    };
    let stages = match agent_semantic_shell_parser::parse_bash_command_candidates(command) {
        Ok(stages) => stages,
        Err(_) => return Ok(None),
    };
    // Search owns the entire Host call before Reader probing. A mixed pipeline
    // such as `rg ... | sed ...` is still a search operation; probing the
    // downstream formatter would both launch unnecessary work and manufacture
    // read evidence for a command that must be denied before execution.
    if configured_semantic_actions(&generation, Some(&stages)).contains(&"search") {
        return Ok(None);
    }
    for rule in &generation.rules {
        if !rule.matchers.iter().any(|matcher| *matcher == host_matcher)
            || !rule.actions.iter().any(|action| *action == "read")
            || rule.registered_extensions.is_empty()
        {
            continue;
        }
        if confirmed_read_subject(payload.tool_input.get(), &rule.registered_extensions)?.is_some()
        {
            return Ok(None);
        }
        for stage in stages
            .iter()
            .rev()
            .filter(|stage| !stage.is_separator() && stage.executable().is_some())
        {
            let mut subjects = agent_semantic_shell_parser::command_stage_source_paths(stage)
                .into_iter()
                .filter(|path| registered_source_operand(path, &rule.registered_extensions));
            let Some(subject) = subjects.next() else {
                continue;
            };
            // One probe observes one normalized executable/operand pair. A
            // stage with several registered operands is not guessed; another
            // stage in the same Host envelope may still carry one exact pair.
            if subjects.next().is_some() {
                continue;
            }
            return Ok(Some(AotReaderProbeRequest {
                command_tokens: stage.words().to_vec(),
                subject,
                wrapped_command: rule.wrapped_command,
                reader_behavior_patterns: reader_patterns(&generation),
            }));
        }
    }
    Ok(None)
}

fn reader_patterns(generation: &CompiledHookPolicyBundle<'_>) -> Vec<Vec<String>> {
    generation
        .command_action_patterns
        .iter()
        .filter(|family| family.action == "read")
        .flat_map(|family| family.argv_pattern_any.iter())
        .map(|pattern| pattern.iter().map(|token| (*token).to_owned()).collect())
        .collect()
}

fn configured_semantic_actions<'a>(
    generation: &'a CompiledHookPolicyBundle<'a>,
    stages: Option<&[agent_semantic_shell_parser::CommandStage]>,
) -> Vec<&'a str> {
    let mut actions = Vec::new();
    for family in &generation.command_action_patterns {
        if stages.is_some_and(|stages| {
            stages.iter().any(|stage| {
                family.argv_pattern_any.iter().any(|pattern| {
                    let pattern = pattern
                        .iter()
                        .map(|token| (*token).to_owned())
                        .collect::<Vec<_>>();
                    agent_semantic_shell_parser::command_tokens_match_argv_pattern(
                        stage.words(),
                        &pattern,
                        true,
                        "|",
                    )
                })
            })
        }) && !actions.contains(&family.action)
        {
            actions.push(family.action);
        }
    }
    actions
}

pub fn evaluate_pre_tool<'a>(
    generation_json: &'a str,
    payload_json: &'a str,
    host_matcher: &'a str,
) -> Result<Option<AotHookDecision<'a>>, String> {
    let generation: CompiledHookPolicyBundle<'a> =
        serde_json::from_str(generation_json).map_err(|error| error.to_string())?;
    if generation.schema_id != HOOK_POLICY_BUNDLE_SCHEMA_ID
        || generation.schema_version != HOOK_POLICY_BUNDLE_SCHEMA_VERSION
    {
        return Err("unsupported HookPolicyBundle schema identity".to_owned());
    }
    let payload: BorrowedHookPayload<'a> =
        serde_json::from_str(payload_json).map_err(|error| error.to_string())?;
    if !host_matcher_matches_tool_name(host_matcher, payload.tool_name) {
        return Err(format!(
            "Host matcher {host_matcher:?} does not match payload tool_name {:?}",
            payload.tool_name
        ));
    }
    let shell_input = if payload.tool_name == "Bash" {
        Some(
            serde_json::from_str::<BorrowedShellToolInput<'_>>(payload.tool_input.get()).map_err(
                |error| format!("decode Host tool input for AOT rule matching: {error}"),
            )?,
        )
    } else {
        None
    };
    let shell_command = shell_input
        .as_ref()
        .and_then(|input| input.command.as_deref());
    let shell_stages = shell_command
        .map(agent_semantic_shell_parser::parse_bash_command_candidates)
        .transpose()
        .map_err(|error| format!("parse Bash command for AOT rule matching: {error}"))?;
    let mut semantic_actions = configured_semantic_actions(&generation, shell_stages.as_deref());
    let mut registered_extensions = generation
        .rules
        .iter()
        .flat_map(|rule| rule.registered_extensions.iter().copied())
        .collect::<Vec<_>>();
    registered_extensions.sort_unstable();
    registered_extensions.dedup();
    let invocation_read = match host_matcher {
        "Bash" if !registered_extensions.is_empty() => {
            confirmed_read_subject(payload.tool_input.get(), &registered_extensions)?
        }
        _ => None,
    };
    // A dynamic Reader probe is a Host-observed filesystem permission, not a
    // configured command alias.  Promote a validated observation before the
    // action gate so the second evaluation can select the same read rule as a
    // native `Read` or catalog-known reader.  Without this, an unknown
    // interpreter-backed reader records `access=read` but is incorrectly
    // allowed because no static command pattern supplied the action.
    if invocation_read.is_some() && !semantic_actions.contains(&"read") {
        semantic_actions.push("read");
    }

    for rule in &generation.rules {
        if !rule
            .matchers
            .iter()
            .any(|matcher| configured_matcher_matches(matcher, host_matcher))
        {
            continue;
        }
        if !rule.actions.is_empty()
            && !rule
                .actions
                .iter()
                .any(|action| semantic_actions.contains(action))
        {
            continue;
        }
        if !rule_conditions_match(
            rule,
            &generation,
            &payload,
            shell_command,
            shell_stages.as_deref(),
        )? {
            continue;
        }
        let rule_confirmed_read = if rule.actions.iter().any(|action| *action == "read")
            && !rule.registered_extensions.is_empty()
        {
            match host_matcher {
                "Bash" => {
                    confirmed_read_subject(payload.tool_input.get(), &rule.registered_extensions)?
                }
                _ => None,
            }
        } else {
            None
        };
        if rule.actions.iter().any(|action| *action == "read") && rule_confirmed_read.is_none() {
            continue;
        }
        let confirmed_read = rule_confirmed_read.as_ref().or(invocation_read.as_ref());
        if rule.reason_kind == "agent-choice-required"
            && rule.route.is_some_and(|target| {
                payload
                    .agent_role
                    .is_some_and(|role| normalized_agent_eq(role, target))
            })
        {
            return Ok(None);
        }
        let subject = confirmed_read.map(|read| read.subject.clone());
        let read_evidence = confirmed_read.map(|read| read.evidence);
        let parent_task = rule.language.map_or_else(
            || {
                format!(
                    "Invoke Host tool `{}` exactly once with input {}",
                    payload.tool_name,
                    payload.tool_input.get()
                )
            },
            |language| {
                format!(
                    "Execute the registered {language} Search Playbook route for this read once"
                )
            },
        );
        let agent_dispatch_message = rule
            .route
            .map(|target| {
                crate::agent_dispatch_message::render_collaboration_instruction(
                    Some(target),
                    payload.session_id,
                    &parent_task,
                )
            })
            .unwrap_or_default();
        let message = rule
            .message
            .replace(
                "{{languageId}}",
                rule.language.unwrap_or("registered-language"),
            )
            .replace("{{agentDispatchMessage}}", &agent_dispatch_message);
        return Ok(Some(AotHookDecision {
            schema_id: "agent.semantic-protocols.hook.decision",
            schema_version: 1,
            decision: rule.decision,
            permission_decision: rule.decision,
            reason_kind: rule.reason_kind,
            config_rule_id: rule.id,
            generation_digest: generation.generation_digest,
            evidence: read_evidence.unwrap_or("host-matcher"),
            operation_intent: rule.intent,
            profile: rule.profile,
            language: rule.language,
            route: rule.route,
            subject,
            session_id: payload.session_id,
            tool_use_id: payload.tool_use_id,
            message: message.clone(),
            context: message,
            access: if read_evidence.is_some() {
                "read"
            } else {
                "unknown"
            },
            access_mode: if read_evidence.is_some() {
                "read-permission"
            } else {
                "unknown"
            },
            backend: confirmed_read.map_or("not-attempted", |read| read.backend),
            terminal: confirmed_read.map_or("host-matcher-decision", |read| read.terminal),
            elapsed_micros: confirmed_read.map_or(0, |read| read.elapsed_micros),
            process_launched: confirmed_read.is_some_and(|read| read.process_launched),
            probe_process_launched: confirmed_read.is_some_and(|read| read.probe_process_launched),
            policy_fast_path: confirmed_read.is_none_or(|read| read.policy_fast_path),
            cleanup_verified: confirmed_read.is_none_or(|read| read.cleanup_verified),
            timeout: confirmed_read.is_some_and(|read| read.timeout),
            reader_observation_micros: confirmed_read.map_or(0, |read| read.elapsed_micros),
        }));
    }
    Ok(None)
}

#[cfg(test)]
#[path = "../tests/unit/aot_evaluator.rs"]
mod tests;
