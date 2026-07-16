//! Native Codex app-server host-tree observations for resident agents.

use serde_json::Value;

use crate::{CodexRolloutSessionMetadata, codex_rollout_session_metadata};

/// Reads direct native Codex child sessions for one root task.
///
/// This recovery lane is intentionally fail-soft: if the installed Codex
/// app-server is unavailable, ASP leaves lifecycle bootstrap in `Audit`
/// instead of blocking unrelated tool use.
pub fn codex_app_server_child_session_metadata(
    root_session_id: &str,
) -> Result<Vec<CodexRolloutSessionMetadata>, String> {
    let Some(threads) = read_direct_child_threads(root_session_id) else {
        return Ok(Vec::new());
    };
    trace(
        root_session_id,
        "thread-list-received",
        serde_json::json!({ "threadCount": threads.len() }),
    );
    let mut records = Vec::new();
    for thread in &threads {
        if let Some(mut record) = child_rollout_metadata(thread, root_session_id)? {
            if let Some(runtime) = read_thread_runtime_observation(&record.session_id) {
                record.model = runtime.model.or(record.model);
                record.reasoning_effort = runtime.reasoning_effort.or(record.reasoning_effort);
            }
            records.push(record);
        }
    }
    trace(
        root_session_id,
        "complete",
        serde_json::json!({ "recordCount": records.len() }),
    );
    Ok(records)
}

struct CodexThreadRuntimeObservation {
    model: Option<String>,
    reasoning_effort: Option<String>,
}

