use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use super::runtime_gates::{duration_literal, duration_millis_from_manifest, read_toml};
use super::shared::SharedBenchmarkToml;

pub(crate) fn asp_codex_rollout_session_index_algorithm_pressure_stays_inside_scenario_gate() {
    let root = temp_project_root("asp-agent-session-lifecycle-audit-empty-scenario");
    std::fs::create_dir_all(root.join("home/.agent-semantic-protocols"))
        .expect("create scenario state home");
    write_rollout_only_child(&root, "scenario-root-session", "scenario-child-session");
    let started_at = Instant::now();
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "agent",
            "session",
            "lifecycle",
            "audit",
            "--root-session-id",
            "scenario-root-session",
            "--json",
        ])
        .current_dir(&root)
        .env(
            "ASP_STATE_HOME",
            root.join("home/.agent-semantic-protocols"),
        )
        .env("HOME", root.join("home"))
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("run lifecycle audit scenario");
    let elapsed = started_at.elapsed();

    assert!(
        output.status.success(),
        "lifecycle audit scenario failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        elapsed <= Duration::from_secs(3),
        "empty lifecycle audit scenario took {:?}",
        elapsed
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("parse lifecycle audit json");
    assert_eq!(
        json["summary"]["rolloutSessionCount"].as_u64(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(
        json["summary"]["rolloutOnlySessionCount"].as_u64(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(
        json["summary"]["registrySessionCount"].as_u64(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(
        json["rolloutOnlySessions"][0]["sessionId"].as_str(),
        Some("scenario-child-session"),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );

    let _ = std::fs::remove_dir_all(root);
}

pub(crate) fn asp_agent_session_status_and_reuse_hot_paths_stay_inside_scenario_gate() {
    let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_agent_session_status_and_reuse_hot_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);
    let max_stdout_bytes = benchmark
        .max_stdout_bytes
        .expect("agent session hot path benchmark must define max_stdout_bytes");

    let root = temp_project_root("asp-agent-session-status-hot-path");
    let home = root.join("home");
    let state_home = home.join(".agent-semantic-protocols");
    std::fs::create_dir_all(&state_home).expect("create scenario state home");
    write_asp_explorer_config(&home, "gpt-5.4-mini", "workspace-write");
    write_rollout_only_child(&root, "scenario-root-session", "scenario-child-session");

    let register = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "agent",
            "session",
            "register",
            "--name",
            "asp-explore",
            "--child-session-id",
            "scenario-child-session",
            "--root-session-id",
            "scenario-root-session",
            "--roles",
            "subagent,search",
            "--json",
        ])
        .current_dir(&root)
        .env("ASP_STATE_HOME", &state_home)
        .env("HOME", &home)
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("register status hot path fixture");
    assert!(
        register.status.success(),
        "register failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&register.stdout),
        String::from_utf8_lossy(&register.stderr)
    );

    let scenario_started_at = Instant::now();
    let status_started_at = Instant::now();
    let status = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "agent",
            "session",
            "status",
            "--name",
            "asp-explore",
            "--root-session-id",
            "scenario-root-session",
            "--json",
        ])
        .current_dir(&root)
        .env("ASP_STATE_HOME", &state_home)
        .env("HOME", &home)
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("run status hot path fixture");
    let status_elapsed = status_started_at.elapsed();
    assert!(
        status.status.success(),
        "status failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&status.stdout),
        String::from_utf8_lossy(&status.stderr)
    );
    assert!(
        status_elapsed.as_millis() <= max_total_ms,
        "agent session status hot path exceeded benchmark max_total={} observed={}",
        benchmark.max_total,
        duration_literal(status_elapsed)
    );
    assert!(
        status.stdout.len() as u64 <= max_stdout_bytes,
        "agent session status stdout exceeded benchmark max_stdout_bytes={} observed={}",
        max_stdout_bytes,
        status.stdout.len()
    );
    let status_json: Value = serde_json::from_slice(&status.stdout).expect("parse status json");
    assert_eq!(status_json["routable"].as_bool(), Some(true));
    assert_eq!(
        status_json["rolloutActivity"]["sessionActivity"]["status"].as_str(),
        Some("idle-resumable"),
        "{}",
        String::from_utf8_lossy(&status.stdout)
    );
    assert_eq!(
        status_json["nextAction"].as_str(),
        Some("child-idle-resumable-reuse-existing-child"),
        "{}",
        String::from_utf8_lossy(&status.stdout)
    );

    let reuse_started_at = Instant::now();
    let reuse = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "agent",
            "session",
            "reuse",
            "--name",
            "asp-explore",
            "--root-session-id",
            "scenario-root-session",
            "--json",
        ])
        .current_dir(&root)
        .env("ASP_STATE_HOME", &state_home)
        .env("HOME", &home)
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("run reuse hot path fixture");
    let reuse_elapsed = reuse_started_at.elapsed();
    assert!(
        reuse.status.success(),
        "reuse failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&reuse.stdout),
        String::from_utf8_lossy(&reuse.stderr)
    );
    assert!(
        reuse_elapsed.as_millis() <= max_total_ms,
        "agent session reuse hot path exceeded benchmark max_total={} observed={}",
        benchmark.max_total,
        duration_literal(reuse_elapsed)
    );
    assert!(
        reuse.stdout.len() as u64 <= max_stdout_bytes,
        "agent session reuse stdout exceeded benchmark max_stdout_bytes={} observed={}",
        max_stdout_bytes,
        reuse.stdout.len()
    );
    let scenario_elapsed = scenario_started_at.elapsed();
    assert!(
        scenario_elapsed.as_millis() <= max_total_ms,
        "agent session status+reuse hot path exceeded benchmark max_total={} observed={}",
        benchmark.max_total,
        duration_literal(scenario_elapsed)
    );
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-agent-session-status-and-reuse-hot-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "asp agent session status --name asp-explore --root-session-id <id> --json",
            "asp agent session reuse --name asp-explore --root-session-id <id> --json"
        ],
        "phase": benchmark.phase.as_deref().unwrap_or("warm"),
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": benchmark.max_provider_process_count,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "fallbackReason": benchmark.fallback_reason,
            "routeSource": benchmark.route_source,
        },
        "observed": {
            "statusElapsed": duration_literal(status_elapsed),
            "reuseElapsed": duration_literal(reuse_elapsed),
            "total": duration_literal(scenario_elapsed),
            "statusStdoutBytes": status.stdout.len(),
            "reuseStdoutBytes": reuse.stdout.len(),
        }
    });
    assert_eq!(performance_gate["expected"]["fallbackReason"], "none");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_agent_session_status_and_reuse_hot_paths_stay_inside_scenario_gate_test() {
    asp_agent_session_status_and_reuse_hot_paths_stay_inside_scenario_gate();
}

