//! Codex rollout JSONL session index parser.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{BufRead, BufReader},
    path::Path,
};

use serde_json::Value;


use super::parse::first_json_string;

pub(super) fn thread_spawn_child_session_ids_for_rollout(
    lines: &[String],
    root_session_id: &str,
) -> Vec<String> {
    let mut child_session_ids = BTreeSet::new();
    for line in lines {
        if !line.contains("thread_spawn") {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if value.get("type").and_then(Value::as_str) != Some("response_item") {
            continue;
        }
        let Some(payload) = value.get("payload") else {
            continue;
        };
        if payload.get("type").and_then(Value::as_str) != Some("thread_spawn") {
            continue;
        }
        let parent_matches = match payload.get("parent_thread_id").and_then(Value::as_str) {
            Some(parent_thread_id) => parent_thread_id == root_session_id,
            None => true,
        };
        if !parent_matches {
            continue;
        }
        if let Some(child_session_id) = payload.get("id").and_then(Value::as_str) {
            child_session_ids.insert(child_session_id.to_string());
        }
    }
    child_session_ids.into_iter().collect()
}

pub(super) fn spawned_agent_ids_for_rollout(lines: &[String]) -> Vec<String> {
    let mut ids = BTreeSet::new();
    for line in lines {
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        collect_structured_subagent_spawn_evidence(&value, &mut ids, &mut BTreeMap::new());
        if value.get("type").and_then(Value::as_str) != Some("response_item") {
            continue;
        }
        let Some(payload) = value.get("payload") else {
            continue;
        };
        if payload.get("type").and_then(Value::as_str) != Some("function_call_output") {
            continue;
        }
        let output_json = payload.get("output").and_then(|output| match output {
            Value::String(output) => serde_json::from_str::<Value>(output).ok(),
            Value::Object(_) => Some(output.clone()),
            _ => None,
        });
        let Some(output_json) = output_json else {
            continue;
        };
        if let Some(agent_id) = first_json_string(
            &output_json,
            &[
                "/agent_id",
                "/agentId",
                "/agent_thread_id",
                "/agentThreadId",
            ],
        ) {
            ids.insert(agent_id);
        }
    }
    ids.into_iter().collect()
}

pub(super) fn spawned_agent_paths_for_rollout(lines: &[String]) -> BTreeMap<String, String> {
    let mut task_name_by_call = BTreeMap::new();
    let mut agent_path_by_session = BTreeMap::new();
    for line in lines {
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        collect_structured_subagent_spawn_evidence(
            &value,
            &mut BTreeSet::new(),
            &mut agent_path_by_session,
        );
        if value.get("type").and_then(Value::as_str) != Some("response_item") {
            continue;
        }
        let Some(payload) = value.get("payload") else {
            continue;
        };
        let payload_type = payload.get("type").and_then(Value::as_str);
        if payload_type == Some("function_call") {
            let tool_name = payload
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if !tool_name.ends_with("spawn_agent") {
                continue;
            }
            let Some(call_id) = first_json_string(payload, &["/call_id", "/callId", "/id"]) else {
                continue;
            };
            let arguments = payload
                .get("arguments")
                .and_then(|arguments| match arguments {
                    Value::String(arguments) => serde_json::from_str::<Value>(arguments).ok(),
                    Value::Object(_) => Some(arguments.clone()),
                    _ => None,
                });
            if let Some(task_name) = arguments.as_ref().and_then(|arguments| {
                first_json_string(arguments, &["/task_name", "/taskName", "/name"])
            }) {
                task_name_by_call.insert(call_id, task_name);
            }
            continue;
        }
        if payload_type != Some("function_call_output") {
            continue;
        }
        let Some(call_id) = first_json_string(payload, &["/call_id", "/callId", "/id"]) else {
            continue;
        };
        let Some(task_name) = task_name_by_call.get(&call_id) else {
            continue;
        };
        let output = payload.get("output").and_then(|output| match output {
            Value::String(output) => serde_json::from_str::<Value>(output).ok(),
            Value::Object(_) => Some(output.clone()),
            _ => None,
        });
        let Some(output) = output else {
            continue;
        };
        let Some(agent_id) = first_json_string(
            &output,
            &[
                "/agent_id",
                "/agentId",
                "/agent_thread_id",
                "/agentThreadId",
            ],
        ) else {
            continue;
        };
        let agent_path = first_json_string(
            &output,
            &["/agent_path", "/agentPath", "/agent_name", "/agentName"],
        )
        .unwrap_or_else(|| format!("/root/{task_name}"));
        agent_path_by_session.insert(agent_id, agent_path);
    }
    agent_path_by_session
}

pub(super) fn rollout_topology_lines(rollout_path: &Path) -> Result<Vec<String>, String> {
    let file = fs::File::open(rollout_path).map_err(|error| {
        format!(
            "failed to open Codex rollout topology {}: {error}",
            rollout_path.display()
        )
    })?;
    BufReader::new(file)
        .lines()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            format!(
                "failed to read Codex rollout topology {}: {error}",
                rollout_path.display()
            )
        })
}