/// Reads the runtime settings owned by Codex for an existing child thread.
///
/// SubagentStart does not expose reasoning effort.  `thread/resume` does, and
/// a resume without overrides does not start a turn or ask the child to attest
/// its own settings.  This is therefore the host-owned runtime evidence lane
/// used to complete typed resident validation.
fn read_thread_runtime_observation(
    child_session_id: &str,
) -> Option<CodexThreadRuntimeObservation> {
    let codex_binary = std::env::var_os("ASP_CODEX_BIN").unwrap_or_else(|| "codex".into());
    let mut child = std::process::Command::new(codex_binary)
        .args(["app-server", "--stdio"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let requests = [
        serde_json::json!({
            "method": "initialize",
            "id": 1,
            "params": {
                "clientInfo": {
                    "name": "asp_lifecycle_runtime_audit",
                    "title": "ASP Lifecycle Runtime Audit",
                    "version": env!("CARGO_PKG_VERSION"),
                },
                "capabilities": { "experimentalApi": true },
            },
        }),
        serde_json::json!({ "method": "initialized", "params": {} }),
        serde_json::json!({
            "method": "thread/resume",
            "id": 2,
            "params": {
                "threadId": child_session_id,
                "excludeTurns": true,
            },
        }),
    ]
    .into_iter()
    .map(|request| request.to_string())
    .collect::<Vec<_>>()
    .join("\n")
        + "\n";
    let mut stdin = child.stdin.take()?;
    if std::io::Write::write_all(&mut stdin, requests.as_bytes()).is_err() {
        let _ = child.kill();
        return None;
    }
    let stdout = child.stdout.take()?;
    let response = std::io::BufRead::lines(std::io::BufReader::new(stdout))
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<Value>(&line).ok())
        .find(|value| value.get("id").and_then(Value::as_i64) == Some(2));
    let _ = child.kill();
    let _ = child.wait();
    let response = response?;
    if response.get("error").is_some() {
        return None;
    }
    Some(CodexThreadRuntimeObservation {
        model: response
            .pointer("/result/model")
            .and_then(Value::as_str)
            .map(str::to_string),
        reasoning_effort: response
            .pointer("/result/reasoningEffort")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn read_direct_child_threads(root_session_id: &str) -> Option<Vec<Value>> {
    let codex_binary = std::env::var_os("ASP_CODEX_BIN").unwrap_or_else(|| "codex".into());
    let mut child = match std::process::Command::new(codex_binary)
        .args(["app-server", "--stdio"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            trace(
                root_session_id,
                "spawn-unavailable",
                serde_json::json!({ "error": error.to_string() }),
            );
            return None;
        }
    };
    let requests = app_server_requests(root_session_id);
    let mut stdin = child.stdin.take()?;
    if std::io::Write::write_all(&mut stdin, requests.as_bytes()).is_err() {
        let _ = child.kill();
        return None;
    }
    let stdout = child.stdout.take()?;
    let response = std::io::BufRead::lines(std::io::BufReader::new(stdout))
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<Value>(&line).ok())
        .find(|value| value.get("id").and_then(Value::as_i64) == Some(2));
    let _ = child.kill();
    let _ = child.wait();
    let response = response?;
    if let Some(error) = response.get("error") {
        trace(
            root_session_id,
            "thread-list-error",
            serde_json::json!({ "error": error }),
        );
        return None;
    }
    response
        .pointer("/result/data")
        .and_then(Value::as_array)
        .cloned()
}

fn app_server_requests(root_session_id: &str) -> String {
    [
        serde_json::json!({
            "method": "initialize",
            "id": 1,
            "params": {
                "clientInfo": {
                    "name": "asp_lifecycle_audit",
                    "title": "ASP Lifecycle Audit",
                    "version": env!("CARGO_PKG_VERSION"),
                },
                "capabilities": { "experimentalApi": true },
            },
        }),
        serde_json::json!({ "method": "initialized", "params": {} }),
        serde_json::json!({
            "method": "thread/list",
            "id": 2,
            "params": {
                "parentThreadId": root_session_id,
                "limit": 100,
                "sortKey": "updated_at",
                "sortDirection": "desc",
            },
        }),
    ]
    .into_iter()
    .map(|request| request.to_string())
    .collect::<Vec<_>>()
    .join("\n")
        + "\n"
}

fn child_rollout_metadata(
    thread: &Value,
    root_session_id: &str,
) -> Result<Option<CodexRolloutSessionMetadata>, String> {
    let Some(child_session_id) = thread.get("id").and_then(Value::as_str) else {
        return Ok(None);
    };
    let parent_thread_id = json_string(
        thread,
        &[
            "/parentThreadId",
            "/parent_thread_id",
            "/source/subAgent/thread_spawn/parent_thread_id",
            "/source/subAgent/threadSpawn/parentThreadId",
        ],
    );
    if parent_thread_id.as_deref() != Some(root_session_id) {
        return Ok(None);
    }
    let agent_path = json_string(
        thread,
        &[
            "/source/subAgent/thread_spawn/agent_path",
            "/source/subAgent/threadSpawn/agentPath",
            "/source/sub_agent/thread_spawn/agent_path",
        ],
    );
    let rollout_metadata = codex_rollout_session_metadata(child_session_id)?;
    trace(
        root_session_id,
        "child-candidate",
        serde_json::json!({
            "childSessionId": child_session_id,
            "parentThreadId": parent_thread_id,
            "agentPath": agent_path,
            "rolloutMetadataPresent": rollout_metadata.is_some(),
        }),
    );
    let Some(mut metadata) = rollout_metadata else {
        return Ok(None);
    };
    metadata.session_id = child_session_id.to_string();
    metadata.root_session_id = Some(root_session_id.to_string());
    metadata.parent_thread_id = Some(root_session_id.to_string());
    metadata.thread_source = Some("subagent".to_string());
    metadata.agent_path = agent_path;
    metadata.agent_role = json_string(thread, &["/agentRole", "/agent_role"]);
    metadata.spawn_depth = thread
        .pointer("/source/subAgent/thread_spawn/depth")
        .or_else(|| thread.pointer("/source/subAgent/threadSpawn/depth"))
        .and_then(Value::as_u64)
        .and_then(|depth| i64::try_from(depth).ok())
        .or(Some(1));
    Ok(Some(metadata))
}

fn json_string(value: &Value, pointers: &[&str]) -> Option<String> {
    pointers
        .iter()
        .find_map(|pointer| value.pointer(pointer).and_then(Value::as_str))
        .map(str::to_string)
}

fn trace(root_session_id: &str, stage: &str, details: Value) {
    if std::env::var_os("ASP_CODEX_APP_SERVER_TRACE").is_some() {
        eprintln!(
            "{}",
            serde_json::json!({
                "trace": "codex-app-server-host-tree",
                "stage": stage,
                "rootSessionId": root_session_id,
                "details": details,
            })
        );
    }
}
