use super::render_codex_plugin_hooks_json;

#[test]
fn rendered_codex_hooks_bind_every_event_to_canonical_absolute_asp_binary() {
    let asp_binary = std::path::Path::new("/tmp/asp state/runtime/bin/asp");
    let rendered = render_codex_plugin_hooks_json(asp_binary).expect("render Codex hooks");
    let hooks: serde_json::Value = serde_json::from_str(&rendered).expect("decode Codex hooks");
    let events = hooks["hooks"].as_object().expect("hook event map");
    let expected_prefix = "'/tmp/asp state/runtime/bin/asp' hook ";
    let mut command_count = 0;

    for handlers in events.values() {
        for handler in handlers.as_array().expect("hook handlers") {
            for hook in handler["hooks"].as_array().expect("hook commands") {
                let command = hook["command"].as_str().expect("hook command");
                assert!(
                    command.starts_with(expected_prefix),
                    "hook command escaped canonical ASP binary: {command}"
                );
                assert!(
                    !command.starts_with("asp hook "),
                    "bare ASP command reintroduced PATH-dependent routing: {command}"
                );
                command_count += 1;
            }
        }
    }
    assert!(command_count > 0, "Codex plugin must define hook commands");
}
