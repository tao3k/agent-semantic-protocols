//! Explicit acceptance of Codex Host-to-Hook delivery from a normal-task rollout.
//!
//! This is a diagnostic-plane reader. The Hook event path must never discover or
//! scan Codex rollouts itself.

use serde::Serialize;
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

/// The diagnostic plane must have a fixed cost even after a long-running
/// normal task.  The Host declaration is emitted near the start of a rollout,
/// while the probe and Hook decision are emitted near its end.
const ROLLOUT_PREFIX_BYTES: u64 = 1024 * 1024;
const ROLLOUT_TAIL_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct HostAcceptanceReceipt {
    schema_id: &'static str,
    schema_version: &'static str,
    state: &'static str,
    reason_kind: &'static str,
    plugin_loaded: bool,
    probe_call_observed: bool,
    hook_event_observed: bool,
    deny_observed: bool,
    source_bytes_returned: bool,
}

impl HostAcceptanceReceipt {
    pub(super) fn accepted(&self) -> bool {
        self.state == "accepted"
    }

    pub(super) fn state(&self) -> &'static str {
        self.state
    }

    pub(super) fn reason_kind(&self) -> &'static str {
        self.reason_kind
    }
}

pub(super) fn run_accept_host(args: &[String]) -> Result<(), String> {
    let rollout_path = argument_value(args, "--host-rollout")
        .ok_or_else(|| "missing required --host-rollout PATH".to_owned())?;
    let probe_path = argument_value(args, "--host-probe-path")
        .ok_or_else(|| "missing required --host-probe-path PATH".to_owned())?;
    let source_sentinel = argument_value(args, "--host-sentinel")
        .ok_or_else(|| "missing required --host-sentinel TOKEN".to_owned())?;
    let receipt = inspect_host_rollout(Path::new(rollout_path), probe_path, source_sentinel)?;
    println!(
        "{}",
        serde_json::to_string(&receipt)
            .map_err(|error| format!("failed to serialize Host acceptance receipt: {error}"))?
    );
    if receipt.accepted() {
        Ok(())
    } else {
        Err(format!(
            "normal-task Hook Host acceptance failed: reasonKind={}",
            receipt.reason_kind()
        ))
    }
}

fn argument_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|argument| argument == flag)
        .and_then(|index| args.get(index + 1))
        .map(String::as_str)
}

#[derive(Default)]
struct HostAcceptanceEvidence {
    plugin_loaded: bool,
    probe_call_observed: bool,
    hook_event_observed: bool,
    deny_observed: bool,
    source_bytes_returned: bool,
}

struct HostAcceptanceContext<'a> {
    rollout_path: &'a Path,
    probe_path: &'a str,
    source_sentinel: &'a str,
}

pub(super) fn inspect_host_rollout(
    rollout_path: &Path,
    probe_path: &str,
    source_sentinel: &str,
) -> Result<HostAcceptanceReceipt, String> {
    if probe_path.trim().is_empty() {
        return Err("--host-probe-path must not be empty".to_owned());
    }
    if source_sentinel.trim().is_empty() {
        return Err("--host-sentinel must not be empty".to_owned());
    }
    let metadata = std::fs::metadata(rollout_path).map_err(|error| {
        format!(
            "failed to inspect Host rollout {}: {error}",
            rollout_path.display()
        )
    })?;
    let mut file = File::open(rollout_path).map_err(|error| {
        format!(
            "failed to open Host rollout {}: {error}",
            rollout_path.display()
        )
    })?;
    let mut evidence = HostAcceptanceEvidence::default();
    let context = HostAcceptanceContext {
        rollout_path,
        probe_path,
        source_sentinel,
    };
    let prefix_len = metadata.len().min(ROLLOUT_PREFIX_BYTES);
    inspect_rollout_window(&mut file, 0, prefix_len, false, &context, &mut evidence)?;
    if metadata.len() > prefix_len {
        let tail_start = metadata.len().saturating_sub(ROLLOUT_TAIL_BYTES);
        inspect_rollout_window(
            &mut file,
            tail_start,
            metadata.len() - tail_start,
            tail_start != 0,
            &context,
            &mut evidence,
        )?;
    }

    let (state, reason_kind) = if !evidence.plugin_loaded {
        ("rejected", "plugin-not-loaded")
    } else if !evidence.probe_call_observed {
        ("rejected", "probe-call-missing")
    } else if evidence.source_bytes_returned {
        ("rejected", "source-bytes-leaked")
    } else if !evidence.hook_event_observed {
        ("rejected", "hook-event-missing")
    } else if !evidence.deny_observed {
        ("rejected", "hook-deny-missing")
    } else {
        ("accepted", "normal-task-hook-deny-observed")
    };

    Ok(HostAcceptanceReceipt {
        schema_id: "agent.semantic-protocols.hook-host-acceptance",
        schema_version: "1",
        state,
        reason_kind,
        plugin_loaded: evidence.plugin_loaded,
        probe_call_observed: evidence.probe_call_observed,
        hook_event_observed: evidence.hook_event_observed,
        deny_observed: evidence.deny_observed,
        source_bytes_returned: evidence.source_bytes_returned,
    })
}

