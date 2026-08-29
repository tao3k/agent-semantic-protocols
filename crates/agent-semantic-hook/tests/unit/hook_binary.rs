use super::{hook_event, panic_terminal};
use std::ffi::OsString;

#[test]
fn event_binding_accepts_plugin_and_direct_shapes() {
    assert_eq!(
        hook_event(&[OsString::from("hook"), OsString::from("pre-tool"),]),
        Some("pre-tool")
    );
    assert_eq!(
        hook_event(&[OsString::from("post-tool")]),
        Some("post-tool")
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
