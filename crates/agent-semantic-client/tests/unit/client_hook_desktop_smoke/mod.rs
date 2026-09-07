// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Mutex, MutexGuard, OnceLock},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde_json::{Value, json};

mod performance;

const HOOK_STDIN_PERFORMANCE_GATE: Duration = Duration::from_secs(2);

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
        assert_eq!(
            decision["routes"][0]["providerId"], "asp-rust",
            "Read alias must retain the provider-owned recovery route: {decision}"
        );
        assert_route_mentions(&decision, "src/lib.rs");
        assert_route_uses_native_syntax_playbook_recovery(&decision);
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
        assert_eq!(
            decision["routes"][0]["providerId"], "asp-rust",
            "shell Read wrapper must retain the provider-owned recovery route: {decision}"
        );
        assert_route_mentions(&decision, "src/lib.rs");
        assert_route_uses_native_syntax_playbook_recovery(&decision);
    }
}

#[test]
fn codex_desktop_write_aliases_require_semantic_ast_patch() {
    for tool_name in [
        "apply_patch",
        "functions.apply_patch",
        "Write",
        "writeFile",
        "FsWriteFile",
        "fs.writeFile",
    ] {
        let root = temp_project_root(&format!("codex-desktop-write-alias-{tool_name}"));
        write_hook_fixture(&root);

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
                .contains("asp ast-patch template"),
            "{tool_name}: {}",
            decision["message"].as_str().expect("decision message")
        );
        assert!(
            decision["message"]
                .as_str()
                .expect("decision message")
                .contains("asp rust ast-patch dry-run"),
            "{tool_name}: {}",
            decision["message"].as_str().expect("decision message")
        );
    }
}

