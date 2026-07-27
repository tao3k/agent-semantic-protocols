use super::{
    render_codex_plugin_hooks_json, validate_codex_plugin_hooks_manifest,
    ASP_CODEX_PLUGIN_HOOKS_JSON,
};

#[test]
fn rendered_codex_hooks_keep_every_event_on_the_plugin_path_command() {
    let rendered = render_codex_plugin_hooks_json().expect("render Codex hooks");
    let hooks: serde_json::Value = serde_json::from_str(&rendered).expect("decode Codex hooks");
    let events = hooks["hooks"].as_object().expect("hook event map");
    let mut command_count = 0;
    let mut pre_tool_count = 0;

    for (event, handlers) in events {
        for handler in handlers.as_array().expect("hook handlers") {
            for hook in handler["hooks"].as_array().expect("hook commands") {
                let command = hook["command"].as_str().expect("hook command");
                assert!(
                    command.starts_with("asp hook "),
                    "plugin hook command escaped the plugin PATH contract: {command}"
                );
                if event == "PreToolUse" {
                    assert_eq!(command, "asp hook pre-tool --client codex");
                    pre_tool_count += 1;
                }
                command_count += 1;
            }
        }
    }
    assert!(command_count > 0, "Codex plugin must define hook commands");
    assert_eq!(
        pre_tool_count, 1,
        "Codex plugin must define one pre-tool hook"
    );
}

#[test]
fn codex_hook_manifest_gate_rejects_absolute_binary_commands() {
    let mut hooks: serde_json::Value =
        serde_json::from_str(ASP_CODEX_PLUGIN_HOOKS_JSON).expect("decode canonical Codex hooks");
    hooks["hooks"]["PreToolUse"][0]["hooks"][0]["command"] = serde_json::Value::String(
        "/Users/example/.agent-semantic-protocols/runtime/bin/asp hook pre-tool --client codex"
            .to_string(),
    );

    let error = validate_codex_plugin_hooks_manifest(&hooks)
        .expect_err("absolute command must be rejected");
    assert!(
        error.contains("asp hook pre-tool --client codex"),
        "schema gate must identify the required bare plugin PATH command: {error}"
    );
}

use crate::command::hook_runtime::hook_runtime_codex_plugin::{
    codex_plugin_source_root, write_codex_plugin_file,
};

#[test]
fn unchanged_codex_plugin_file_is_a_zero_write_noop() {
    let temp = unique_plugin_test_directory("zero-write");
    let path = temp.join("hooks").join("hooks.json");
    write_codex_plugin_file(&path, "{\"version\": 1}").expect("write initial plugin file");

    let mut permissions = std::fs::metadata(&path)
        .expect("read initial plugin metadata")
        .permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&path, permissions).expect("make plugin file read-only");

    write_codex_plugin_file(&path, "{\"version\": 1}")
        .expect("unchanged plugin content must not attempt a write");
    assert_eq!(
        std::fs::read_to_string(&path).expect("read unchanged plugin file"),
        "{\"version\": 1}\n"
    );
    std::fs::remove_dir_all(&temp).expect("remove plugin test directory");
}

#[test]
fn plugin_source_root_does_not_escape_the_explicit_project_root() {
    let temp = unique_plugin_test_directory("explicit-root");
    let error = codex_plugin_source_root(&temp)
        .expect_err("an invalid explicit project root must not fall back to cwd or build source");
    assert!(error.contains("is not an ASP Codex plugin marketplace source root"));
    std::fs::remove_dir_all(&temp).expect("remove plugin test directory");
}

fn unique_plugin_test_directory(label: &str) -> std::path::PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "asp-codex-plugin-{label}-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(&path).expect("create plugin test directory");
    path
}

#[test]
fn codex_hook_manifest_gate_rejects_unknown_events_and_fields() {
    let canonical: serde_json::Value =
        serde_json::from_str(ASP_CODEX_PLUGIN_HOOKS_JSON).expect("decode canonical Codex hooks");

    let mut unknown_event = canonical.clone();
    unknown_event["hooks"]["BeforeToolUse"] = unknown_event["hooks"]["PreToolUse"].clone();
    let error = validate_codex_plugin_hooks_manifest(&unknown_event)
        .expect_err("unknown event must be rejected");
    assert!(!error.is_empty(), "gate must reject schema drift: {error}");

    let mut unknown_field = canonical;
    unknown_field["hooks"]["PreToolUse"][0]["unexpectedField"] =
        serde_json::Value::String("*".to_string());
    let error = validate_codex_plugin_hooks_manifest(&unknown_field)
        .expect_err("unknown handler field must be rejected");
    assert!(
        !error.is_empty(),
        "gate must reject unknown handler fields: {error}"
    );
}

#[test]
fn codex_hook_schema_document_is_v1_and_machine_readable() {
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/codex-plugin-hooks.v1.schema.json"
    ))
    .expect("decode Codex hook schema");
    assert_eq!(
        schema["$id"],
        "https://agent-semantic-protocols.dev/schemas/codex-plugin-hooks.v1.schema.json"
    );
    assert_eq!(schema["additionalProperties"], false);
}
