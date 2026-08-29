use std::io::Read as _;
use std::path::PathBuf;

use crate::{aot_evaluator, reader_probe};

/// Runs the standalone immutable `HookGeneration` evaluator entrypoint.
pub fn main_entry() {
    if inherited_no_agent_bypass() {
        // An empty object is a valid pass-through response for every Codex
        // command-hook output schema. Internal ASP receipt fields must never be
        // flattened into the Host wire envelope.
        println!("{{}}");
        return;
    }
    if std::env::args_os().any(|argument| argument == "--version") {
        println!("asp-hook schema=1");
        return;
    }
    let mut invocation = std::env::args_os().skip(1);
    let first = invocation
        .next()
        .and_then(|argument| argument.into_string().ok());
    let event = if first.as_deref() == Some("hook") {
        invocation
            .next()
            .and_then(|argument| argument.into_string().ok())
    } else {
        first
    };
    match event.as_deref() {
        Some("pre-tool") => {}
        Some("permission") | Some("permission-request") => {
            println!(
                "{}",
                crate::render_codex_permission_request(
                    "deny",
                    Some(
                        "ASP Hook denies the permission request until the command is admitted by policy."
                    ),
                )
            );
            return;
        }
        // Observational Host events must still return one valid JSON object.
        // Producing no stdout makes Codex report an invalid PostToolUse Hook
        // response even though no policy decision is required for the event.
        Some(
            "post-tool" | "stop" | "notification" | "user-prompt" | "session-start"
            | "subagent-start" | "subagent-stop",
        ) => {
            println!("{{}}");
            return;
        }
        _ => {
            println!("{{}}");
            return;
        }
    }
    if let Err(error) = evaluate() {
        let (reason_kind, terminal, message) = match error {
            EvaluationFailure::HostMatcherAuthority(message) => (
                "host-action-authority-unavailable",
                "host-matcher-authority-unavailable",
                message,
            ),
            EvaluationFailure::GenerationAuthority(message) => (
                "hook-generation-unavailable",
                "hook-generation-unavailable",
                message,
            ),
        };
        let typed = serde_json::json!({
            "schemaId": "agent.semantic-protocols.hook.execution-failure",
            "schemaVersion": 1,
            "decision": "deny",
            "state": "failed",
            "phase": "bootstrap",
            "reasonKind": reason_kind,
            "terminal": terminal,
            "processLaunched": false,
            "message": message,
        });
        let message = typed
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("ASP Hook denied the operation.");
        println!("{}", crate::render_codex_pre_tool_deny(&typed, message));
    }
}

fn inherited_no_agent_bypass() -> bool {
    std::env::var_os("ASP_NO_AGENT").is_some_and(|value| value == "1")
}

enum EvaluationFailure {
    HostMatcherAuthority(String),
    GenerationAuthority(String),
}

fn evaluate() -> Result<(), EvaluationFailure> {
    let mut args = std::env::args_os().skip(1);
    let mut generation_path = None;
    let mut host_matcher = None;
    while let Some(argument) = args.next() {
        match argument.to_str() {
            Some("--generation") => generation_path = args.next().map(PathBuf::from),
            Some("--host-match") => {
                if host_matcher.is_some() {
                    return Err(EvaluationFailure::HostMatcherAuthority(
                        "plugin Host matcher requires exactly one --host-match or --host-match-prefix"
                            .to_owned(),
                    ));
                }
                host_matcher = args.next().and_then(|value| value.into_string().ok());
            }
            Some("--host-match-prefix") => {
                if host_matcher.is_some() {
                    return Err(EvaluationFailure::HostMatcherAuthority(
                        "plugin Host matcher requires exactly one --host-match or --host-match-prefix"
                            .to_owned(),
                    ));
                }
                host_matcher = args.next().and_then(|value| value.into_string().ok());
            }
            _ => {}
        }
    }
    let host_matcher = host_matcher.ok_or_else(|| {
        EvaluationFailure::HostMatcherAuthority(
            "plugin Host matcher requires exactly one --host-match or --host-match-prefix"
                .to_owned(),
        )
    })?;
    let mut payload_json = String::new();
    std::io::stdin()
        .read_to_string(&mut payload_json)
        .map_err(|error| {
            EvaluationFailure::GenerationAuthority(format!("failed to read Hook payload: {error}"))
        })?;
    let payload: serde_json::Value = serde_json::from_str(&payload_json).map_err(|error| {
        EvaluationFailure::GenerationAuthority(format!("decode Hook payload: {error}"))
    })?;
    let tool_name = payload
        .get("tool_name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    if !aot_evaluator::host_matcher_matches_tool_name(&host_matcher, tool_name) {
        return Err(EvaluationFailure::HostMatcherAuthority(format!(
            "plugin Host matcher binding mismatch: matcher={host_matcher} toolName={tool_name}"
        )));
    }
    if command_local_no_agent_bypass(&payload, &host_matcher) {
        println!("{{}}");
        return Ok(());
    }
    let generation_path = match generation_path {
        Some(path) => path,
        None => match std::env::var_os("ASP_HOOK_GENERATION_ROOT") {
            Some(root) if !root.is_empty() => {
                PathBuf::from(root).join("compiled-hook-generation.json")
            }
            Some(_) => {
                return Err(EvaluationFailure::GenerationAuthority(
                    "ASP_HOOK_GENERATION_ROOT is set but empty".to_owned(),
                ));
            }
            None => implicit_generation_path().map_err(EvaluationFailure::GenerationAuthority)?,
        },
    };
    if let Some(typed) =
        evaluate_payload_at_generation(&generation_path, &payload_json, &host_matcher)
            .map_err(EvaluationFailure::GenerationAuthority)?
    {
        println!("{typed}");
    }
    Ok(())
}

