use std::borrow::Cow;

use serde::{Deserialize, Serialize};

pub const HOOK_GENERATION_SCHEMA_ID: &str = "agent.semantic-protocols.hook-generation";
pub const HOOK_GENERATION_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledHookGeneration<'a> {
    #[serde(borrow)]
    pub schema_id: &'a str,
    pub schema_version: u32,
    #[serde(borrow)]
    pub generation_digest: &'a str,
    #[serde(default, borrow)]
    pub reader_behavior_patterns: Vec<Vec<&'a str>>,
    #[serde(default, borrow)]
    pub registered_languages: Vec<&'a str>,
    #[serde(borrow)]
    pub rules: Vec<CompiledDecisionRule<'a>>,
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
    #[serde(default)]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery_command: Option<String>,
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
        if probe.schema_id == "agent.semantic-protocols.reader-probe-observation"
            && probe.schema_version == 1
            && probe.access == "read"
            && probe.access_mode == "read-permission"
            && matches!(
                probe.terminal,
                "read-permission-observed"
                    | "reader-behavior-catalog-hit"
                    | "reader-behavior-cache-hit"
            )
            && probe.cleanup_verified
            && registered_source_operand(probe.subject, registered_extensions)
        {
            return Ok(Some(ConfirmedRead {
                subject: Cow::Borrowed(probe.subject),
                evidence: match probe.backend {
                    "hook-generation-reader-catalog" => "reader-behavior-static-catalog",
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
    let generation: CompiledHookGeneration<'_> = serde_json::from_str(generation_json)
        .map_err(|error| format!("decode compiled HookGeneration: {error}"))?;
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
    let mut executable_stages = stages
        .iter()
        .filter(|stage| !stage.is_separator() && stage.executable().is_some());
    let Some(stage) = executable_stages.next() else {
        return Ok(None);
    };
    if executable_stages.next().is_some() {
        return Ok(None);
    }
    for rule in generation.rules {
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
        let mut subjects = agent_semantic_shell_parser::command_stage_source_paths(stage)
            .into_iter()
            .filter(|path| registered_source_operand(path, &rule.registered_extensions));
        let Some(subject) = subjects.next() else {
            continue;
        };
        if subjects.next().is_some() {
            return Ok(None);
        }
        return Ok(Some(AotReaderProbeRequest {
            command_tokens: stage.words().to_vec(),
            subject,
            wrapped_command: rule.wrapped_command,
            reader_behavior_patterns: generation
                .reader_behavior_patterns
                .iter()
                .map(|pattern| pattern.iter().map(|token| (*token).to_owned()).collect())
                .collect(),
        }));
    }
    Ok(None)
}

pub fn evaluate_pre_tool<'a>(
    generation_json: &'a str,
    payload_json: &'a str,
    host_matcher: &'a str,
) -> Result<Option<AotHookDecision<'a>>, String> {
    let generation: CompiledHookGeneration<'a> =
        serde_json::from_str(generation_json).map_err(|error| error.to_string())?;
    if generation.schema_id != HOOK_GENERATION_SCHEMA_ID
        || generation.schema_version != HOOK_GENERATION_SCHEMA_VERSION
    {
        return Err("unsupported HookGeneration schema identity".to_owned());
    }
    let payload: BorrowedHookPayload<'a> =
        serde_json::from_str(payload_json).map_err(|error| error.to_string())?;
    if !host_matcher_matches_tool_name(host_matcher, payload.tool_name) {
        return Err(format!(
            "Host matcher {host_matcher:?} does not match payload tool_name {:?}",
            payload.tool_name
        ));
    }

    for rule in &generation.rules {
        if !rule
            .matchers
            .iter()
            .any(|matcher| configured_matcher_matches(matcher, host_matcher))
        {
            continue;
        }
        if !rule_conditions_match(rule, &generation, &payload)? {
            continue;
        }
        let confirmed_read = if host_matcher == "Bash"
            && rule.actions.iter().any(|action| *action == "read")
            && !rule.registered_extensions.is_empty()
        {
            confirmed_read_subject(payload.tool_input.get(), &rule.registered_extensions)?
        } else {
            None
        };
        if rule.actions.iter().any(|action| *action == "read") && confirmed_read.is_none() {
            continue;
        }
        let subject = confirmed_read.as_ref().map(|read| read.subject.clone());
        let read_evidence = confirmed_read.as_ref().map(|read| read.evidence);
        let message = rule.message.replace(
            "{{languageId}}",
            rule.language.unwrap_or("registered-language"),
        );
        let recovery_command = rule
            .language
            .zip(subject.as_deref())
            .map(|(language, subject)| {
                format!(
                    "asp {language} search owner {subject} items --workspace {} --view seeds",
                    payload.cwd.unwrap_or(".")
                )
            });
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
            recovery_command,
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
            backend: confirmed_read
                .as_ref()
                .map_or("not-attempted", |read| read.backend),
            terminal: confirmed_read
                .as_ref()
                .map_or("host-matcher-decision", |read| read.terminal),
            elapsed_micros: confirmed_read
                .as_ref()
                .map_or(0, |read| read.elapsed_micros),
            process_launched: confirmed_read
                .as_ref()
                .is_some_and(|read| read.process_launched),
            probe_process_launched: confirmed_read
                .as_ref()
                .is_some_and(|read| read.probe_process_launched),
            policy_fast_path: confirmed_read
                .as_ref()
                .is_none_or(|read| read.policy_fast_path),
            cleanup_verified: confirmed_read
                .as_ref()
                .is_none_or(|read| read.cleanup_verified),
            timeout: confirmed_read.as_ref().is_some_and(|read| read.timeout),
            reader_observation_micros: confirmed_read
                .as_ref()
                .map_or(0, |read| read.elapsed_micros),
        }));
    }
    Ok(None)
}

