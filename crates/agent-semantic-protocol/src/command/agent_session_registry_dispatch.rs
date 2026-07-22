//! Durable resident-child dispatch claim and terminal-receipt commands.

use agent_semantic_client_db::{
    AgentSessionDispatchClaimRequest, AgentSessionDispatchCompleteRequest, AgentSessionRegistry,
    agent_session_unix_timestamp,
};

use super::agent_session_registry_args::SessionArgs;
use super::agent_session_registry_state::{
    current_project_session_scope_id, required_non_empty, resolved_root_session_id,
};

pub(super) fn claim_dispatch(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
) -> Result<(), String> {
    let project_id = current_project_session_scope_id(registry)?;
    let root_session_id = resolved_root_session_id(registry, args.root_session_id.as_deref())?
        .ok_or_else(|| {
            "dispatch claim requires the current or explicit root session id".to_string()
        })?;
    let name = required_non_empty(args.name.as_deref(), "--name")?;
    let dispatch_identity =
        required_non_empty(args.dispatch_identity.as_deref(), "--dispatch-identity")?;
    let command_digest = required_non_empty(args.command_digest.as_deref(), "--command-digest")?;
    let now = agent_session_unix_timestamp()?;
    let live_target =
        super::agent_session_registry_host_capability::fresh_host_resident_target_observation(
            registry,
            &root_session_id,
            name,
            now,
        )?;
    let verified_live_target = live_target.as_ref().is_some_and(|observation| {
        observation.target_status == "present" && observation.identity_status == "verified"
    });
    if !verified_live_target {
        let observed = live_target
            .as_ref()
            .map_or("missing-or-stale", |observation| {
                if observation.target_status == "absent" {
                    "absent"
                } else {
                    "present-unverified"
                }
            });
        return Err(format!(
            "dispatch-live-target-unverified: resident `{name}` host observation is {observed}; use a verified native binding"
        ));
    }
    let resident_bridge_target = if args.resident_bridge {
        let canonical_target = live_target
            .as_ref()
            .and_then(|observation| observation.canonical_target.as_deref())
            .ok_or_else(|| {
                format!(
                    "dispatch-live-target-unverified: resident `{name}` has no verified canonical target"
                )
            })?;
        Some(format!("resident-command-bridge:{canonical_target}"))
    } else {
        None
    };
    let result = registry.claim_dispatch(AgentSessionDispatchClaimRequest {
        project_id: &project_id,
        root_session_id: &root_session_id,
        name,
        dispatch_identity,
        command_digest,
        delivery_target_override: resident_bridge_target.as_deref(),
        now,
    })?;
    if args.json {
        println!(
            "{}",
            serde_json::to_string(&result)
                .map_err(|error| format!("failed to encode dispatch claim: {error}"))?
        );
    } else {
        println!(
            "[agent-session-dispatch] action={} identity=\"{}\" status={} attempt={} target=\"{}\"",
            result.action,
            dispatch_identity,
            result.lease.status,
            result.lease.attempt_count,
            result.lease.delivery_target_id.as_deref().unwrap_or("")
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/unit/agent_session_registry_dispatch.rs"]
mod tests;

pub(super) fn validate_exact_argv(argv: &[String]) -> Result<(), String> {
    argv.first()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "--command-json must encode a non-empty argv array".to_string())?;
    if argv.iter().any(|value| value.contains('\0')) {
        return Err("--command-json argv cannot contain NUL bytes".to_string());
    }
    if argv.windows(4).any(|window| {
        std::path::Path::new(&window[0])
            .file_name()
            .is_some_and(|name| name == "asp")
            && window[1] == "agent"
            && window[2] == "session"
            && window[3] == "dispatch-execute"
    }) {
        return Err("recursive `asp agent session dispatch-execute` is forbidden".to_string());
    }
    Ok(())
}

pub(super) fn dispatch_execution_context_allowed(
    current_session_id: &str,
    root_session_id: &str,
    delivery_target_id: &str,
) -> bool {
    current_session_id != root_session_id
        || delivery_target_id.starts_with("resident-command-bridge:/root/")
}

pub(super) fn execute_dispatch(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
) -> Result<(), String> {
    use sha2::Digest as _;

    let project_id = current_project_session_scope_id(registry)?;
    let root_session_id = resolved_root_session_id(registry, args.root_session_id.as_deref())?
        .ok_or_else(|| {
            "dispatch execution requires the current or explicit root session id".to_string()
        })?;
    let current_session_id = std::env::var("CODEX_THREAD_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "dispatch execution requires CODEX_THREAD_ID".to_string())?;
    let name = required_non_empty(args.name.as_deref(), "--name")?;
    let dispatch_identity =
        required_non_empty(args.dispatch_identity.as_deref(), "--dispatch-identity")?;
    let command_digest = required_non_empty(args.command_digest.as_deref(), "--command-digest")?;
    let command_json = required_non_empty(args.command_json.as_deref(), "--command-json")?;
    let argv = serde_json::from_str::<Vec<String>>(command_json)
        .map_err(|error| format!("--command-json must encode an argv string array: {error}"))?;
    validate_exact_argv(&argv)?;
    let canonical_argv = serde_json::to_string(&argv)
        .map_err(|error| format!("failed to encode canonical argv: {error}"))?;
    let observed_digest = format!("{:x}", sha2::Sha256::digest(canonical_argv.as_bytes()));
    if observed_digest != command_digest {
        return Err(format!(
            "dispatch-command-digest-mismatch: expected={command_digest} observed={observed_digest}"
        ));
    }

    let lease = registry
        .dispatch_lease(&project_id, &root_session_id, name, dispatch_identity)?
        .ok_or_else(|| {
            format!("dispatch lease `{dispatch_identity}` does not exist; claim it before execute")
        })?;
    if lease.command_digest != command_digest {
        return Err("dispatch lease command digest does not match --command-digest".to_string());
    }
    if lease.status == "terminal" {
        print_dispatch_receipt(args, dispatch_identity, &lease)?;
        return Ok(());
    }
    if lease.status != "in-flight" {
        return Err(format!(
            "dispatch lease `{dispatch_identity}` is not executable: status={}",
            lease.status
        ));
    }
    let delivery_target_id = lease
        .delivery_target_id
        .as_deref()
        .ok_or_else(|| format!("dispatch lease `{dispatch_identity}` has no delivery target"))?;
    if !dispatch_execution_context_allowed(
        &current_session_id,
        &root_session_id,
        delivery_target_id,
    ) {
        return Err(
            "root execution requires a claimed canonical resident-command bridge".to_string(),
        );
    }
    if args.resident_bridge {
        let canonical_target = delivery_target_id
            .strip_prefix("resident-command-bridge:")
            .ok_or_else(|| {
                "--resident-bridge requires a resident-command-bridge delivery target".to_string()
            })?;
        let observation =
            super::agent_session_registry_host_capability::fresh_host_resident_target_observation(
                registry,
                &root_session_id,
                name,
                agent_session_unix_timestamp()?,
            )?
            .ok_or_else(|| {
                "--resident-bridge requires a fresh host target observation".to_string()
            })?;
        if observation.target_status != "present"
            || observation.identity_status != "verified"
            || observation.canonical_target.as_deref() != Some(canonical_target)
        {
            return Err(
                "--resident-bridge target does not match the fresh verified canonical target"
                    .to_string(),
            );
        }
    } else if delivery_target_id.starts_with("resident-command-bridge:") {
        return Err(
            "resident-command-bridge delivery requires explicit --resident-bridge".to_string(),
        );
    }

    let execution = std::process::Command::new(&argv[0])
        .args(&argv[1..])
        .status();
    let (evidence_ref, successful_execution, execution_error) = match execution {
        Ok(status) => {
            let code = status
                .code()
                .map_or_else(|| "signal".to_string(), |code| code.to_string());
            (format!("parser-exit:{code}"), status.success(), None)
        }
        Err(error) => (
            format!("parser-spawn-error:{:?}", error.kind()),
            false,
            Some(error.to_string()),
        ),
    };
    let receipt = registry.complete_dispatch(AgentSessionDispatchCompleteRequest {
        project_id: &project_id,
        root_session_id: &root_session_id,
        name,
        dispatch_identity,
        command_digest,
        evidence_ref: &evidence_ref,
        now: agent_session_unix_timestamp()?,
    })?;
    print_dispatch_receipt(args, dispatch_identity, &receipt)?;
    if successful_execution {
        Ok(())
    } else if let Some(error) = execution_error {
        Err(format!("dispatch command failed to start: {error}"))
    } else {
        Err(format!("dispatch command failed: {evidence_ref}"))
    }
}

fn print_dispatch_receipt(
    args: &SessionArgs,
    dispatch_identity: &str,
    lease: &agent_semantic_client_db::agent_session_registry::AgentSessionDispatchLeaseRecord,
) -> Result<(), String> {
    if args.json {
        println!(
            "{}",
            serde_json::to_string(lease)
                .map_err(|error| format!("failed to encode dispatch receipt: {error}"))?
        );
    } else {
        println!(
            "[agent-session-dispatch] action=complete identity=\"{}\" status={} attempt={} evidence=\"{}\"",
            dispatch_identity,
            lease.status,
            lease.attempt_count,
            lease.evidence_ref.as_deref().unwrap_or("")
        );
    }
    Ok(())
}

pub(super) fn complete_dispatch(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
) -> Result<(), String> {
    let project_id = current_project_session_scope_id(registry)?;
    let root_session_id = resolved_root_session_id(registry, args.root_session_id.as_deref())?
        .ok_or_else(|| {
            "dispatch completion requires the current or explicit root session id".to_string()
        })?;
    let name = required_non_empty(args.name.as_deref(), "--name")?;
    let dispatch_identity =
        required_non_empty(args.dispatch_identity.as_deref(), "--dispatch-identity")?;
    let command_digest = required_non_empty(args.command_digest.as_deref(), "--command-digest")?;
    let evidence_ref = required_non_empty(args.evidence_ref.as_deref(), "--evidence-ref")?;
    let lease = registry.complete_dispatch(AgentSessionDispatchCompleteRequest {
        project_id: &project_id,
        root_session_id: &root_session_id,
        name,
        dispatch_identity,
        command_digest,
        evidence_ref,
        now: agent_session_unix_timestamp()?,
    })?;
    if args.json {
        println!(
            "{}",
            serde_json::to_string(&lease)
                .map_err(|error| format!("failed to encode dispatch receipt: {error}"))?
        );
    } else {
        println!(
            "[agent-session-dispatch] action=complete identity=\"{}\" status={} attempt={} evidence=\"{}\"",
            dispatch_identity, lease.status, lease.attempt_count, evidence_ref
        );
    }
    Ok(())
}

pub(super) fn mark_dispatch_orphaned(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
) -> Result<(), String> {
    let project_id = current_project_session_scope_id(registry)?;
    let root_session_id = resolved_root_session_id(registry, args.root_session_id.as_deref())?
        .ok_or_else(|| {
            "dispatch orphan marking requires the current or explicit root session id".to_string()
        })?;
    let name = required_non_empty(args.name.as_deref(), "--name")?;
    let dispatch_identity =
        required_non_empty(args.dispatch_identity.as_deref(), "--dispatch-identity")?;
    let command_digest = required_non_empty(args.command_digest.as_deref(), "--command-digest")?;
    let lease = registry.mark_dispatch_orphaned(
        agent_semantic_client_db::agent_session_registry::AgentSessionDispatchMarkOrphanedRequest {
            project_id: &project_id,
            root_session_id: &root_session_id,
            name,
            dispatch_identity,
            command_digest,
            now: agent_session_unix_timestamp()?,
        },
    )?;
    if args.json {
        println!(
            "{}",
            serde_json::to_string(&lease)
                .map_err(|error| format!("failed to encode dispatch receipt: {error}"))?
        );
    } else {
        println!(
            "[agent-session-dispatch] action=mark-orphaned identity=\"{}\" status={} attempt={} evidence=\"previous-dispatch-receipt-missing\"",
            dispatch_identity, lease.status, lease.attempt_count
        );
    }
    Ok(())
}