/// Evaluate one already-bounded Host payload against the immutable current
/// HookGeneration. This is shared by the standalone evaluator and the direct
/// `asp hook pre-tool` compatibility surface so there is only one policy
/// engine and one publication format.
pub fn evaluate_payload_from_current(
    payload_json: &str,
    host_matcher: &str,
) -> Result<Option<serde_json::Value>, String> {
    let payload: serde_json::Value = serde_json::from_str(payload_json)
        .map_err(|error| format!("decode Hook payload: {error}"))?;
    if command_local_no_agent_bypass(&payload, host_matcher) {
        return Ok(Some(serde_json::json!({})));
    }
    let generation_path = implicit_generation_path()?;
    evaluate_payload_at_generation(&generation_path, payload_json, host_matcher)
}

fn command_local_no_agent_bypass(payload: &serde_json::Value, host_matcher: &str) -> bool {
    if host_matcher != "Bash"
        || payload.get("tool_name").and_then(serde_json::Value::as_str) != Some("Bash")
    {
        return false;
    }
    let Some(command) = payload
        .get("tool_input")
        .and_then(|input| input.get("command").or_else(|| input.get("cmd")))
        .and_then(serde_json::Value::as_str)
    else {
        return false;
    };
    agent_semantic_shell_parser::parse_bash_command_candidates(command).is_ok_and(|stages| {
        agent_semantic_shell_parser::command_stages_match_process_environment_assignment(
            &stages,
            &["ASP_NO_AGENT=1"],
        )
    })
}

fn evaluate_payload_at_generation(
    generation_path: &std::path::Path,
    payload_json: &str,
    host_matcher: &str,
) -> Result<Option<serde_json::Value>, String> {
    let mut generation = load_generation(&generation_path)?;
    if let Some(generation_digest) = active_generation_digest(&generation_path)? {
        let mut projection: serde_json::Value = serde_json::from_str(&generation)
            .map_err(|error| format!("decode HookGeneration identity projection: {error}"))?;
        projection["generationDigest"] = serde_json::Value::String(generation_digest);
        generation = serde_json::to_string(&projection)
            .map_err(|error| format!("encode HookGeneration identity projection: {error}"))?;
    }
    let mut payload_json = payload_json.to_owned();
    if let Some(decision) =
        aot_evaluator::evaluate_pre_tool(&generation, &payload_json, host_matcher)?
    {
        if decision.decision == "allow" {
            return Ok(Some(serde_json::json!({})));
        }
        let typed = serde_json::to_value(decision).map_err(|error| error.to_string())?;
        let message = typed
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("ASP Hook denied the operation.");
        return Ok(Some(crate::render_codex_pre_tool_deny(&typed, message)));
    }
    if let Some(request) =
        aot_evaluator::reader_probe_request(&generation, &payload_json, &host_matcher)?
    {
        let mut payload: serde_json::Value = serde_json::from_str(&payload_json)
            .map_err(|error| format!("decode Hook payload for Reader probe: {error}"))?;
        let observation = match std::env::var_os("ASP_STATE_HOME").filter(|path| !path.is_empty()) {
            Some(state_home) => reader_probe::diagnose_reader_probe_with_state_home(
                request.command_tokens,
                request.subject,
                request.reader_behavior_patterns,
                std::path::Path::new(&state_home),
            ),
            None => reader_probe::diagnose_reader_probe(
                request.command_tokens,
                request.subject,
                request.reader_behavior_patterns,
            ),
        };
        reader_probe::bind_reader_probe_observation(&mut payload, observation.as_ref())?;
        payload_json = serde_json::to_string(&payload)
            .map_err(|error| format!("encode Hook payload with Reader probe: {error}"))?;
    }
    if let Some(decision) =
        aot_evaluator::evaluate_pre_tool(&generation, &payload_json, &host_matcher)?
    {
        if decision.decision == "allow" {
            return Ok(Some(serde_json::json!({})));
        }
        let typed = serde_json::to_value(decision).map_err(|error| error.to_string())?;
        let message = typed
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("ASP Hook denied the operation.");
        return Ok(Some(crate::render_codex_pre_tool_deny(&typed, message)));
    }
    Ok(Some(serde_json::json!({})))
}

