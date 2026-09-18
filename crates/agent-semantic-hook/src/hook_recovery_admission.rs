// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Parser-backed admission for the minimal Hook recovery command set.

/// Return whether a Hook event contains one canonical recovery command.
pub fn canonical_recovery_admission(event: Option<&str>, input: &[u8]) -> bool {
    if !matches!(event, Some("pre-tool" | "permission-request")) {
        return false;
    }
    let Ok(payload) = serde_json::from_slice::<serde_json::Value>(input) else {
        return false;
    };
    let Some(command) = hook_payload_command(&payload) else {
        return false;
    };
    let Ok(stages) = agent_semantic_shell_parser::parse_bash_command_candidates(command) else {
        return false;
    };
    if stages.len() != 1 {
        return false;
    }
    let words = stages[0].words();
    let Some(asp_index) = words.iter().position(|word| {
        word.rsplit(['/', '\\'])
            .next()
            .is_some_and(|name| name == "asp")
    }) else {
        return false;
    };
    let trusted_prefix = asp_index == 0
        || (asp_index == 3
            && words[0].rsplit(['/', '\\']).next() == Some("direnv")
            && words[1] == "exec"
            && words[2] == ".");
    if !trusted_prefix {
        return false;
    }
    let exact_server_control = words.get(asp_index + 1).map(String::as_str) == Some("server")
        && matches!(
            words.get(asp_index + 2).map(String::as_str),
            Some("status" | "start" | "restart")
        )
        && words.len() == asp_index + 3;
    if exact_server_control {
        return true;
    }
    let exact_hook_doctor = words.get(asp_index + 1).map(String::as_str) == Some("hook")
        && words.get(asp_index + 2).map(String::as_str) == Some("doctor")
        && words.get(asp_index + 3).map(String::as_str) == Some("--client")
        && words.get(asp_index + 4).map(String::as_str) == Some("codex")
        && words.len() == asp_index + 5;
    if exact_hook_doctor {
        return true;
    }
    if agent_semantic_runtime::resolve_state_home().is_err() {
        return false;
    }
    exact_canonical_binary_install(words, asp_index)
}

fn exact_canonical_binary_install(words: &[String], asp_index: usize) -> bool {
    if words.get(asp_index + 1).map(String::as_str) != Some("install")
        || words.get(asp_index + 2).map(String::as_str) != Some("binary")
    {
        return false;
    }
    if words.len() == asp_index + 3 {
        return true;
    }
    let Some(target) = words.get(asp_index + 4) else {
        return false;
    };
    words.get(asp_index + 3).map(String::as_str) == Some("--target")
        && words.len() == asp_index + 5
        && agent_semantic_runtime::resolve_state_home()
            .map(|state_home| {
                agent_semantic_artifacts::StateHomeLayout::new(state_home)
                    .runtime_state()
                    .bin()
                    .join("asp")
            })
            .is_ok_and(|canonical| canonical == std::path::Path::new(target))
}

fn hook_payload_command(payload: &serde_json::Value) -> Option<&str> {
    let tool_input = payload
        .get("tool_input")
        .or_else(|| payload.get("toolInput"));
    tool_input
        .and_then(|input| input.get("cmd").or_else(|| input.get("command")))
        .and_then(serde_json::Value::as_str)
        .or_else(|| payload.get("command").and_then(serde_json::Value::as_str))
}
