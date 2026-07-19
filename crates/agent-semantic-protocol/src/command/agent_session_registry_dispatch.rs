//! Durable resident-child dispatch claim and terminal-receipt commands.

use agent_semantic_client_db::{
    AgentSessionDispatchClaimRequest, AgentSessionDispatchCompleteRequest, AgentSessionRegistry,
    agent_session_unix_timestamp,
};
use sha2::{Digest, Sha256};
use std::{env, process::Command};

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
    let resident_bridge_target = live_target.as_ref().and_then(|observation| {
        (args.resident_bridge && observation.target_status == "present")
            .then(|| observation.canonical_target.as_deref())
            .flatten()
    });
    if !verified_live_target && resident_bridge_target.is_none() {
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
            "dispatch-live-target-unverified: resident `{name}` host observation is {observed}; use a verified native binding, or --resident-bridge after auditing its configured canonical target as present"
        ));
    }
    let logical_resident_target =
        resident_bridge_target.map(|target| format!("resident-command-bridge:{target}"));
    let result = registry.claim_dispatch(AgentSessionDispatchClaimRequest {
        project_id: &project_id,
        root_session_id: &root_session_id,
        name,
        dispatch_identity,
        command_digest,
        delivery_target_override: logical_resident_target.as_deref(),
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

pub(super) fn execute_dispatch(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
) -> Result<(), String> {
    let project_id = current_project_session_scope_id(registry)?;
    let root_session_id = required_non_empty(args.root_session_id.as_deref(), "--root-session-id")?;
    let current_session_id = env::var("CODEX_THREAD_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "dispatch execute requires CODEX_THREAD_ID from a child task".to_string())?;
    let name = required_non_empty(args.name.as_deref(), "--name")?;
    let dispatch_identity =
        required_non_empty(args.dispatch_identity.as_deref(), "--dispatch-identity")?;
    let command_digest = required_non_empty(args.command_digest.as_deref(), "--command-digest")?;
    let command_json = required_non_empty(args.command_json.as_deref(), "--command-json")?;
    let argv: Vec<String> = serde_json::from_str(command_json)
        .map_err(|error| format!("decode dispatch command argv: {error}"))?;
    validate_exact_argv(&argv)?;
    let canonical_argv = serde_json::to_vec(&argv)
        .map_err(|error| format!("encode canonical dispatch argv: {error}"))?;
    let actual_digest = format!("{:x}", Sha256::digest(&canonical_argv));
    if actual_digest != command_digest {
        return Err(format!(
            "dispatch command digest mismatch: expected={command_digest} actual={actual_digest}"
        ));
    }
    let lease = registry
        .dispatch_lease(&project_id, root_session_id, name, dispatch_identity)?
        .ok_or_else(|| "dispatch execute requires an existing lease".to_string())?;
    if lease.command_digest != command_digest {
        return Err("dispatch lease digest does not match the exact parser argv".to_string());
    }
    if lease.status != "in-flight" {
        return Err(format!(
            "dispatch lease is not executable: status={}",
            lease.status
        ));
    }
    let delivery_target = lease.delivery_target_id.as_deref().unwrap_or_default();
    if !dispatch_execution_context_allowed(&current_session_id, root_session_id, delivery_target) {
        return Err(
            "dispatch execute in the root task requires a claimed canonical resident bridge lease"
                .to_string(),
        );
    }
    if !delivery_target.starts_with("resident-command-bridge:/root/") {
        return Err("dispatch lease is not a canonical resident bridge capability".to_string());
    }

    let executable = if argv.first().is_some_and(|value| value == "asp") {
        env::current_exe().map_err(|error| format!("resolve asp executable: {error}"))?
    } else {
        argv[0].clone().into()
    };
    let status = Command::new(executable)
        .args(&argv[1..])
        .status()
        .map_err(|error| format!("execute parser dispatch: {error}"))?;
    let exit_code = status.code().unwrap_or(1);
    let evidence_ref = format!("parser-exit:{exit_code}");
    registry.complete_dispatch(AgentSessionDispatchCompleteRequest {
        project_id: &project_id,
        root_session_id,
        name,
        dispatch_identity,
        command_digest,
        evidence_ref: &evidence_ref,
        now: agent_session_unix_timestamp()?,
    })?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("parser dispatch exited with status {exit_code}"))
    }
}

fn dispatch_execution_context_allowed(
    current_session_id: &str,
    root_session_id: &str,
    delivery_target: &str,
) -> bool {
    current_session_id != root_session_id
        || delivery_target.starts_with("resident-command-bridge:/root/")
}

fn validate_exact_argv(argv: &[String]) -> Result<(), String> {
    if argv.first().is_none_or(|binary| binary.trim().is_empty()) {
        return Err("resident dispatch argv requires a non-empty executable".to_string());
    }
    if argv.first().is_some_and(|binary| binary == "asp")
        && argv.get(1).is_some_and(|value| value == "agent")
        && argv.get(2).is_some_and(|value| value == "session")
        && argv.get(3).is_some_and(|value| value == "dispatch-execute")
    {
        return Err("resident dispatch cannot recursively execute itself".to_string());
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/unit/agent_session_registry_dispatch.rs"]
mod tests;

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
