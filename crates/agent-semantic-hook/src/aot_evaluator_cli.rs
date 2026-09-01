use std::io::Read as _;
use std::path::PathBuf;

use crate::{aot_evaluator, reader_probe};

/// Runs the Hook evaluator embedded in the canonical ASP binary.
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
    if std::env::args_os().any(|argument| argument == "--identity") {
        emit_runtime_identity();
        return;
    }
    let mut invocation = std::env::args_os().skip(1);
    let first = invocation
        .next()
        .and_then(|argument| argument.into_string().ok());
    let event = first;
    match event.as_deref() {
        Some("pre-tool") => {}
        Some("permission") | Some("permission-request") => {
            emit_permission_request_terminal();
            return;
        }
        // Observational Host events must still return one valid JSON object.
        // Producing no stdout makes Codex report an invalid PostToolUse Hook
        // response even though no policy decision is required for the event.
        Some(
            event @ ("post-tool" | "stop" | "notification" | "user-prompt" | "session-start"
            | "subagent-start" | "subagent-stop"),
        ) => {
            emit_observational_event(event);
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
                "hook-policy-bundle-unavailable",
                "hook-policy-bundle-unavailable",
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

fn emit_runtime_identity() {
    let result = crate::aot_compiler::compile_embedded_hook_policy_bundle()
        .and_then(|bundle| {
            serde_json::from_slice::<serde_json::Value>(&bundle)
                .map_err(|error| format!("decode embedded Hook policy identity: {error}"))
        })
        .and_then(|bundle| {
            bundle
                .get("generationDigest")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| "embedded Hook policy identity is missing its digest".to_owned())
        });
    match result {
        Ok(policy_content_digest) => println!(
            "{}",
            serde_json::json!({
                "schemaId": "agent.semantic-protocols.hook-runtime-identity",
                "schemaVersion": 1,
                "policyContentDigest": policy_content_digest,
            })
        ),
        Err(error) => println!(
            "{}",
            serde_json::json!({
                "schemaId": "agent.semantic-protocols.hook-runtime-identity",
                "schemaVersion": 1,
                "state": "failed",
                "reasonKind": "embedded-hook-policy-identity-invalid",
                "message": error,
            })
        ),
    }
}

fn emit_observational_event(event: &str) {
    let _ = event;
    println!("{{}}");
}

/// PermissionRequest is the Host approval plane after PreToolUse policy.
///
/// This event carries no authenticated PreTool admission receipt, so it cannot
/// safely re-evaluate or strengthen ASP policy. A policy denial terminates in
/// PreToolUse; every invocation that reaches PermissionRequest is returned to
/// the Host approval flow unchanged.
fn emit_permission_request_terminal() {
    println!("{}", crate::render_codex_permission_request("allow", None));
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
    let mut policy_bundle_path = None;
    let mut host_matcher = None;
    while let Some(argument) = args.next() {
        match argument.to_str() {
            Some("--policy-bundle") => policy_bundle_path = args.next().map(PathBuf::from),
            Some("--host-match") => {
                if host_matcher.is_some() {
                    return Err(EvaluationFailure::HostMatcherAuthority(
                        "plugin Host matcher requires exactly one --host-match".to_owned(),
                    ));
                }
                host_matcher = args.next().and_then(|value| value.into_string().ok());
            }
            _ => {}
        }
    }
    let host_matcher = host_matcher.ok_or_else(|| {
        EvaluationFailure::HostMatcherAuthority(
            "plugin Host matcher requires exactly one --host-match".to_owned(),
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
    let payload_json = serde_json::to_string(&payload).map_err(|error| {
        EvaluationFailure::GenerationAuthority(format!("encode enriched Hook payload: {error}"))
    })?;
    let evaluated = match policy_bundle_path {
        Some(path) => evaluate_payload_at_policy_bundle(&path, &payload_json, &host_matcher),
        None => evaluate_payload_from_embedded(&payload_json, &host_matcher),
    }
    .map_err(EvaluationFailure::GenerationAuthority)?;
    match evaluated {
        Some(typed) => println!("{typed}"),
        None => println!("{{}}"),
    }
    Ok(())
}

/// Evaluate one already-bounded Host payload against the immutable current
/// HookPolicyBundle. This is shared by the standalone evaluator and the direct
/// canonical Runtime Hook binary so there is only one policy engine.
pub fn evaluate_payload_from_embedded(
    payload_json: &str,
    host_matcher: &str,
) -> Result<Option<serde_json::Value>, String> {
    let payload: serde_json::Value = serde_json::from_str(payload_json)
        .map_err(|error| format!("decode Hook payload: {error}"))?;
    if command_local_no_agent_bypass(&payload, host_matcher) {
        return Ok(Some(serde_json::json!({})));
    }
    let payload_json = serde_json::to_string(&payload)
        .map_err(|error| format!("encode enriched Hook payload: {error}"))?;
    let policy_bundle = crate::aot_compiler::compile_embedded_hook_policy_bundle()?;
    let policy_bundle = std::str::from_utf8(&policy_bundle)
        .map_err(|error| format!("embedded Hook policy bundle is not UTF-8: {error}"))?;
    evaluate_payload_with_policy_bundle(policy_bundle, &payload_json, host_matcher)
}

fn command_local_no_agent_bypass(payload: &serde_json::Value, host_matcher: &str) -> bool {
    if host_matcher != "Bash" {
        return false;
    }
    payload_has_process_no_agent_assignment(payload)
}

/// Host payload evidence for the process-bound escape.  This is intentionally
/// narrower than textual matching: only an actual Bash command stage with the
/// exact environment assignment can activate it.
pub fn payload_has_process_no_agent_assignment(payload: &serde_json::Value) -> bool {
    if payload.get("tool_name").and_then(serde_json::Value::as_str) != Some("Bash") {
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

fn evaluate_payload_at_policy_bundle(
    policy_bundle_path: &std::path::Path,
    payload_json: &str,
    host_matcher: &str,
) -> Result<Option<serde_json::Value>, String> {
    let policy_bundle = load_policy_bundle(policy_bundle_path)?;
    evaluate_payload_with_policy_bundle(&policy_bundle, payload_json, host_matcher)
}

fn evaluate_payload_with_policy_bundle(
    policy_bundle: &str,
    payload_json: &str,
    host_matcher: &str,
) -> Result<Option<serde_json::Value>, String> {
    let mut payload_json = payload_json.to_owned();
    if let Some(decision) =
        aot_evaluator::evaluate_pre_tool(policy_bundle, &payload_json, host_matcher)?
    {
        if decision.decision == "allow" {
            return Ok(Some(serde_json::json!({})));
        }
        return render_and_record_deny(&decision, &payload_json).map(Some);
    }
    if let Some(request) =
        aot_evaluator::reader_probe_request(policy_bundle, &payload_json, host_matcher)?
    {
        let mut payload: serde_json::Value = serde_json::from_str(&payload_json)
            .map_err(|error| format!("decode Hook payload for Reader probe: {error}"))?;
        let observation = match std::env::var_os("ASP_STATE_HOME").filter(|path| !path.is_empty()) {
            Some(state_home) => reader_probe::diagnose_reader_probe_with_state_home(
                request.command_tokens,
                request.subject,
                request.wrapped_command,
                request.reader_behavior_patterns,
                std::path::Path::new(&state_home),
            ),
            None => reader_probe::diagnose_reader_probe(
                request.command_tokens,
                request.subject,
                request.wrapped_command,
                request.reader_behavior_patterns,
            ),
        };
        reader_probe::bind_reader_probe_observation(&mut payload, observation.as_ref())?;
        payload_json = serde_json::to_string(&payload)
            .map_err(|error| format!("encode Hook payload with Reader probe: {error}"))?;
    }
    if let Some(decision) =
        aot_evaluator::evaluate_pre_tool(policy_bundle, &payload_json, host_matcher)?
    {
        if decision.decision == "allow" {
            return Ok(Some(serde_json::json!({})));
        }
        return render_and_record_deny(&decision, &payload_json).map(Some);
    }
    Ok(Some(serde_json::json!({})))
}

fn render_and_record_deny(
    decision: &aot_evaluator::AotHookDecision<'_>,
    payload_json: &str,
) -> Result<serde_json::Value, String> {
    let payload: serde_json::Value = serde_json::from_str(payload_json)
        .map_err(|error| format!("decode denied Hook payload for event state: {error}"))?;
    if decision.route.is_some() {
        let project_root = payload
            .get("cwd")
            .and_then(serde_json::Value::as_str)
            .map(PathBuf::from)
            .or_else(|| std::env::current_dir().ok())
            .ok_or_else(|| "configured Agent deny has no workspace root".to_owned())?;
        crate::publish_aot_hook_session_route(&project_root, decision, &payload)?;
    }
    let typed = serde_json::to_value(decision).map_err(|error| error.to_string())?;
    let message = typed
        .get("message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("ASP Hook denied the operation.");
    Ok(crate::render_codex_pre_tool_deny(&typed, message))
}

fn load_policy_bundle(path: &std::path::Path) -> Result<String, String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("failed to inspect compiled Hook policy bundle: {error}"))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err("Hook policy bundle is not a regular immutable file".to_owned());
    }
    std::fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read Hook policy bundle {}: {error}",
            path.display()
        )
    })
}
