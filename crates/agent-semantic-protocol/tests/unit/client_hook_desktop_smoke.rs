use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::{Value, json};

#[test]
fn codex_desktop_read_aliases_reach_runtime_policy() {
    let root = temp_project_root("codex-desktop-read-aliases");
    write_hook_fixture(&root);

    for tool_name in [
        "Read",
        "readFile",
        "FsReadFile",
        "fs/readFile",
        "fs.readFile",
        "mcp__filesystem__read_file",
    ] {
        let decision = run_hook_decision(
            &root,
            json!({
                "tool_name": tool_name,
                "tool_input": {
                    "path": "src/lib.rs"
                }
            }),
        );

        assert_eq!(decision["decision"], "deny", "{tool_name}");
        assert_eq!(decision["reasonKind"], "direct-source-read", "{tool_name}");
        assert_eq!(decision["routes"][0]["providerId"], "rs-harness");
        assert_route_mentions(&decision, "src/lib.rs");
    }
}

#[test]
fn codex_desktop_shell_read_wrappers_reach_runtime_policy() {
    let root = temp_project_root("codex-desktop-shell-read-wrappers");
    write_hook_fixture(&root);

    for command in [
        r#"node -e "const fs=require('fs'); console.log(fs.readFileSync('src/lib.rs','utf8'))""#,
        "python3 - <<'PY'\nfrom pathlib import Path\nprint(Path('src/lib.rs').read_text())\nPY",
    ] {
        let decision = run_hook_decision(
            &root,
            json!({
                "tool_name": "functions.exec_command",
                "tool_input": {
                    "cmd": command
                }
            }),
        );

        assert_eq!(decision["decision"], "deny", "{command}");
        assert_eq!(decision["reasonKind"], "bulk-source-dump", "{command}");
        assert_eq!(decision["routes"][0]["providerId"], "rs-harness");
        assert_route_mentions(&decision, "src/lib.rs");
    }
}

#[test]
fn codex_desktop_write_aliases_require_semantic_ast_patch() {
    let root = temp_project_root("codex-desktop-write-aliases");
    write_hook_fixture(&root);

    for tool_name in [
        "apply_patch",
        "functions.apply_patch",
        "Write",
        "writeFile",
        "FsWriteFile",
        "fs.writeFile",
    ] {
        let decision = run_hook_decision(
            &root,
            json!({
                "tool_name": tool_name,
                "tool_input": {
                    "path": "src/lib.rs",
                    "content": "pub fn replacement() {}\n"
                }
            }),
        );

        assert_eq!(decision["decision"], "deny", "{tool_name}");
        assert_eq!(
            decision["reasonKind"], "semantic-ast-patch-required",
            "{tool_name}"
        );
        assert!(
            decision["message"]
                .as_str()
                .expect("decision message")
                .contains("asp ast-patch template")
        );
        assert!(
            decision["message"]
                .as_str()
                .expect("decision message")
                .contains("asp rust ast-patch dry-run")
        );
    }
}

fn write_hook_fixture(root: &Path) {
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::write(root.join("src/lib.rs"), "pub fn probe() {}\n").expect("write source");

    fs::create_dir_all(root.join(".cache/agent-semantic-protocol/hooks"))
        .expect("create activation dir");
    fs::write(
        root.join(".cache/agent-semantic-protocol/hooks/activation.json"),
        root_owned_rust_activation_json(),
    )
    .expect("write activation");

    fs::create_dir_all(root.join(".codex/agent-semantic-protocol/hooks"))
        .expect("create config dir");
    fs::write(
        root.join(".codex/agent-semantic-protocol/hooks/config.toml"),
        CLIENT_CONFIG,
    )
    .expect("write client config");
}

fn temp_project_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = env::temp_dir().join(format!("asp-{label}-{unique}"));
    fs::create_dir_all(&root).expect("create temp root");
    root
}

fn run_hook_decision(root: &Path, event: Value) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(root)
        .arg("hook")
        .arg("pre-tool")
        .arg("--client")
        .arg("codex")
        .arg("--event-json")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn asp hook");

    child
        .stdin
        .as_mut()
        .expect("hook stdin")
        .write_all(event.to_string().as_bytes())
        .expect("write hook event");

    let output = child.wait_with_output().expect("run asp hook");

    assert!(
        output.status.success(),
        "hook command failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let envelope: Value = serde_json::from_str(stdout.trim()).expect("hook json envelope");
    let context = envelope["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("additional context");
    let decision_json = context
        .strip_prefix("[agent-hook-decision] ")
        .expect("decision prefix");
    serde_json::from_str(decision_json).expect("decision json")
}

fn assert_route_mentions(decision: &Value, needle: &str) {
    let argv = decision["routes"][0]["argv"]
        .as_array()
        .expect("route argv");
    assert!(
        argv.iter().any(|arg| arg.as_str() == Some(needle)),
        "route argv should mention {needle}: {argv:?}"
    );
}

fn root_owned_rust_activation_json() -> &'static str {
    r#"{
  "schemaId": "agent.semantic-protocols.hook.activation",
  "schemaVersion": "1",
  "protocolId": "agent.semantic-protocols.hook",
  "protocolVersion": "1",
  "projectRoot": ".",
  "generatedBy": {
    "runtime": "agent-semantic-hook",
    "version": "0.1.0"
  },
  "providers": [
    {
      "manifestId": "agent.semantic-protocols.providers.rust.rs-harness",
      "manifestDigest": "sha256:b7d08e7410b0034e70bb5101964613c78865c50bfe310d0a1827ae78a660db3c",
      "languageId": "rust",
      "providerId": "rs-harness",
      "binary": "rs-harness",
      "providerCommandPrefix": [],
      "coverage": {
        "packageRoots": ["."],
        "sourceRoots": ["src", "tests"],
        "configFiles": ["Cargo.toml"],
        "sourceExtensions": [".rs"],
        "ignoredPathPrefixes": ["target"]
      }
    }
  ]
}
"#
}

const CLIENT_CONFIG: &str = r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"
"#;
