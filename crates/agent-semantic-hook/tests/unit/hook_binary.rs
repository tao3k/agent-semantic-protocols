// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::hook_event;
use super::panic_terminal;
use std::ffi::OsString;

#[test]
fn event_binding_accepts_only_the_direct_hook_binary_shape() {
    assert_eq!(hook_event(&[OsString::from("pre-tool")]), Some("pre-tool"));
    assert_eq!(
        hook_event(&[OsString::from("post-tool")]),
        Some("post-tool")
    );
    assert_eq!(
        hook_event(&[OsString::from("hook"), OsString::from("pre-tool")]),
        None,
        "the standalone asp-hook binary must not recreate an `asp hook` namespace"
    );
    assert_eq!(hook_event(&[OsString::from("--version")]), None);
}

#[test]
fn panic_terminals_are_valid_codex_event_envelopes() {
    let pre_tool = panic_terminal(Some("pre-tool"), "boom".to_owned());
    assert_eq!(pre_tool["hookSpecificOutput"]["permissionDecision"], "deny");
    let permission = panic_terminal(Some("permission-request"), "boom".to_owned());
    assert_eq!(
        permission["hookSpecificOutput"]["decision"]["behavior"],
        "deny"
    );
    assert_eq!(
        panic_terminal(Some("post-tool"), "boom".to_owned()),
        serde_json::json!({})
    );
}