fn active_generation_digest(generation_path: &std::path::Path) -> Result<Option<String>, String> {
    if let Some(digest) = std::env::var_os("ASP_HOOK_GENERATION_DIGEST") {
        let digest = digest
            .into_string()
            .map_err(|_| "ASP_HOOK_GENERATION_DIGEST is not UTF-8".to_owned())?;
        if digest.is_empty() {
            return Err("ASP_HOOK_GENERATION_DIGEST is set but empty".to_owned());
        }
        return Ok(Some(digest));
    }
    let Some(name) = generation_path
        .parent()
        .and_then(std::path::Path::file_name)
        .and_then(std::ffi::OsStr::to_str)
    else {
        return Ok(None);
    };
    Ok(
        (name.len() == 64 && name.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .then(|| format!("blake3-256:{name}")),
    )
}

fn implicit_generation_path() -> Result<PathBuf, String> {
    let state_home = match std::env::var_os("ASP_STATE_HOME") {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        Some(_) => return Err("ASP_STATE_HOME is set but empty".to_owned()),
        None => {
            let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
            if home.is_empty() {
                return Err("HOME is set but empty".to_owned());
            }
            PathBuf::from(home).join(".agent-semantic-protocols")
        }
    };
    let current = state_home.join("hooks/current");
    if current.exists() {
        let generation_root = std::fs::canonicalize(&current).map_err(|error| {
            format!(
                "resolve current HookGeneration {}: {error}",
                current.display()
            )
        })?;
        return Ok(generation_root.join("compiled-hook-generation.json"));
    }
    let launcher_executable = std::env::current_exe()
        .map_err(|error| format!("resolve Hook evaluator executable: {error}"))?;
    let immutable_evaluator = std::fs::canonicalize(&launcher_executable).map_err(|error| {
        format!(
            "resolve Hook evaluator immutable candidate {}: {error}",
            launcher_executable.display()
        )
    })?;
    let metadata = std::fs::symlink_metadata(&immutable_evaluator).map_err(|error| {
        format!(
            "inspect Hook evaluator immutable candidate {}: {error}",
            immutable_evaluator.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "Hook evaluator immutable candidate {} is not a regular non-symlink file",
            immutable_evaluator.display()
        ));
    }
    let parent = immutable_evaluator.parent().ok_or_else(|| {
        format!(
            "Hook evaluator immutable candidate {} has no parent directory",
            immutable_evaluator.display()
        )
    })?;
    Ok(parent.join("compiled-hook-generation.json"))
}

fn load_generation(path: &std::path::Path) -> Result<String, String> {
    let current = path
        .parent()
        .ok_or("HookGeneration path has no current directory")?;
    let current_metadata = std::fs::symlink_metadata(current)
        .map_err(|error| format!("failed to inspect HookGeneration current directory: {error}"))?;
    let generation_metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("failed to inspect compiled HookGeneration: {error}"))?;
    if !current_metadata.file_type().is_dir()
        || current_metadata.file_type().is_symlink()
        || !generation_metadata.file_type().is_file()
        || generation_metadata.file_type().is_symlink()
    {
        return Err("active HookGeneration is not a regular immutable publication".to_owned());
    }
    std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read HookGeneration {}: {error}", path.display()))
}
