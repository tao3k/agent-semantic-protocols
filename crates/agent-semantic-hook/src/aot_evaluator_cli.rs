// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::io::Read as _;
use std::path::PathBuf;

use crate::aot_evaluator;
use crate::reader_probe;

/// Internal audit result kept separate from the Codex Host response envelope.
#[doc(hidden)]
pub struct AotHookEvaluationReceipt {
    pub host_output: Option<serde_json::Value>,
    pub reader_probe_event_path: Option<PathBuf>,
}

/// Runs the Hook evaluator embedded in the canonical ASP binary.
pub fn main_entry() {
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
        Some("subagent-stop") => {
            emit_subagent_stop_terminal();
            return;
        }
        Some(
            event @ ("post-tool" | "stop" | "notification" | "user-prompt" | "session-start"
            | "subagent-start"),
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
                "hook-serving-config-unavailable",
                "hook-serving-config-unavailable",
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
    let result = crate::hook_runtime_identity_receipt();
    match result {
        Ok(receipt) => println!(
            "{}",
            serde_json::to_string(&receipt).expect("Hook Runtime identity receipt is serializable")
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
    if event != "post-tool" {
        println!("{{}}");
        return;
    }
    #[cfg(feature = "compiler")]
    match append_post_tool_workspace_mutation() {
        Ok(()) => println!("{{}}"),
        Err(failure) => {
            let typed = serde_json::to_string(&failure)
                .unwrap_or_else(|_| "Hook memory inbox failed".to_owned());
            println!(
                "{}",
                serde_json::json!({
                    "hookSpecificOutput": {
                        "hookEventName": "PostToolUse",
                        "additionalContext": typed,
                    },
                    "systemMessage": typed,
                })
            );
        }
    }
    #[cfg(not(feature = "compiler"))]
    println!("{{}}");
}

#[cfg(feature = "compiler")]
fn append_post_tool_workspace_mutation()
-> Result<(), agent_semantic_client_protocol::HookMemoryInboxFailure> {
    use agent_semantic_client_protocol::{
        HookMemoryInboxFailure, HookMemoryInboxFailureReason, HookWorkspaceMutationEvent,
    };
    let fail = |reason, detail| HookMemoryInboxFailure::new(reason, detail);
    let mut payload_json = String::new();
    std::io::stdin()
        .read_to_string(&mut payload_json)
        .map_err(|error| {
            fail(
                HookMemoryInboxFailureReason::DecodeRecord,
                format!("read PostToolUse payload: {error}"),
            )
        })?;
    let payload: serde_json::Value = serde_json::from_str(&payload_json).map_err(|error| {
        fail(
            HookMemoryInboxFailureReason::DecodeRecord,
            format!("decode PostToolUse payload: {error}"),
        )
    })?;
    let tool_name = payload
        .get("tool_name")
        .or_else(|| payload.get("toolName"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let tool_input = payload
        .get("tool_input")
        .or_else(|| payload.get("toolInput"))
        .unwrap_or(&serde_json::Value::Null);
    let changed_paths = crate::workspace_mutation_paths(tool_name, tool_input);
    if changed_paths.is_empty() {
        return Ok(());
    }
    let project_root = payload
        .get("cwd")
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| {
            fail(
                HookMemoryInboxFailureReason::EncodeEvent,
                "PostToolUse workspace mutation has no project root".to_owned(),
            )
        })?;
    if !project_root.is_absolute() {
        return Err(fail(
            HookMemoryInboxFailureReason::EncodeEvent,
            "PostToolUse workspace root must be absolute".to_owned(),
        ));
    }
    let mut relative_paths = changed_paths
        .iter()
        .map(|path| normalize_changed_path(&project_root, path))
        .collect::<Result<Vec<_>, _>>()?;
    relative_paths.sort();
    relative_paths.dedup();
    let canonical_project_root = std::fs::canonicalize(&project_root).map_err(|error| {
        fail(
            HookMemoryInboxFailureReason::EncodeEvent,
            format!(
                "canonicalize PostToolUse workspace root {}: {error}",
                project_root.display()
            ),
        )
    })?;
    let mutation_id = payload
        .get("tool_use_id")
        .or_else(|| payload.get("toolUseId"))
        .and_then(serde_json::Value::as_str)
        .filter(|identity| !identity.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("hook:{}", blake3::hash(payload_json.as_bytes()).to_hex()));
    let event = HookWorkspaceMutationEvent {
        mutation_id,
        project_root: canonical_project_root.display().to_string(),
        changed_paths: relative_paths,
        tool_name: tool_name.to_owned(),
        session_id: payload
            .get("session_id")
            .or_else(|| payload.get("sessionId"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        tool_use_id: payload
            .get("tool_use_id")
            .or_else(|| payload.get("toolUseId"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
    };
    let path = crate::hook_memory_inbox::default_path()
        .map_err(|error| fail(HookMemoryInboxFailureReason::Open, error))?;
    crate::hook_memory_inbox::append_workspace_mutation(&path, event)?;
    Ok(())
}

#[cfg(feature = "compiler")]
fn normalize_changed_path(
    project_root: &std::path::Path,
    candidate: &str,
) -> Result<String, agent_semantic_client_protocol::HookMemoryInboxFailure> {
    use agent_semantic_client_protocol::{HookMemoryInboxFailure, HookMemoryInboxFailureReason};
    use std::path::Component;
    let candidate = std::path::Path::new(candidate);
    let relative = if candidate.is_absolute() {
        candidate.strip_prefix(project_root).map_err(|_| {
            HookMemoryInboxFailure::new(
                HookMemoryInboxFailureReason::EncodeEvent,
                format!("changed path escapes project root: {}", candidate.display()),
            )
        })?
    } else {
        candidate
    };
    let mut normalized = PathBuf::new();
    for component in relative.components() {
        match component {
            Component::Normal(segment) => normalized.push(segment),
            _ => {
                return Err(HookMemoryInboxFailure::new(
                    HookMemoryInboxFailureReason::EncodeEvent,
                    format!("changed path is not normalized: {}", candidate.display()),
                ));
            }
        }
    }
    normalized
        .to_str()
        .filter(|path| !path.is_empty())
        .map(|path| path.replace(std::path::MAIN_SEPARATOR, "/"))
        .ok_or_else(|| {
            HookMemoryInboxFailure::new(
                HookMemoryInboxFailureReason::EncodeEvent,
                "changed path is empty or not UTF-8",
            )
        })
}

fn emit_subagent_stop_terminal() {
    let mut payload_json = String::new();
    if std::io::stdin().read_to_string(&mut payload_json).is_err() {
        println!(
            "{}",
            crate::search_subagent_output_contract::evaluate_subagent_stop(&serde_json::json!({}))
        );
        return;
    }
    let terminal = serde_json::from_str::<serde_json::Value>(&payload_json)
        .map(|payload| crate::search_subagent_output_contract::evaluate_subagent_stop(&payload))
        .unwrap_or_else(|_| {
            crate::search_subagent_output_contract::evaluate_subagent_stop(&serde_json::json!({}))
        });
    println!("{terminal}");
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

enum EvaluationFailure {
    HostMatcherAuthority(String),
    GenerationAuthority(String),
}

fn evaluate() -> Result<(), EvaluationFailure> {
    let mut args = std::env::args_os().skip(1);
    let mut host_matcher = None;
    while let Some(argument) = args.next() {
        match argument.to_str() {
            Some("--policy-bundle") => {
                return Err(EvaluationFailure::GenerationAuthority(
                    "--policy-bundle is not a supported asp-hook production argument".to_owned(),
                ));
            }
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
    let payload_json = serde_json::to_string(&payload).map_err(|error| {
        EvaluationFailure::GenerationAuthority(format!("encode enriched Hook payload: {error}"))
    })?;
    let evaluated = evaluate_payload_from_serving_config(&payload_json, &host_matcher)
        .map_err(EvaluationFailure::GenerationAuthority)?;
    match evaluated {
        Some(typed) => println!("{typed}"),
        None => println!("{{}}"),
    }
    Ok(())
}

/// Evaluate one already-bounded Host payload against the current serving
/// Hook configuration. The system template is the base and an admitted State
/// Home config is its overlay; both compile to one immutable HookPolicyBundle
/// for this evaluation.
pub fn evaluate_payload_from_serving_config(
    payload_json: &str,
    host_matcher: &str,
) -> Result<Option<serde_json::Value>, String> {
    let payload: serde_json::Value = serde_json::from_str(payload_json)
        .map_err(|error| format!("decode Hook payload: {error}"))?;
    let payload_json = serde_json::to_string(&payload)
        .map_err(|error| format!("encode enriched Hook payload: {error}"))?;
    let policy_bundle = crate::aot_compiler::compile_serving_hook_policy_bundle()?;
    let policy_bundle = std::str::from_utf8(&policy_bundle)
        .map_err(|error| format!("serving Hook policy bundle is not UTF-8: {error}"))?;
    evaluate_payload_with_policy_bundle(policy_bundle, &payload_json, host_matcher)
}

#[doc(hidden)]
pub fn evaluate_payload_with_policy_bundle(
    policy_bundle: &str,
    payload_json: &str,
    host_matcher: &str,
) -> Result<Option<serde_json::Value>, String> {
    evaluate_payload_with_policy_bundle_and_state_home(
        policy_bundle,
        payload_json,
        host_matcher,
        std::env::var_os("ASP_STATE_HOME")
            .filter(|path| !path.is_empty())
            .as_deref()
            .map(std::path::Path::new),
    )
}

#[doc(hidden)]
pub fn evaluate_payload_with_policy_bundle_and_state_home(
    policy_bundle: &str,
    payload_json: &str,
    host_matcher: &str,
    state_home: Option<&std::path::Path>,
) -> Result<Option<serde_json::Value>, String> {
    evaluate_payload_with_policy_bundle_and_state_home_with_receipt(
        policy_bundle,
        payload_json,
        host_matcher,
        state_home,
    )
    .map(|receipt| receipt.host_output)
}

#[doc(hidden)]
pub fn evaluate_payload_with_policy_bundle_and_state_home_with_receipt(
    policy_bundle: &str,
    payload_json: &str,
    host_matcher: &str,
    state_home: Option<&std::path::Path>,
) -> Result<AotHookEvaluationReceipt, String> {
    let mut payload_json = payload_json.to_owned();
    if let Some(host_output) =
        crate::search_playbook_pretool::evaluate(&payload_json, host_matcher)?
    {
        return Ok(AotHookEvaluationReceipt {
            host_output: Some(host_output),
            reader_probe_event_path: None,
        });
    }
    if let Some(decision) =
        aot_evaluator::evaluate_pre_tool(policy_bundle, &payload_json, host_matcher)?
    {
        if decision.decision == "allow" {
            return Ok(AotHookEvaluationReceipt {
                host_output: Some(serde_json::json!({})),
                reader_probe_event_path: None,
            });
        }
        return render_and_record_deny(&decision, &payload_json).map(|host_output| {
            AotHookEvaluationReceipt {
                host_output: Some(host_output),
                reader_probe_event_path: None,
            }
        });
    }
    if let Some(request) =
        aot_evaluator::reader_probe_request(policy_bundle, &payload_json, host_matcher)?
    {
        let mut payload: serde_json::Value = serde_json::from_str(&payload_json)
            .map_err(|error| format!("decode Hook payload for Reader probe: {error}"))?;
        let observation = match state_home {
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
        let decision =
            aot_evaluator::evaluate_pre_tool(policy_bundle, &payload_json, host_matcher)?;
        let project_root = payload
            .get("cwd")
            .and_then(serde_json::Value::as_str)
            .map(PathBuf::from)
            .or_else(|| std::env::current_dir().ok())
            .ok_or_else(|| "Reader probe has no workspace root".to_owned())?;
        let reader_probe_event_path = if let Some(observation) = observation.as_ref() {
            Some(crate::append_reader_probe_event_state(
                &project_root,
                state_home,
                host_matcher,
                &payload,
                observation,
                decision.as_ref(),
            )?)
        } else {
            None
        };
        if let Some(decision) = decision {
            if decision.decision == "allow" {
                return Ok(AotHookEvaluationReceipt {
                    host_output: Some(serde_json::json!({})),
                    reader_probe_event_path,
                });
            }
            return render_and_record_deny(&decision, &payload_json).map(|host_output| {
                AotHookEvaluationReceipt {
                    host_output: Some(host_output),
                    reader_probe_event_path,
                }
            });
        }
        return Ok(AotHookEvaluationReceipt {
            host_output: Some(serde_json::json!({})),
            reader_probe_event_path,
        });
    }
    if let Some(decision) =
        aot_evaluator::evaluate_pre_tool(policy_bundle, &payload_json, host_matcher)?
    {
        if decision.decision == "allow" {
            return Ok(AotHookEvaluationReceipt {
                host_output: Some(serde_json::json!({})),
                reader_probe_event_path: None,
            });
        }
        return render_and_record_deny(&decision, &payload_json).map(|host_output| {
            AotHookEvaluationReceipt {
                host_output: Some(host_output),
                reader_probe_event_path: None,
            }
        });
    }
    Ok(AotHookEvaluationReceipt {
        host_output: Some(serde_json::json!({})),
        reader_probe_event_path: None,
    })
}

fn render_and_record_deny(
    decision: &aot_evaluator::AotHookDecision<'_>,
    _payload_json: &str,
) -> Result<serde_json::Value, String> {
    let typed = serde_json::to_value(decision).map_err(|error| error.to_string())?;
    let message = typed
        .get("message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("ASP Hook denied the operation.");
    Ok(crate::render_codex_pre_tool_deny(&typed, message))
}
