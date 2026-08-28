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
    #[serde(borrow)]
    pub rules: Vec<CompiledDecisionRule<'a>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledDecisionRule<'a> {
    #[serde(borrow)]
    pub id: &'a str,
    #[serde(borrow)]
    pub matchers: Vec<&'a str>,
    #[serde(default)]
    pub actions: Vec<&'a str>,
    #[serde(default, borrow)]
    pub registered_extensions: Vec<&'a str>,
    #[serde(borrow)]
    pub decision: &'a str,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<&'a str>,
    pub message: &'a str,
    pub context: &'a str,
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
    #[serde(default)]
    command: Option<&'a str>,
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
    subject: &'a str,
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
            && probe.access_mode == "O_RDONLY"
            && probe.terminal == "open-entry-observed"
            && probe.cleanup_verified
            && registered_source_operand(probe.subject, registered_extensions)
        {
            return Ok(Some(ConfirmedRead {
                subject: probe.subject,
                evidence: "reader-probe-open-read-only",
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
    let Some(command) = input.command else {
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
                subject,
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
    let Some(command) = input.command else {
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
        if confirmed_read_subject(payload.tool_input.get(), &rule.registered_extensions)?.is_some() {
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
        if !rule.matchers.contains(&host_matcher) {
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
        let subject = confirmed_read.as_ref().map(|read| read.subject);
        let read_evidence = confirmed_read.as_ref().map(|read| read.evidence);
        return Ok(Some(AotHookDecision {
            schema_id: "agent.semantic-protocols.hook.decision",
            schema_version: 1,
            decision: rule.decision,
            permission_decision: rule.decision,
            reason_kind: rule.reason_kind,
            config_rule_id: rule.id,
            generation_digest: generation.generation_digest,
            evidence: read_evidence.unwrap_or("host-matcher"),
            profile: rule.profile,
            language: rule.language,
            route: rule.route,
            subject,
            session_id: payload.session_id,
            tool_use_id: payload.tool_use_id,
            message: rule.message,
            context: rule.message,
            access: if read_evidence.is_some() { "read" } else { "unknown" },
            access_mode: if read_evidence.is_some() {
                "read-only"
            } else {
                "unknown"
            },
            backend: confirmed_read
                .as_ref()
                .map_or("not-attempted", |read| read.backend),
            terminal: confirmed_read
                .as_ref()
                .map_or("host-matcher-decision", |read| read.terminal),
            elapsed_micros: confirmed_read.as_ref().map_or(0, |read| read.elapsed_micros),
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

fn host_matcher_matches_tool_name(host_matcher: &str, tool_name: &str) -> bool {
    host_matcher == tool_name
        || (tool_name == "apply_patch" && matches!(host_matcher, "apply_patch" | "Edit" | "Write"))
        || (tool_name == "spawn_agent" && matches!(host_matcher, "spawn_agent" | "Agent"))
}

#[cfg(test)]
#[path = "../tests/unit/aot_evaluator.rs"]
mod tests;
