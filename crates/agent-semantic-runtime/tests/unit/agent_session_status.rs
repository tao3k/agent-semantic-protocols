use std::{fs, path::Path};

use crate::agent_session_status::{
    codex_rollout_session_metadata, codex_rollout_session_metadata_recent,
    current_agent_runtime_session,
};
use crate::codex_rollout_sessions::codex_rollout_session_index;

static CODEX_HOME_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
static AGENT_SESSION_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const AGENT_SESSION_ENV_VARS: [&str; 5] = [
    "CODEX_THREAD_ID",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_REMOTE_SESSION_ID",
    "AGENT_SESSION_ID",
    "SESSION_ID",
];

struct CodexHomeEnvGuard {
    _guard: std::sync::MutexGuard<'static, ()>,
}

impl CodexHomeEnvGuard {
    fn set(path: &Path) -> Self {
        let guard = CODEX_HOME_ENV_LOCK.lock().expect("codex env lock");
        unsafe {
            std::env::set_var("CODEX_HOME", path);
        }
        Self { _guard: guard }
    }
}

impl Drop for CodexHomeEnvGuard {
    fn drop(&mut self) {
        unsafe {
            std::env::remove_var("CODEX_HOME");
        }
    }
}

struct AgentSessionEnvGuard {
    _guard: std::sync::MutexGuard<'static, ()>,
    previous: Vec<(&'static str, Option<String>)>,
}

impl AgentSessionEnvGuard {
    fn set(vars: &[(&'static str, &str)]) -> Self {
        let guard = AGENT_SESSION_ENV_LOCK
            .lock()
            .expect("agent session env lock");
        let previous = AGENT_SESSION_ENV_VARS
            .iter()
            .map(|name| (*name, std::env::var(name).ok()))
            .collect::<Vec<_>>();
        unsafe {
            for name in AGENT_SESSION_ENV_VARS {
                std::env::remove_var(name);
            }
            for (name, value) in vars {
                std::env::set_var(name, value);
            }
        }
        Self {
            _guard: guard,
            previous,
        }
    }
}

impl Drop for AgentSessionEnvGuard {
    fn drop(&mut self) {
        unsafe {
            for name in AGENT_SESSION_ENV_VARS {
                std::env::remove_var(name);
            }
            for (name, value) in &self.previous {
                if let Some(value) = value {
                    std::env::set_var(name, value);
                }
            }
        }
    }
}

#[test]
fn current_agent_runtime_session_uses_codex_thread_id() {
    let _env = AgentSessionEnvGuard::set(&[("CODEX_THREAD_ID", "codex-session-1")]);

    let session = current_agent_runtime_session().expect("codex session");

    assert_eq!(session.client, "codex");
    assert_eq!(session.id, "codex-session-1");
}

#[test]
fn current_agent_runtime_session_prefers_codex_thread_id_over_other_ids() {
    let _env = AgentSessionEnvGuard::set(&[
        ("CODEX_THREAD_ID", "codex-session-2"),
        ("AGENT_SESSION_ID", "agent-session"),
        ("SESSION_ID", "generic-session"),
    ]);

    let session = current_agent_runtime_session().expect("codex session");

    assert_eq!(session.client, "codex");
    assert_eq!(session.id, "codex-session-2");
}

#[test]
fn codex_rollout_metadata_uses_latest_turn_context() {
    let root =
        std::env::temp_dir().join(format!("asp-runtime-rollout-latest-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let _env = CodexHomeEnvGuard::set(&root);
    let rollout_dir = root.join("sessions").join("2026").join("07").join("02");
    fs::create_dir_all(&rollout_dir).expect("create rollout dir");
    let rollout = rollout_dir.join("rollout-test-child-session.jsonl");
    let session_meta = serde_json::json!({
        "type": "session_meta",
        "payload": {
            "id": "child-session",
            "session_id": "root-session",
            "parent_thread_id": "root-session",
            "thread_source": "subagent",
            "agent_role": "asp_explorer"
        }
    });
    let first_turn = serde_json::json!({
        "type": "turn_context",
        "payload": {
            "model": "gpt-5.3-codex-spark",
            "reasoning_effort": "medium",
            "sandbox_policy": {"type": "read-only"},
            "approval_policy": "never",
            "permission_profile": {"type": "disabled"}
        }
    });
    let latest_turn = serde_json::json!({
        "type": "turn_context",
        "payload": {
            "model": "gpt-5.5",
            "effort": "low",
            "sandbox_policy": {"type": "danger-full-access"},
            "approval_policy": "on-request",
            "permission_profile": {"type": "full"}
        }
    });
    fs::write(
        &rollout,
        format!("{session_meta}\n{first_turn}\n{latest_turn}\n"),
    )
    .expect("write rollout");

    let metadata = codex_rollout_session_metadata("child-session")
        .expect("read metadata")
        .expect("metadata");

    assert_eq!(metadata.model.as_deref(), Some("gpt-5.5"));
    assert_eq!(metadata.reasoning_effort.as_deref(), Some("low"));
    assert_eq!(
        metadata.sandbox_policy.as_deref(),
        Some("danger-full-access")
    );
    assert_eq!(metadata.approval_policy.as_deref(), Some("on-request"));
    assert_eq!(metadata.permission_profile.as_deref(), Some("full"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn codex_rollout_metadata_recent_rejects_stale_registration_window() {
    let root =
        std::env::temp_dir().join(format!("asp-runtime-rollout-recent-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let _env = CodexHomeEnvGuard::set(&root);
    let rollout_dir = root.join("sessions").join("2026").join("07").join("02");
    fs::create_dir_all(&rollout_dir).expect("create rollout dir");
    let rollout = rollout_dir.join("rollout-2026-07-02T14-13-13-child-session.jsonl");
    let session_meta = serde_json::json!({
        "type": "session_meta",
        "payload": {
            "id": "child-session",
            "session_id": "root-session",
            "parent_thread_id": "root-session",
            "thread_source": "subagent",
            "agent_role": "asp_explorer"
        }
    });
    let turn_context = serde_json::json!({
        "type": "turn_context",
        "payload": {
            "model": "gpt-5.3-codex-spark",
            "sandbox_policy": {"type": "read-only"},
            "approval_policy": "never",
            "permission_profile": {"type": "disabled"}
        }
    });
    fs::write(&rollout, format!("{session_meta}\n{turn_context}\n")).expect("write rollout");

    let metadata = codex_rollout_session_metadata("child-session")
        .expect("read metadata")
        .expect("metadata");
    let reference_unix = metadata.rollout_created_at_unix.expect("rollout timestamp");

    assert!(
        codex_rollout_session_metadata_recent("child-session", reference_unix + 30, 30)
            .expect("recent lookup")
            .is_some()
    );
    assert!(
        codex_rollout_session_metadata_recent("child-session", reference_unix + 31, 30)
            .expect("stale lookup")
            .is_none()
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn codex_rollout_session_index_lists_root_subagents_and_nested_depth() {
    let root_session_id = "019f1f1a-5389-7223-a150-77dcb5ea8dd4";
    let child_session_id = "019f26e8-0dd6-71d3-8539-c362032b9e15";
    let nested_session_id = "019f28fc-81b5-7920-83a7-5b800210d8a8";
    let root =
        std::env::temp_dir().join(format!("asp-runtime-rollout-index-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let _env = CodexHomeEnvGuard::set(&root);
    let root_rollout_dir = root.join("sessions").join("2026").join("07").join("01");
    let child_rollout_dir = root.join("sessions").join("2026").join("07").join("03");
    fs::create_dir_all(&root_rollout_dir).expect("create root rollout dir");
    fs::create_dir_all(&child_rollout_dir).expect("create child rollout dir");

    let root_rollout = root_rollout_dir.join(format!(
        "rollout-2026-07-01T12-14-06-{root_session_id}.jsonl"
    ));
    let root_session_meta = serde_json::json!({
        "type": "session_meta",
        "payload": {
            "id": root_session_id,
            "session_id": root_session_id,
            "cwd": "/workspace"
        }
    });
    let child_spawn_output =
        serde_json::json!({"agent_id": child_session_id, "nickname": "ASP owner"});
    let child_spawn = serde_json::json!({
        "type": "response_item",
        "payload": {
            "type": "function_call_output",
            "output": child_spawn_output.to_string()
        }
    });
    fs::write(
        &root_rollout,
        format!("{root_session_meta}\n{child_spawn}\n"),
    )
    .expect("write root rollout");

    let child = child_rollout_dir.join(format!(
        "rollout-2026-07-03T00-36-10-{child_session_id}.jsonl"
    ));
    let child_session_meta = serde_json::json!({
        "type": "session_meta",
        "payload": {
            "id": child_session_id,
            "session_id": root_session_id,
            "parent_thread_id": root_session_id,
            "thread_source": "subagent",
            "agent_nickname": "ASP owner",
            "agent_role": "asp_explorer",
            "cwd": "/workspace",
            "source": {
                "subagent": {
                    "thread_spawn": {
                        "parent_thread_id": root_session_id,
                        "depth": 1,
                        "agent_nickname": "ASP owner",
                        "agent_role": "asp_explorer",
                        "agent_path": "/Users/example/.codex/agents/asp-explorer.toml"
                    }
                }
            }
        }
    });
    let child_turn_context = serde_json::json!({
        "type": "turn_context",
        "payload": {
            "model": "gpt-5.4-mini",
            "reasoningEffort": "low",
            "sandbox_policy": {"type": "read-only"},
            "approval_policy": "never",
            "permission_profile": {"type": "disabled"}
        }
    });
    let nested_spawn_output =
        serde_json::json!({"agent_id": nested_session_id, "nickname": "ASP selector"});
    let nested_spawn = serde_json::json!({
        "type": "response_item",
        "payload": {
            "type": "function_call_output",
            "output": nested_spawn_output.to_string()
        }
    });
    fs::write(
        &child,
        format!("{child_session_meta}\n{child_turn_context}\n{nested_spawn}\n"),
    )
    .expect("write child rollout");

    let nested = child_rollout_dir.join(format!(
        "rollout-2026-07-03T10-17-44-{nested_session_id}.jsonl"
    ));
    let nested_session_meta = serde_json::json!({
        "type": "session_meta",
        "payload": {
            "id": nested_session_id,
            "session_id": root_session_id,
            "parent_thread_id": child_session_id,
            "thread_source": "subagent",
            "agent_nickname": "ASP selector",
            "agent_role": "asp_explorer",
            "source": {
                "subagent": {
                    "thread_spawn": {
                        "parent_thread_id": child_session_id,
                        "depth": 2,
                        "agent_nickname": "ASP selector",
                        "agent_role": "asp_explorer"
                    }
                }
            }
        }
    });
    let nested_turn_context = serde_json::json!({
        "type": "turn_context",
        "payload": {
            "model": "gpt-5.4-mini"
        }
    });
    let nested_closed = serde_json::json!({
        "type": "event_msg",
        "timestamp": 10,
        "payload": {
            "status": "closed",
            "turn_id": "nested-turn"
        }
    });
    fs::write(
        &nested,
        format!("{nested_session_meta}\n{nested_turn_context}\n{nested_closed}\n"),
    )
    .expect("write nested rollout");

    let other = child_rollout_dir
        .join("rollout-2026-07-03T10-18-00-019f2924-7c9b-7640-b536-07dc9920030e.jsonl");
    let other_session_meta = serde_json::json!({
        "type": "session_meta",
        "payload": {
            "id": "019f2924-7c9b-7640-b536-07dc9920030e",
            "session_id": "other-root",
            "parent_thread_id": "other-root",
            "thread_source": "subagent"
        }
    });
    fs::write(&other, format!("{other_session_meta}\n")).expect("write other rollout");

    let index = codex_rollout_session_index(root_session_id)
        .expect("index lookup")
        .expect("index");
    assert_eq!(index.root_session_id, root_session_id);
    assert_eq!(index.records.len(), 2);
    assert_eq!(index.scanned_rollout_count, 3);

    let child = index
        .records
        .iter()
        .find(|record| record.session_id == child_session_id)
        .expect("child record");
    assert_eq!(child.parent_thread_id.as_deref(), Some(root_session_id));
    assert_eq!(child.thread_source.as_deref(), Some("subagent"));
    assert_eq!(child.agent_role.as_deref(), Some("asp_explorer"));
    assert_eq!(
        child.agent_path.as_deref(),
        Some("/Users/example/.codex/agents/asp-explorer.toml")
    );
    assert_eq!(child.spawn_depth, Some(1));
    assert_eq!(child.model.as_deref(), Some("gpt-5.4-mini"));
    assert_eq!(child.reasoning_effort.as_deref(), Some("low"));

    let nested = index
        .records
        .iter()
        .find(|record| record.session_id == nested_session_id)
        .expect("nested record");
    assert_eq!(nested.parent_thread_id.as_deref(), Some(child_session_id));
    assert_eq!(nested.agent_role.as_deref(), Some("asp_explorer"));
    assert_eq!(nested.spawn_depth, Some(2));

    assert_eq!(index.activity_by_session.len(), 3);
    let root_activity = index
        .activity_by_session
        .get(root_session_id)
        .expect("root activity");
    assert_eq!(root_activity.status, "active");
    assert_eq!(
        root_activity.last_running_session_id.as_deref(),
        Some(child_session_id)
    );
    assert!(!root_activity.running_session_closed);

    let child_activity = index
        .activity_by_session
        .get(child_session_id)
        .expect("child activity");
    assert_eq!(
        child_activity.last_running_session_id.as_deref(),
        Some(nested_session_id)
    );
    assert_eq!(child_activity.scanned_line_count, 3);

    let nested_activity = index
        .activity_by_session
        .get(nested_session_id)
        .expect("nested activity");
    assert_eq!(nested_activity.status, "closed");
    assert_eq!(
        nested_activity.current_turn_id.as_deref(),
        Some("nested-turn")
    );
    assert_eq!(
        nested_activity.last_terminal_event.as_deref(),
        Some("event_msg:closed")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn codex_rollout_index_joins_v2_parent_spawn_identity_into_sparse_child_metadata() {
    let root_session_id = "019f1f1a-5389-7223-a150-77dcb5ea8dd5";
    let child_session_id = "019f26e8-0dd6-71d3-8539-c362032b9e16";
    let root = std::env::temp_dir().join(format!(
        "asp-runtime-rollout-v2-spawn-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let _env = CodexHomeEnvGuard::set(&root);
    let rollout_dir = root.join("sessions").join("2026").join("07").join("14");
    fs::create_dir_all(&rollout_dir).expect("create rollout dir");

    let root_rollout = rollout_dir.join(format!("rollout-root-{root_session_id}.jsonl"));
    let root_session_meta = serde_json::json!({
        "type": "session_meta",
        "payload": {"id": root_session_id, "session_id": root_session_id}
    });
    let spawn_activity = serde_json::json!({
        "type": "event_msg",
        "payload": {
            "type": "item_completed",
            "item": {
                "type": "SubAgentActivity",
                "kind": "started",
                "agent_thread_id": child_session_id,
                "agent_path": "/root/asp_explorer"
            }
        }
    });
    let filler = serde_json::json!({
        "type": "response_item",
        "payload": {"type": "message", "role": "assistant", "content": []}
    })
    .to_string();
    let before_spawn = std::iter::repeat_n(&filler, 512)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    let after_spawn = std::iter::repeat_n(&filler, 8192)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        &root_rollout,
        format!("{root_session_meta}\n{before_spawn}\n{spawn_activity}\n{after_spawn}\n"),
    )
    .expect("write root rollout");

    let child_rollout = rollout_dir.join(format!("rollout-child-{child_session_id}.jsonl"));
    let child_session_meta = serde_json::json!({
        "type": "session_meta",
        "payload": {"id": child_session_id, "agent_role": "default"}
    });
    let child_turn = serde_json::json!({
        "type": "turn_context",
        "payload": {"model": "gpt-5.6-sol", "reasoning_effort": "xhigh"}
    });
    fs::write(
        &child_rollout,
        format!("{child_session_meta}\n{child_turn}\n"),
    )
    .expect("write child rollout");

    let index = codex_rollout_session_index(root_session_id)
        .expect("index lookup")
        .expect("index");
    let child = index
        .records
        .iter()
        .find(|record| record.session_id == child_session_id)
        .expect("joined child record");
    assert_eq!(child.root_session_id.as_deref(), Some(root_session_id));
    assert_eq!(child.parent_thread_id.as_deref(), Some(root_session_id));
    assert_eq!(child.thread_source.as_deref(), Some("subagent"));
    assert_eq!(child.spawn_depth, Some(1));
    assert_eq!(child.agent_role.as_deref(), Some("default"));
    assert_eq!(child.agent_path.as_deref(), Some("/root/asp_explorer"));
    assert_eq!(child.model.as_deref(), Some("gpt-5.6-sol"));
    assert_eq!(child.reasoning_effort.as_deref(), Some("xhigh"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn codex_rollout_index_recovers_child_from_root_attributed_session_meta_without_spawn_edge() {
    let root_session_id = "019f1f1a-5389-7223-a150-77dcb5ea8dd6";
    let child_session_id = "019f26e8-0dd6-71d3-8539-c362032b9e17";
    let root = std::env::temp_dir().join(format!(
        "asp-runtime-rollout-root-attribution-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let _env = CodexHomeEnvGuard::set(&root);
    let rollout_dir = root.join("sessions").join("2026").join("07").join("14");
    fs::create_dir_all(&rollout_dir).expect("create rollout dir");

    let root_rollout = rollout_dir.join(format!("rollout-root-{root_session_id}.jsonl"));
    fs::write(
        &root_rollout,
        format!(
            "{}\n",
            serde_json::json!({
                "type": "session_meta",
                "payload": {"id": root_session_id, "session_id": root_session_id}
            })
        ),
    )
    .expect("write root rollout");

    let child_rollout = rollout_dir.join(format!("rollout-child-{child_session_id}.jsonl"));
    let child_session_meta = serde_json::json!({
        "type": "session_meta",
        "payload": {
            "id": child_session_id,
            "session_id": root_session_id,
            "parent_thread_id": root_session_id,
            "thread_source": "subagent",
            "agent_role": "default",
            "source": {
                "subagent": {
                    "thread_spawn": {
                        "parent_thread_id": root_session_id,
                        "depth": 1,
                        "agent_path": "/root/asp_explorer"
                    }
                }
            }
        }
    });
    let child_turn = serde_json::json!({
        "type": "turn_context",
        "payload": {"model": "gpt-5.6-sol", "reasoning_effort": "xhigh"}
    });
    fs::write(
        &child_rollout,
        format!("{child_session_meta}\n{child_turn}\n"),
    )
    .expect("write child rollout");

    let index = codex_rollout_session_index(root_session_id)
        .expect("index lookup")
        .expect("root-attributed child index");
    let child = index
        .records
        .iter()
        .find(|record| record.session_id == child_session_id)
        .expect("root-attributed child record");
    assert_eq!(child.parent_thread_id.as_deref(), Some(root_session_id));
    assert_eq!(child.agent_role.as_deref(), Some("default"));
    assert_eq!(child.agent_path.as_deref(), Some("/root/asp_explorer"));
    assert_eq!(child.model.as_deref(), Some("gpt-5.6-sol"));
    assert_eq!(child.reasoning_effort.as_deref(), Some("xhigh"));

    let _ = fs::remove_dir_all(root);
}
