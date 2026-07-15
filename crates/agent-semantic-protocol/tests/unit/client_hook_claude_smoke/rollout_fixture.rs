use std::process::Command;

use serde_json::json;
use std::path::Path;

pub(super) fn write_codex_asp_explore_rollout(
    root: &Path,
    root_session_id: &str,
    child_session_id: &str,
    actual_model: &str,
) {
    let codex_home = root.join(".codex-home");
    let agents_dir = codex_home.join("agents");
    std::fs::create_dir_all(&agents_dir).expect("create test Codex agents dir");
    let agent_path = agents_dir.join("asp-explorer.toml");
    if !agent_path.is_file() {
        std::fs::write(
            &agent_path,
            "name = \"asp_explorer\"\nmodel = \"gpt-5.4-mini\"\nmodel_reasoning_effort = \"low\"\nsandbox_mode = \"read-only\"\n",
        )
        .expect("write test asp-explorer agent config");
    }

    let (root_rollout_dir_suffix, root_rollout_file_stamp) =
        codex_rollout_test_stamp(root_session_id);
    let root_rollout_dir = codex_home.join("sessions").join(root_rollout_dir_suffix);
    std::fs::create_dir_all(&root_rollout_dir).expect("create test Codex root sessions dir");
    let root_rollout_path = root_rollout_dir.join(format!(
        "rollout-{root_rollout_file_stamp}-{root_session_id}.jsonl"
    ));
    let root_session_meta = json!({
        "type": "session_meta",
        "payload": {
            "session_id": root_session_id,
            "id": root_session_id,
            "thread_source": "root"
        }
    });
    let child_spawn = json!({
        "type": "response_item",
        "payload": {
            "type": "thread_spawn",
            "id": child_session_id,
            "parent_thread_id": root_session_id,
            "agent_role": "asp_explorer",
            "agent_nickname": "ASP search",
            "agent_path": agent_path
        }
    });
    std::fs::write(
        root_rollout_path,
        format!("{root_session_meta}\n{child_spawn}\n"),
    )
    .expect("write test Codex root rollout");

    let (rollout_dir_suffix, rollout_file_stamp) = codex_rollout_test_stamp(child_session_id);
    let rollout_dir = codex_home.join("sessions").join(rollout_dir_suffix);
    std::fs::create_dir_all(&rollout_dir).expect("create test Codex child sessions dir");
    let rollout_path = rollout_dir.join(format!(
        "rollout-{rollout_file_stamp}-{child_session_id}.jsonl"
    ));
    let session_meta = json!({
        "type": "session_meta",
        "payload": {
            "session_id": root_session_id,
            "id": child_session_id,
            "parent_thread_id": root_session_id,
            "thread_source": "subagent",
            "agent_role": "asp_explorer",
            "agent_nickname": "ASP search",
            "source": {
                "subagent": {
                    "thread_spawn": {
                        "parent_thread_id": root_session_id,
                        "depth": 1,
                        "agent_role": "asp_explorer",
                        "agent_nickname": "ASP search",
                        "agent_path": agent_path
                    }
                }
            }
        }
    });
    let turn_context = json!({
        "type": "turn_context",
        "payload": {
            "model": actual_model,
            "reasoning_effort": "low",
            "sandbox_policy": {"type": "read-only"},
            "approval_policy": "never",
            "permission_profile": {"type": "disabled"}
        }
    });
    std::fs::write(rollout_path, format!("{session_meta}\n{turn_context}\n"))
        .expect("write test Codex rollout");
}

pub(super) fn write_codex_v2_asp_explorer_rollout(
    root: &Path,
    root_session_id: &str,
    child_session_id: &str,
) {
    write_codex_asp_explore_rollout(root, root_session_id, child_session_id, "gpt-5.4-mini");
    let (rollout_dir_suffix, rollout_file_stamp) = codex_rollout_test_stamp(child_session_id);
    let rollout_path = root
        .join(".codex-home")
        .join("sessions")
        .join(rollout_dir_suffix)
        .join(format!(
            "rollout-{rollout_file_stamp}-{child_session_id}.jsonl"
        ));
    let text = std::fs::read_to_string(&rollout_path).expect("read test Codex v2 rollout");
    let mut lines = text.lines();
    let mut session_meta: serde_json::Value =
        serde_json::from_str(lines.next().expect("session metadata line"))
            .expect("parse session metadata");
    session_meta["payload"]["agent_role"] = json!("default");
    session_meta["payload"]["source"]["subagent"]["thread_spawn"]["agent_role"] = json!("default");
    session_meta["payload"]["source"]["subagent"]["thread_spawn"]["agent_path"] =
        json!("/root/asp_explorer");
    let remaining = lines.collect::<Vec<_>>().join("\n");
    std::fs::write(rollout_path, format!("{session_meta}\n{remaining}\n"))
        .expect("write test Codex v2 rollout");
}

pub(super) fn append_codex_rollout_terminal_event(
    root: &Path,
    child_session_id: &str,
    terminal_event: &str,
) {
    let (rollout_dir_suffix, rollout_file_stamp) = codex_rollout_test_stamp(child_session_id);
    let rollout_path = root
        .join(".codex-home")
        .join("sessions")
        .join(rollout_dir_suffix)
        .join(format!(
            "rollout-{rollout_file_stamp}-{child_session_id}.jsonl"
        ));
    let event = json!({
        "timestamp": "2026-07-11T12:00:00.000Z",
        "type": "event_msg",
        "payload": {"type": terminal_event}
    });
    use std::io::Write as _;
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&rollout_path)
        .expect("open test Codex rollout for terminal event");
    writeln!(file, "{event}").expect("append test Codex terminal event");
}

pub(super) fn codex_rollout_test_stamp(session_id: &str) -> (String, String) {
    let seconds = uuid_v7_unix_seconds(session_id);
    let output = date_for_unix_seconds(seconds, "bsd")
        .or_else(|| date_for_unix_seconds(seconds, "gnu"))
        .expect("render test Codex rollout timestamp");
    let stamp = String::from_utf8(output.stdout).expect("date output must be utf-8");
    let (dir, file) = stamp.trim().split_once('|').expect("date output format");
    (dir.to_owned(), file.to_owned())
}

fn uuid_v7_unix_seconds(session_id: &str) -> u64 {
    let timestamp_hex: String = session_id
        .chars()
        .filter(|ch| *ch != '-')
        .take(12)
        .collect();
    u64::from_str_radix(&timestamp_hex, 16).expect("test session id must include uuid-v7 timestamp")
        / 1_000
}

fn date_for_unix_seconds(seconds: u64, style: &str) -> Option<std::process::Output> {
    let seconds = seconds.to_string();
    let mut command = Command::new("date");
    command.arg("-u");
    if style == "bsd" {
        command.args(["-r", &seconds]);
    } else {
        command.arg("-d").arg(format!("@{seconds}"));
    }
    command.arg("+%Y/%m/%d|%Y-%m-%dT%H-%M-%S");
    let output = command.output().ok()?;
    output.status.success().then_some(output)
}