fn write_rollout_only_child(root: &std::path::Path, root_session_id: &str, child_session_id: &str) {
    let sessions_dir = root.join("home/.codex/sessions/2026/07/05");
    std::fs::create_dir_all(&sessions_dir).expect("create scenario codex sessions dir");
    let rollout_path = sessions_dir.join(format!(
        "rollout-2026-07-05T00-00-00-{child_session_id}.jsonl"
    ));
    let session_meta = serde_json::json!({
        "timestamp": "2026-07-05T00:00:00.000Z",
        "type": "session_meta",
        "payload": {
            "session_id": root_session_id,
            "id": child_session_id,
            "parent_thread_id": root_session_id,
            "thread_source": "subagent",
            "timestamp": "2026-07-05T00:00:00.000Z",
            "cwd": root.display().to_string(),
            "originator": "Codex Desktop",
            "cli_version": "0.142.5",
            "source": {
                "subagent": {
                    "thread_spawn": {
                        "parent_thread_id": root_session_id,
                        "depth": 1,
                        "agent_path": null,
                        "agent_nickname": "ASP search",
                        "agent_role": "asp_explorer"
                    }
                }
            }
        }
    });
    let turn_context = serde_json::json!({
        "timestamp": "2026-07-05T00:00:00.100Z",
        "type": "turn_context",
        "payload": {
            "model": "gpt-5.4-mini",
            "sandbox_policy": {"type": "workspace-write"},
            "approval_policy": "never",
            "permission_profile": {"type": "managed"}
        }
    });
    let task_complete = serde_json::json!({
        "timestamp": "2026-07-05T00:00:00.200Z",
        "type": "event_msg",
        "payload": {
            "type": "task_complete",
            "turn_id": "turn-child"
        }
    });
    std::fs::write(
        &rollout_path,
        format!("{session_meta}\n{turn_context}\n{task_complete}\n"),
    )
    .expect("write rollout-only child session");
}

fn write_asp_explorer_config(home: &std::path::Path, model: &str, sandbox: &str) {
    let agents_dir = home.join(".codex").join("agents");
    std::fs::create_dir_all(&agents_dir).expect("create codex agents dir");
    std::fs::write(
        agents_dir.join("asp-explorer.toml"),
        format!("name = \"asp_explorer\"\nmodel = \"{model}\"\nsandbox_mode = \"{sandbox}\"\n"),
    )
    .expect("write asp explorer config");
}

fn temp_project_root(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "agent-semantic-protocol-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).expect("create temp project root");
    root
}
