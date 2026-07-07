use std::path::Path;

pub(super) fn write_codex_asp_explorer_fixture(
    home: &Path,
    root_session_id: &str,
    child_session_id: &str,
    model: &str,
    sandbox: &str,
) {
    write_codex_asp_explorer_fixture_with_actual_sandbox(
        home,
        root_session_id,
        child_session_id,
        model,
        sandbox,
        sandbox,
    );
}

pub(super) fn write_codex_asp_explorer_fixture_with_actual_sandbox(
    home: &Path,
    root_session_id: &str,
    child_session_id: &str,
    model: &str,
    expected_sandbox: &str,
    actual_sandbox: &str,
) {
    write_codex_asp_explorer_fixture_with_actual_profile(
        home,
        root_session_id,
        child_session_id,
        model,
        model,
        expected_sandbox,
        actual_sandbox,
    );
}

pub(super) fn write_codex_asp_explorer_fixture_with_actual_profile(
    home: &Path,
    root_session_id: &str,
    child_session_id: &str,
    expected_model: &str,
    actual_model: &str,
    expected_sandbox: &str,
    actual_sandbox: &str,
) {
    write_codex_asp_explorer_fixture_with_actual_agent_path(
        home,
        root_session_id,
        child_session_id,
        expected_model,
        actual_model,
        expected_sandbox,
        actual_sandbox,
        None,
    );
}

pub(super) fn write_codex_asp_explorer_fixture_with_actual_agent_path(
    home: &Path,
    root_session_id: &str,
    child_session_id: &str,
    expected_model: &str,
    actual_model: &str,
    expected_sandbox: &str,
    actual_sandbox: &str,
    actual_agent_path: Option<&Path>,
) {
    write_codex_asp_explorer_fixture_with_agent_path_presence(
        home,
        root_session_id,
        child_session_id,
        expected_model,
        actual_model,
        expected_sandbox,
        actual_sandbox,
        actual_agent_path,
        true,
    );
}

pub(super) fn write_codex_asp_explorer_fixture_without_agent_path(
    home: &Path,
    root_session_id: &str,
    child_session_id: &str,
    expected_model: &str,
    actual_model: &str,
    expected_sandbox: &str,
    actual_sandbox: &str,
) {
    write_codex_asp_explorer_fixture_with_agent_path_presence(
        home,
        root_session_id,
        child_session_id,
        expected_model,
        actual_model,
        expected_sandbox,
        actual_sandbox,
        None,
        false,
    );
}

fn write_codex_asp_explorer_fixture_with_agent_path_presence(
    home: &Path,
    root_session_id: &str,
    child_session_id: &str,
    expected_model: &str,
    actual_model: &str,
    expected_sandbox: &str,
    actual_sandbox: &str,
    actual_agent_path: Option<&Path>,
    include_agent_path: bool,
) {
    let agents_dir = home.join(".codex").join("agents");
    std::fs::create_dir_all(&agents_dir).expect("create codex agents dir");
    let expected_agent_path = agents_dir.join("asp-explorer.toml");
    std::fs::write(
        &expected_agent_path,
        format!(
            "name = \"asp_explorer\"\nmodel = \"{expected_model}\"\nsandbox_mode = \"{expected_sandbox}\"\n"
        ),
    )
    .expect("write asp explorer config");
    let rollout_agent_path = actual_agent_path
        .unwrap_or(expected_agent_path.as_path())
        .display()
        .to_string();

    let sessions_dir = home.join(".codex").join("sessions");
    let rollout_dir = sessions_dir.join("2026").join("07").join("01");
    std::fs::create_dir_all(&rollout_dir).expect("create codex rollout dir");
    std::fs::create_dir_all(&sessions_dir).expect("create codex sessions dir");
    let root_rollout_path =
        rollout_dir.join(format!("rollout-2026-07-01T00-00-00-{root_session_id}.jsonl"));
    let root_session_meta = serde_json::json!({
        "type": "session_meta",
        "payload": {
            "session_id": root_session_id,
            "id": root_session_id
        }
    });
    let child_spawn_output =
        serde_json::json!({"agent_id": child_session_id, "nickname": "ASP search"});
    let child_spawn = serde_json::json!({
        "type": "response_item",
        "payload": {
            "type": "function_call_output",
            "output": child_spawn_output.to_string()
        }
    });
    let root_rollout = format!("{root_session_meta}\n{child_spawn}\n");
    std::fs::write(&root_rollout_path, &root_rollout).expect("write root codex rollout");
    std::fs::write(
        sessions_dir.join(format!(
            "rollout-2026-07-01T00-00-00-{root_session_id}.jsonl"
        )),
        &root_rollout,
    )
    .expect("write root codex rollout root index");
    let rollout_path =
        rollout_dir.join(format!("rollout-2026-07-01T00-00-00-{child_session_id}.jsonl"));
    let mut session_meta = serde_json::json!({
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
                        "agent_nickname": "ASP search"
                    }
                }
            }
        }
    });
    if include_agent_path {
        session_meta["payload"]["source"]["subagent"]["thread_spawn"]["agent_path"] =
            serde_json::json!(rollout_agent_path);
    }
    let turn_context = serde_json::json!({
        "type": "turn_context",
        "payload": {
            "model": actual_model,
            "sandbox_policy": {"type": actual_sandbox},
            "approval_policy": "never",
            "permission_profile": {"type": "disabled"}
        }
    });
    let child_rollout = format!("{session_meta}\n{turn_context}\n");
    std::fs::write(rollout_path, &child_rollout).expect("write codex rollout");
    std::fs::write(
        sessions_dir.join(format!(
            "rollout-2026-07-01T00-00-00-{child_session_id}.jsonl"
        )),
        &child_rollout,
    )
    .expect("write codex rollout root index");
}
