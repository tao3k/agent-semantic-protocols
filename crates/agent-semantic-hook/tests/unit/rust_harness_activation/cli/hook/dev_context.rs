use std::fs;
use std::io::Write;
use std::process::Stdio;

use serde_json::{Value, json};

use crate::rust_harness_activation::support::{
    asp_command, root_owned_rust_activation_json, temp_project_root,
};

#[test]
fn cli_hook_records_dev_context_when_agents_asp_toml_enables_develop_mode() {
    let root = temp_project_root("hook-dev-context-asp-config");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"hook-dev-context-test\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
    )
    .expect("write project anchor");
    let agents_dir = root.join("agents");
    fs::create_dir_all(&agents_dir).expect("create agents config dir");
    fs::write(agents_dir.join("asp.toml"), "develop_mode = true\n").expect("write asp config");
    let activation_path = root.join("activation.json");
    fs::write(&activation_path, root_owned_rust_activation_json()).expect("write activation");
    let trace_dir = root.join("trace");

    let mut child = asp_command()
        .current_dir(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .env("SEMANTIC_PROTOCOL_TRACE_DIR", &trace_dir)
        .args([
            "hook",
            "--client",
            "codex",
            "pre-tool",
            "--emit",
            "decision",
            "--activation",
            activation_path.to_str().expect("utf8 activation path"),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run asp hook");
    child
        .stdin
        .as_mut()
        .expect("hook stdin")
        .write_all(
            json!({
                "tool_name": "functions.exec_command",
                "tool_input": {"cmd": "sed -n '1,40p' src/lib.rs"}
            })
            .to_string()
            .as_bytes(),
        )
        .expect("write hook payload");
    let output = child.wait_with_output().expect("wait for hook command");
    assert!(
        output.status.success(),
        "hook stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let decision: Value = serde_json::from_slice(&output.stdout).expect("hook decision JSON");
    assert_eq!(decision["decision"], "deny");

    let marker_paths = fs::read_dir(trace_dir.join("dev-context"))
        .expect("read dev-context dir")
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .expect("read dev-context entries");
    assert_eq!(marker_paths.len(), 1);
    let marker: Value =
        serde_json::from_slice(&fs::read(&marker_paths[0]).expect("read dev-context marker"))
            .expect("dev-context marker JSON");
    assert_eq!(
        marker["schemaId"],
        "agent.semantic-protocols.dev-active-context"
    );
    assert_eq!(marker["event"], "pre-tool");
    assert_eq!(
        marker["projectRoot"],
        fs::canonicalize(&root)
            .expect("canonical root")
            .display()
            .to_string()
    );
}
