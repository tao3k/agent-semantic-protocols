// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::default_or_discovered_activation_path;

#[test]
fn codex_payload_cwd_owns_the_default_activation_identity() {
    let project_root = std::env::temp_dir().join("asp-hook-payload-cwd-authority");
    let payload = serde_json::json!({
        "cwd": project_root,
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {"cmd": "pwd"}
    });

    let actual = default_or_discovered_activation_path(&payload);
    let expected = agent_semantic_hook::default_activation_path(std::path::Path::new(
        payload["cwd"].as_str().expect("payload cwd"),
    ));

    assert_eq!(actual, expected);
}