fn inspect_rollout_window(
    file: &mut File,
    start: u64,
    length: u64,
    skip_first_partial_line: bool,
    context: &HostAcceptanceContext<'_>,
    evidence: &mut HostAcceptanceEvidence,
) -> Result<(), String> {
    file.seek(SeekFrom::Start(start)).map_err(|error| {
        format!(
            "failed to seek Host rollout {} to byte {start}: {error}",
            context.rollout_path.display()
        )
    })?;
    let mut reader = BufReader::new(file.take(length));
    if skip_first_partial_line {
        let mut partial = Vec::new();
        reader.read_until(b'\n', &mut partial).map_err(|error| {
            format!(
                "failed to align Host rollout {} tail window: {error}",
                context.rollout_path.display()
            )
        })?;
    }

    let mut line = String::new();
    let mut record = 0_u64;
    loop {
        line.clear();
        let bytes = reader.read_line(&mut line).map_err(|error| {
            format!(
                "failed to read Host rollout {} near byte {}: {error}",
                context.rollout_path.display(),
                start + record
            )
        })?;
        if bytes == 0 {
            break;
        }
        record += bytes as u64;
        // A bounded window can end in the middle of a JSONL item.  It is not
        // an invalid rollout record and must not turn a long session into a
        // false rejection.
        if !line.ends_with('\n') {
            break;
        }
        let value: Value = serde_json::from_str(line.trim_end()).map_err(|error| {
            format!(
                "invalid Host rollout JSON at {} near byte {}: {error}",
                context.rollout_path.display(),
                start + record
            )
        })?;
        observe_rollout_item(
            &value,
            context.probe_path,
            context.source_sentinel,
            evidence,
        );
    }
    Ok(())
}

fn observe_rollout_item(
    value: &Value,
    probe_path: &str,
    source_sentinel: &str,
    evidence: &mut HostAcceptanceEvidence,
) {
    let payload_type = value.pointer("/payload/type").and_then(Value::as_str);
    if payload_type == Some("function_call")
        && value
            .pointer("/payload/arguments")
            .and_then(Value::as_str)
            .is_some_and(|arguments| arguments.contains(probe_path))
    {
        evidence.probe_call_observed = true;
    }
    if payload_type == Some("function_call_output")
        && value
            .pointer("/payload/output")
            .and_then(Value::as_str)
            .is_some_and(|output| output.contains(source_sentinel))
    {
        evidence.source_bytes_returned = true;
    }

    let serialized = serde_json::to_string(value).unwrap_or_default();
    let asp_hook_evidence = serialized.contains("agent.semantic-protocols.hook.decision")
        || serialized.contains("[asp-hook]")
        || serialized.contains("[agent-hook-decision]");
    if asp_hook_evidence {
        evidence.plugin_loaded = true;
        evidence.hook_event_observed = true;
    }
    if asp_hook_evidence
        && (serialized.contains("\\\"permissionDecision\\\":\\\"deny\\\"")
            || serialized.contains("\\\"decision\\\":\\\"deny\\\"")
            || serialized.contains("permissionDecision=deny"))
    {
        evidence.deny_observed = true;
    }
}

#[cfg(test)]
#[path = "../../tests/unit/command/hook_host_acceptance.rs"]
mod tests;