fn write_hook_fixture(root: &Path) {
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::write(root.join("src/lib.rs"), "pub fn probe() {}\n").expect("write source");

    let state_home = crate::unit_state_home_fixture::default_state_home(root);
    crate::state_home_fixture::install_provider_script(&state_home, "rust", "#!/bin/sh\nexit 0\n");
    crate::state_home_fixture::write_activation(root, &state_home, &["rust"]);

    let agents = root.join("agents");
    let state_agents = state_home.join("agents");
    fs::create_dir_all(&agents).expect("create agent registry");
    fs::create_dir_all(&state_agents).expect("create state agent registry");
    for (name, bytes) in [
        (
            "config.toml",
            include_bytes!("../../../../../agents/config.toml").as_slice(),
        ),
        (
            "asp_explorer_codex.toml",
            include_bytes!("../../../../../agents/asp_explorer_codex.toml").as_slice(),
        ),
        (
            "asp_explorer_claude.md",
            include_bytes!("../../../../../agents/asp_explorer_claude.md").as_slice(),
        ),
        (
            "asp_testing_codex.toml",
            include_bytes!("../../../../../agents/asp_testing_codex.toml").as_slice(),
        ),
        (
            "asp_testing_claude.md",
            include_bytes!("../../../../../agents/asp_testing_claude.md").as_slice(),
        ),
    ] {
        fs::write(agents.join(name), bytes).expect("write agent registry fixture");
        fs::write(state_agents.join(name), bytes).expect("write state agent registry fixture");
    }

    fs::create_dir_all(root.join(".agent-semantic-protocols/hooks")).expect("create config dir");
    fs::write(
        root.join(".agent-semantic-protocols/hooks/config.toml"),
        agent_semantic_hook::default_client_config_template(),
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
    fs::create_dir_all(root.join(".git")).expect("create git marker");
    root
}

fn run_hook_decision(root: &Path, event: Value) -> Value {
    let _guard = hook_process_guard();
    let mut child = spawn_hook(root);

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
    decision_from_stdout(&stdout)
}

fn hook_process_guard() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn spawn_hook(root: &Path) -> std::process::Child {
    spawn_hook_event(root, "pre-tool")
}

fn spawn_hook_event(root: &Path, event: &str) -> std::process::Child {
    let asp = fresh_asp_binary();
    warm_asp_binary(&asp);
    Command::new(asp)
        .current_dir(root)
        .arg("hook")
        .arg(event)
        .arg("--client")
        .arg("codex")
        .arg("--activation")
        .arg(crate::state_home_fixture::canonical_activation_path(
            root,
            &crate::unit_state_home_fixture::default_state_home(root),
        ))
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env_remove("PRJ_CACHE_HOME")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn asp hook")
}

fn fresh_asp_binary() -> PathBuf {
    if let Ok(path) = env::var("ASP_TEST_ASP_BIN") {
        let path = PathBuf::from(path);
        assert_fresh_asp_binary(&path);
        return path;
    }
    let current_exe = env::current_exe().expect("current test exe");
    let debug_dir = current_exe
        .parent()
        .and_then(Path::parent)
        .expect("target/debug directory");
    let asp = debug_dir.join(format!("asp{}", env::consts::EXE_SUFFIX));
    if asp.is_file() {
        assert_fresh_asp_binary(&asp);
        return asp;
    }
    let cargo_bin = PathBuf::from(env!("CARGO_BIN_EXE_asp"));
    assert_fresh_asp_binary(&cargo_bin);
    cargo_bin
}

fn warm_asp_binary(binary: &Path) {
    static WARMED: OnceLock<()> = OnceLock::new();
    WARMED.get_or_init(|| {
        let output = Command::new(binary)
            .arg("hook")
            .arg("--help")
            .output()
            .expect("warm asp binary");
        assert!(
            output.status.success(),
            "failed to warm asp binary {}: stdout={} stderr={}",
            binary.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    });
}

fn assert_fresh_asp_binary(binary: &Path) {
    assert!(
        binary.is_file(),
        "asp binary does not exist: {}",
        binary.display()
    );
    let binary_mtime = binary
        .metadata()
        .and_then(|metadata| metadata.modified())
        .expect("asp binary mtime");
    for source in [
        "src/main.rs",
        "src/hook_bootstrap.rs",
        "src/command/hook_runtime.rs",
        "src/command/hook_runtime_stdin.rs",
        "../agent-semantic-hook/src/event_state.rs",
    ] {
        let source_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(source);
        let source_mtime = source_path
            .metadata()
            .and_then(|metadata| metadata.modified())
            .expect("hook source mtime");
        assert!(
            binary_mtime >= source_mtime,
            "asp binary {} is older than {}; rebuild the asp binary before running hook performance gates",
            binary.display(),
            source_path.display()
        );
    }
}

fn wait_for_hook_exit(mut child: std::process::Child, budget: Duration) -> (String, Duration) {
    let start = Instant::now();
    let pid = child.id();
    while start.elapsed() < budget {
        if child.try_wait().expect("poll hook child").is_some() {
            let output = child.wait_with_output().expect("collect hook output");
            assert!(
                output.status.success(),
                "hook command failed: stdout={} stderr={}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return (
                String::from_utf8(output.stdout).expect("utf8 stdout"),
                start.elapsed(),
            );
        }
        thread::sleep(Duration::from_millis(5));
    }

    let _ = child.kill();
    let output = child
        .wait_with_output()
        .expect("collect killed hook output");
    panic!(
        "hook command exceeded stdin performance gate {budget:?}: pid={pid} asp={} stdout={} stderr={}",
        fresh_asp_binary().display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn decision_from_stdout(stdout: &str) -> Value {
    let envelope: Value = serde_json::from_str(stdout.trim()).expect("hook json envelope");
    if let Some(decision) = envelope
        .get("agentHookDecision")
        .filter(|value| value.is_object())
    {
        return decision.clone();
    }
    let context = envelope["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap_or_else(|| panic!("additional context missing: {envelope}"));
    let decision_json = context
        .strip_prefix("[agent-hook-decision] ")
        .unwrap_or_else(|| panic!("decision prefix missing: {context}"));
    serde_json::from_str(decision_json).expect("decision json")
}

fn assert_route_mentions(decision: &Value, needle: &str) {
    let argv = decision["routes"][0]["argv"]
        .as_array()
        .expect("route argv");
    assert!(
        argv.iter().any(|arg| arg
            .as_str()
            .is_some_and(|arg| arg == needle || arg.starts_with(&format!("{needle}:")))),
        "route argv should mention {needle}: {argv:?}"
    );
}

fn assert_route_uses_native_syntax_playbook_recovery(decision: &Value) {
    let argv = decision["routes"][0]["argv"]
        .as_array()
        .expect("route argv");
    let args = argv
        .iter()
        .map(|arg| arg.as_str().expect("route arg string"))
        .collect::<Vec<_>>();
    assert!(
        args.windows(4)
            .any(|window| window == ["asp", "rust", "search", "playbook"]),
        "route argv should use search playbook recovery: {args:?}"
    );
    assert!(
        args.windows(2)
            .any(|window| window == ["--scope", "owner:src/lib.rs"]),
        "route argv should scope the native-syntax playbook to the owner: {args:?}"
    );
    assert!(
        !args.contains(&"--code"),
        "playbook recovery route must not request --code: {args:?}"
    );
}