pub(super) fn collect_structured_subagent_spawn_evidence(
    value: &Value,
    session_ids: &mut BTreeSet<String>,
    agent_path_by_session: &mut BTreeMap<String, String>,
) {
    match value {
        Value::Object(object) => {
            let object_value = Value::Object(object.clone());
            let activity_kind = first_json_string(&object_value, &["/kind", "/status"]);
            let activity_session_id = first_json_string(
                &object_value,
                &[
                    "/agent_thread_id",
                    "/agentThreadId",
                    "/agent_id",
                    "/agentId",
                ],
            );
            let activity_agent_path =
                first_json_string(&object_value, &["/agent_path", "/agentPath"]);
            if matches!(activity_kind.as_deref(), Some("started" | "interacted"))
                && let (Some(session_id), Some(agent_path)) =
                    (activity_session_id, activity_agent_path)
                && agent_path.starts_with("/root/")
            {
                session_ids.insert(session_id.clone());
                agent_path_by_session.insert(session_id, agent_path);
            }

            let tool = first_json_string(&object_value, &["/tool"]);
            let status = first_json_string(&object_value, &["/status"]);
            if tool.as_deref() == Some("spawn_agent") && status.as_deref() == Some("completed") {
                if let Some(receiver_thread_ids) = object
                    .get("receiver_thread_ids")
                    .or_else(|| object.get("receiverThreadIds"))
                    .and_then(Value::as_array)
                {
                    session_ids.extend(
                        receiver_thread_ids
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string),
                    );
                }
                if let Some(receiver_agents) = object
                    .get("receiver_agents")
                    .or_else(|| object.get("receiverAgents"))
                    .and_then(Value::as_array)
                {
                    for receiver in receiver_agents {
                        let Some(session_id) = first_json_string(
                            receiver,
                            &[
                                "/thread_id",
                                "/threadId",
                                "/agent_thread_id",
                                "/agentThreadId",
                            ],
                        ) else {
                            continue;
                        };
                        session_ids.insert(session_id.clone());
                        let Some(agent_role) =
                            first_json_string(receiver, &["/agent_role", "/agentRole"])
                        else {
                            continue;
                        };
                        if agent_role != "default" {
                            agent_path_by_session
                                .entry(session_id)
                                .or_insert_with(|| format!("/root/{agent_role}"));
                        }
                    }
                }
            }
            for child in object.values() {
                if !child.is_string() {
                    collect_structured_subagent_spawn_evidence(
                        child,
                        session_ids,
                        agent_path_by_session,
                    );
                }
            }
        }
        Value::Array(values) => {
            for child in values {
                collect_structured_subagent_spawn_evidence(
                    child,
                    session_ids,
                    agent_path_by_session,
                );
            }
        }
        _ => {}
    }
}