fn rule_conditions_match(
    rule: &CompiledDecisionRule<'_>,
    generation: &CompiledHookGeneration<'_>,
    payload: &BorrowedHookPayload<'_>,
) -> Result<bool, String> {
    let has_conditions = !rule.argv_prefix_any.is_empty()
        || !rule.command_contains_any.is_empty()
        || !rule.argv_token_all.is_empty()
        || !rule.argv_source_glob_any.is_empty()
        || !rule.command_any.is_empty()
        || !rule.process_environment_assignment_any.is_empty()
        || !rule.path_glob_any.is_empty()
        || rule.argv_workspace_regular_file
        || rule.argv_structured_document_file
        || rule.structured_projection.is_some();
    if !has_conditions {
        return Ok(true);
    }
    let input: BorrowedShellToolInput<'_> = serde_json::from_str(payload.tool_input.get())
        .map_err(|error| format!("decode Host tool input for AOT rule matching: {error}"))?;
    if payload.tool_name != "Bash" {
        return Ok(rule.path_glob_any.is_empty() || payload.tool_input.get().contains("*** "));
    }
    let Some(command) = input.command.as_deref() else {
        return Ok(false);
    };
    let stages = agent_semantic_shell_parser::parse_bash_command_candidates(command)
        .map_err(|error| format!("parse Bash command for AOT rule matching: {error}"))?;
    let words = stages
        .iter()
        .filter(|stage| !stage.is_separator())
        .flat_map(|stage| stage.words().iter())
        .map(String::as_str)
        .collect::<Vec<_>>();
    if !rule.process_environment_assignment_any.is_empty()
        && !agent_semantic_shell_parser::command_stages_match_process_environment_assignment(
            &stages,
            &rule.process_environment_assignment_any,
        )
    {
        return Ok(false);
    }
    if !rule.argv_prefix_any.is_empty()
        && !rule
            .argv_prefix_any
            .iter()
            .any(|pattern| match_argv_pattern(&stages, pattern, &generation.registered_languages))
    {
        return Ok(false);
    }
    if !rule.command_contains_any.is_empty()
        && !rule
            .command_contains_any
            .iter()
            .any(|needle| command.contains(needle))
    {
        return Ok(false);
    }
    if !rule.argv_token_all.is_empty()
        && !rule
            .argv_token_all
            .iter()
            .all(|required| words.iter().any(|word| word == required))
    {
        return Ok(false);
    }
    let source_paths = stages
        .iter()
        .filter(|stage| !stage.is_separator())
        .flat_map(agent_semantic_shell_parser::command_stage_source_paths)
        .collect::<Vec<_>>();
    if !rule.argv_source_glob_any.is_empty()
        && !source_paths.iter().any(|path| {
            rule.argv_source_glob_any
                .iter()
                .any(|pattern| simple_path_glob_matches(pattern, path))
        })
    {
        return Ok(false);
    }
    if !rule.path_glob_any.is_empty()
        && !source_paths.iter().any(|path| {
            rule.path_glob_any
                .iter()
                .any(|pattern| simple_path_glob_matches(pattern, path))
        })
    {
        return Ok(false);
    }
    if !rule.command_any.is_empty()
        && !stages
            .iter()
            .filter_map(|stage| stage.executable())
            .any(|executable| {
                let basename = executable.rsplit('/').next().unwrap_or(executable);
                rule.command_any.iter().any(|command| basename == *command)
            })
    {
        return Ok(false);
    }
    if rule.argv_structured_document_file
        && !source_paths
            .iter()
            .any(|path| has_any_extension(path, &["json", "toml"]))
    {
        return Ok(false);
    }
    if rule.argv_workspace_regular_file {
        let cwd = std::path::Path::new(payload.cwd.unwrap_or("."));
        if !source_paths.iter().any(|path| {
            let path = std::path::Path::new(path);
            let resolved = if path.is_absolute() {
                path.to_path_buf()
            } else {
                cwd.join(path)
            };
            resolved.is_file()
        }) {
            return Ok(false);
        }
    }
    if let Some(spec) = &rule.structured_projection {
        let classification =
            agent_semantic_shell_parser::structured::classify_single_bounded_path_command(
                command,
                agent_semantic_shell_parser::structured::BoundedPathCommandSpec {
                    binary: spec.binary,
                    optional_subcommand_any: &spec.optional_subcommand_any,
                    option_any: &spec.option_any,
                    option_value_arity: &spec.option_value_arity,
                    max_slice_items: spec.max_slice_items,
                },
            );
        if !matches!(
            classification,
            agent_semantic_shell_parser::structured::StructuredFilterClassification::BoundedPath { .. }
                | agent_semantic_shell_parser::structured::StructuredFilterClassification::BoundedScalarPredicate { .. }
        ) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn match_argv_pattern(
    stages: &[agent_semantic_shell_parser::CommandStage],
    pattern: &[&str],
    registered_languages: &[&str],
) -> bool {
    if let Some(index) = pattern
        .iter()
        .position(|token| *token == "<registered-language>")
    {
        return registered_languages.iter().any(|language| {
            let mut concrete = pattern
                .iter()
                .map(|token| (*token).to_owned())
                .collect::<Vec<_>>();
            concrete[index] = (*language).to_owned();
            agent_semantic_shell_parser::command_stages_match_wrapped_prefix(stages, &concrete)
                .routes_protected()
        });
    }
    let concrete = pattern
        .iter()
        .map(|token| (*token).to_owned())
        .collect::<Vec<_>>();
    agent_semantic_shell_parser::command_stages_match_wrapped_prefix(stages, &concrete)
        .routes_protected()
}

fn simple_path_glob_matches(pattern: &str, path: &str) -> bool {
    if pattern == "**" || pattern == "*" {
        return true;
    }
    pattern
        .rsplit_once("*.")
        .is_some_and(|(_, extension)| has_any_extension(path, &[extension]))
        || pattern == path
}

fn has_any_extension(path: &str, extensions: &[&str]) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, extension)| extensions.contains(&extension))
}

pub fn host_matcher_matches_tool_name(host_matcher: &str, tool_name: &str) -> bool {
    host_matcher == tool_name
        || (host_matcher.ends_with("__") && tool_name.starts_with(host_matcher))
        || (tool_name == "apply_patch" && matches!(host_matcher, "apply_patch" | "Edit" | "Write"))
        || (tool_name == "spawn_agent" && matches!(host_matcher, "spawn_agent" | "Agent"))
}

fn configured_matcher_matches(configured: &str, host_matcher: &str) -> bool {
    configured == host_matcher
        || configured
            .strip_prefix('^')
            .and_then(|matcher| matcher.strip_suffix('$'))
            == Some(host_matcher)
        || (configured == "^mcp__.*$" && host_matcher == "mcp__")
}

#[cfg(test)]
#[path = "../tests/unit/aot_evaluator.rs"]
mod tests;
