use std::os::unix::fs::PermissionsExt as _;
use std::process::{Command, Stdio};

const LAUNCHER: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../asp-codex-plugin/bin/asp-hook"
);

#[test]
fn plugin_launcher_is_policy_free_and_exec_only() {
    let launcher = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../asp-codex-plugin/bin/asp-hook"
    ));
    assert!(launcher.contains("hooks/bin/asp-hook-evaluator"));
    assert!(launcher.contains("exec \"$asp_hook_state_home/hooks/bin/asp-hook-evaluator\" \"$@\""));
    for forbidden in [
        "hooks/current",
        "jq ",
        "decision",
        "ASP_NO_AGENT",
        "compiled-hook-generation",
    ] {
        assert!(
            !launcher.contains(forbidden),
            "launcher contains policy token {forbidden}"
        );
    }
}

#[test]
fn launcher_executes_current_evaluator_without_runtime_asp() {
    let state_home = tempfile::tempdir().expect("state home");
    let bin = state_home.path().join("hooks/bin");
    std::fs::create_dir_all(&bin).expect("Hook bin directory");
    let evaluator = bin.join("asp-hook-evaluator");
    std::fs::write(
        &evaluator,
        "#!/bin/sh\nprintf '%s\\n' \"$*\"\nprintf '%s\\n' \"$(cat)\"\n",
    )
    .expect("fake evaluator");
    let mut permissions = std::fs::metadata(&evaluator)
        .expect("evaluator metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&evaluator, permissions).expect("executable evaluator");
    let mut child = Command::new(LAUNCHER)
        .env_remove("ASP_NO_AGENT")
        .env("ASP_STATE_HOME", state_home.path())
        .env("PATH", "/usr/bin:/bin")
        .args(["pre-tool", "--client", "codex", "--host-match", "Bash"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("launch plugin Hook");
    use std::io::Write as _;
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(br#"{"tool_name":"Bash","tool_input":{}}"#)
        .expect("write payload");
    let output = child.wait_with_output().expect("wait for launcher");
    assert!(
        output.status.success(),
        "launcher failed: status={} stdout={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 evaluator output");
    assert!(stdout.contains("pre-tool --client codex --host-match Bash"));
    assert!(stdout.contains(r#"{"tool_name":"Bash","tool_input":{}}"#));
}

#[test]
fn evaluator_missing_current_is_typed_fail_closed() {
    let state_home = tempfile::tempdir().expect("state home");
    let mut child = Command::new(env!("CARGO_BIN_EXE_asp-hook-evaluator"))
        .env_remove("ASP_NO_AGENT")
        .env("ASP_STATE_HOME", state_home.path())
        .args(["pre-tool", "--client", "codex", "--host-match", "Bash"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("launch evaluator");
    use std::io::Write as _;
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(br#"{"tool_name":"Bash","tool_input":{}}"#)
        .expect("write payload");
    let output = child.wait_with_output().expect("wait for evaluator");
    assert!(!output.status.success());
    let receipt: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("typed failure receipt");
    assert_eq!(receipt["decision"], "deny");
    assert_eq!(receipt["reasonKind"], "hook-generation-unavailable");
    assert_eq!(receipt["terminal"], "hook-generation-unavailable");
    assert_eq!(receipt["processLaunched"], false);
}

#[test]
fn permission_request_is_typed_deny_before_current_load() {
    let state_home = tempfile::tempdir().expect("state home");
    let output = Command::new(env!("CARGO_BIN_EXE_asp-hook-evaluator"))
        .env_remove("ASP_NO_AGENT")
        .env("ASP_STATE_HOME", state_home.path())
        .arg("permission-request")
        .output()
        .expect("launch evaluator");
    assert!(
        output.status.success(),
        "status={} stdout={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("typed permission receipt");
    assert_eq!(receipt["decision"], "deny");
    assert_eq!(receipt["reasonKind"], "permission-request-denied");
    assert_eq!(receipt["processLaunched"], false);
}

#[test]
fn evaluator_binds_generation_from_its_resolved_immutable_candidate() {
    use std::os::unix::fs::symlink;

    let state_home = tempfile::tempdir().expect("state home");
    let hooks = state_home.path().join("hooks");
    let candidate = hooks.join("candidates/g1");
    let bin = hooks.join("bin");
    std::fs::create_dir_all(&candidate).expect("candidate");
    std::fs::create_dir_all(&bin).expect("stable bin");
    let evaluator = candidate.join("asp-hook-evaluator");
    std::fs::copy(env!("CARGO_BIN_EXE_asp-hook-evaluator"), &evaluator)
        .expect("copy evaluator");
    let generation = br#"{
      "schemaId":"agent.semantic-protocols.hook-generation",
      "schemaVersion":1,
      "generationDigest":"blake3-256:g1",
      "rules":[{
        "id":"route-source","matchers":["Bash"],
        "registeredExtensions":["rs"],"decision":"deny",
        "reasonKind":"registered-source-route-required",
        "message":"Use ASP."
      }]
    }"#;
    std::fs::write(candidate.join("compiled-hook-generation.json"), generation)
        .expect("generation bytes");
    let candidate_g2 = hooks.join("candidates/g2");
    std::fs::create_dir_all(&candidate_g2).expect("second candidate");
    std::fs::copy(
        env!("CARGO_BIN_EXE_asp-hook-evaluator"),
        candidate_g2.join("asp-hook-evaluator"),
    )
    .expect("copy second evaluator");
    std::fs::write(
        candidate_g2.join("compiled-hook-generation.json"),
        String::from_utf8(generation.to_vec())
            .expect("UTF-8 generation")
            .replace("blake3-256:g1", "blake3-256:g2")
            .replace("\"id\":\"route-source\"", "\"id\":\"route-source-g2\"")
            .replace("Use ASP.", "Use ASP g2."),
    )
    .expect("second generation bytes");
    symlink("candidates/g1", hooks.join("current")).expect("current generation");
    symlink("../current/asp-hook-evaluator", bin.join("asp-hook-evaluator"))
        .expect("stable evaluator");
    let mut child = Command::new(LAUNCHER)
        .env_remove("ASP_NO_AGENT")
        .env("ASP_STATE_HOME", state_home.path())
        .args(["pre-tool", "--host-match", "Bash"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("launch evaluator");
    symlink("candidates/g2", hooks.join(".current-next")).expect("next current");
    std::fs::rename(hooks.join(".current-next"), hooks.join("current"))
        .expect("switch current generation");
    use std::io::Write as _;
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(br#"{"tool_name":"Bash","tool_input":{"command":"unknown src/lib.rs"}}"#)
        .expect("write payload");
    let output = child.wait_with_output().expect("wait evaluator");
    assert!(
        output.status.success(),
        "generation-bound evaluator failed: status={} stdout={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("typed decision receipt");
    assert_eq!(receipt["decision"], "deny");
    assert_eq!(receipt["subject"], "src/lib.rs");
    match receipt["generationDigest"].as_str() {
        Some("blake3-256:g1") => {
            assert_eq!(receipt["configRuleId"], "route-source");
            assert_eq!(receipt["message"], "Use ASP.");
        }
        Some("blake3-256:g2") => {
            assert_eq!(receipt["configRuleId"], "route-source-g2");
            assert_eq!(receipt["message"], "Use ASP g2.");
        }
        other => panic!("launcher resolved an unexpected HookGeneration: {other:?}"),
    }
}

#[tokio::test]
async fn plugin_launcher_cold_decisions_are_bounded_and_never_probe() {
    use std::os::unix::fs::symlink;
    use std::time::{Duration, Instant};
    use tokio::io::AsyncWriteExt as _;

    const SAMPLES: usize = 64;
    const INVOCATION_TIMEOUT: Duration = Duration::from_secs(1);
    let state_home = tempfile::tempdir().expect("state home");
    let candidate = state_home.path().join("hooks/candidates/perf");
    std::fs::create_dir_all(&candidate).expect("candidate directory");
    let evaluator = candidate.join("asp-hook-evaluator");
    std::fs::copy(env!("CARGO_BIN_EXE_asp-hook-evaluator"), &evaluator)
        .expect("candidate evaluator");
    std::fs::set_permissions(&evaluator, std::fs::Permissions::from_mode(0o755))
        .expect("candidate evaluator mode");
    std::fs::write(
        candidate.join("compiled-hook-generation.json"),
        br#"{"schemaId":"agent.semantic-protocols.hook-generation","schemaVersion":1,"generationDigest":"blake3-256:perf","rules":[{"id":"route-source","matchers":["Bash"],"registeredExtensions":["rs"],"decision":"deny","reasonKind":"registered-source-route-required","message":"Use ASP."}]}"#,
    )
    .expect("compiled generation");
    let hooks = state_home.path().join("hooks");
    symlink("candidates/perf", hooks.join("current")).expect("current generation");
    std::fs::create_dir_all(hooks.join("bin")).expect("stable bin directory");
    symlink("../current/asp-hook-evaluator", hooks.join("bin/asp-hook-evaluator"))
        .expect("stable evaluator");
    let payload = br#"{"session_id":"perf","cwd":".","hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"opaque-reader src/lib.rs"}}"#;
    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        let mut child = tokio::process::Command::new(LAUNCHER)
            .env_remove("ASP_NO_AGENT")
            .env("ASP_STATE_HOME", state_home.path())
            .args(["pre-tool", "--host-match", "Bash"])
            .kill_on_drop(true)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn plugin launcher");
        let mut stdin = child.stdin.take().expect("launcher stdin");
        stdin.write_all(payload).await.expect("write launcher payload");
        stdin.shutdown().await.expect("close launcher stdin");
        drop(stdin);
        let output = tokio::time::timeout(INVOCATION_TIMEOUT, child.wait_with_output())
            .await
            .expect("launcher invocation deadline")
            .expect("launcher output");
        let elapsed = started.elapsed();
        assert!(
            output.status.success(),
            "launcher failed: status={} stdout={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let receipt: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("launcher receipt");
        assert_eq!(receipt["probeProcessLaunched"], false);
        assert_eq!(receipt["elapsedMicros"], 0);
        assert_eq!(receipt["readerObservationMicros"], 0);
        samples.push(elapsed.as_micros());
    }
    samples.sort_unstable();
    let percentile = |percent: usize| samples[(samples.len() * percent).div_ceil(100) - 1];
    let p50 = percentile(50);
    let p95 = percentile(95);
    let p99 = percentile(99);
    let max = *samples.last().expect("wall samples");
    eprintln!(
        "Hook launcher cold wall micros: n={SAMPLES} p50={p50} p95={p95} p99={p99} max={max}"
    );
    // This is whole-process wall time (POSIX launcher + process creation + Rust
    // evaluator), not policy-kernel time. The borrowed evaluator has its own
    // strict submillisecond gate; this boundary stays process-per-invocation and
    // must not be hidden behind a daemon or mmap optimization.
    assert!(
        max < INVOCATION_TIMEOUT.as_micros(),
        "Hook launcher cold wall max={max}us exceeded the 1s process deadline (p99={p99}us)"
    );
}
