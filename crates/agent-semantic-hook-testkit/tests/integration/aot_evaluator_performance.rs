//! Performance contracts for the borrowed AOT evaluator.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use agent_semantic_hook::aot_evaluator::evaluate_pre_tool;

use super::aot_evaluator_contract::{GENERATION, canonical_generation};

#[cfg(unix)]
fn current_thread_cpu_nanos() -> u128 {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let status = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut time) };
    assert_eq!(status, 0, "read current-thread CPU clock");
    (time.tv_sec as u128) * 1_000_000_000 + time.tv_nsec as u128
}

#[cfg(not(unix))]
fn current_thread_cpu_nanos() -> u128 {
    static STARTED: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    STARTED.get_or_init(Instant::now).elapsed().as_nanos()
}

#[test]
fn borrowed_aot_evaluator_meets_submillisecond_p99_without_probe() {
    // Use enough warm samples that an operating-system preemption cannot be
    // mistaken for evaluator work. The enclosing one-second deadline still
    // catches blocking behavior; a real per-call regression remains visible
    // at p99 across this larger sample.
    const WARMUP: usize = 16;
    const SAMPLES: usize = 1_024;
    const TEST_DEADLINE: Duration = Duration::from_secs(1);
    const PAYLOAD: &str = r#"{"session_id":"testkit-perf","cwd":".","hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"< src/lib.rs"}}"#;

    let (sender, receiver) = mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        for _ in 0..WARMUP {
            let decision = evaluate_pre_tool(GENERATION, PAYLOAD, "Bash")
                .expect("warm borrowed HookPolicyBundle")
                .expect("warm registered source decision");
            assert_eq!(decision.decision, "deny");
            assert!(!decision.probe_process_launched);
        }
        let mut samples = Vec::with_capacity(SAMPLES);
        for _ in 0..SAMPLES {
            let started = Instant::now();
            let decision = evaluate_pre_tool(GENERATION, PAYLOAD, "Bash")
                .expect("evaluate borrowed HookPolicyBundle")
                .expect("registered source decision");
            samples.push(started.elapsed().as_micros());
            assert_eq!(decision.decision, "deny");
            assert!(!decision.probe_process_launched);
            assert_eq!(decision.elapsed_micros, 0);
            assert_eq!(decision.reader_observation_micros, 0);
        }
        samples.sort_unstable();
        sender.send(samples).expect("publish performance receipt");
    });
    let samples = receiver
        .recv_timeout(TEST_DEADLINE)
        .expect("borrowed evaluator exceeded the 1s test deadline");
    worker.join().expect("borrowed evaluator worker");

    let percentile = |percent: usize| samples[(samples.len() * percent).div_ceil(100) - 1];
    let p50 = percentile(50);
    let p95 = percentile(95);
    let p99 = percentile(99);
    let max = *samples.last().expect("wall samples");
    eprintln!(
        "Hook borrowed evaluator wall micros: n={SAMPLES} p50={p50} p95={p95} p99={p99} max={max}"
    );
    assert!(p99 < 1_000, "Hook borrowed evaluator p99={p99}us");
}

#[test]
fn canonical_config_aot_policy_kernel_meets_submillisecond_p99() {
    const WARMUP: usize = 16;
    const SAMPLES: usize = 1_024;
    const TEST_DEADLINE: Duration = Duration::from_secs(1);
    let generation = canonical_generation();
    let payload = serde_json::json!({
        "session_id": "testkit-canonical-perf",
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "cargo test -p agent-semantic-hook"}
    })
    .to_string();
    let (sender, receiver) = mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        for _ in 0..WARMUP {
            let decision = evaluate_pre_tool(&generation, &payload, "Bash")
                .expect("warm canonical policy")
                .expect("warm canonical Testing route");
            assert_eq!(decision.config_rule_id, "testing-role-dispatch");
        }
        let mut samples = Vec::with_capacity(SAMPLES);
        for _ in 0..SAMPLES {
            let started = current_thread_cpu_nanos();
            let decision = evaluate_pre_tool(&generation, &payload, "Bash")
                .expect("evaluate canonical policy")
                .expect("canonical Testing route");
            samples.push((current_thread_cpu_nanos() - started) / 1_000);
            assert_eq!(decision.config_rule_id, "testing-role-dispatch");
        }
        samples.sort_unstable();
        sender
            .send(samples)
            .expect("publish canonical performance receipt");
    });
    let samples = receiver
        .recv_timeout(TEST_DEADLINE)
        .expect("canonical AOT evaluator exceeded the 1s test deadline");
    worker.join().expect("canonical AOT evaluator worker");
    let p99 = samples[(samples.len() * 99).div_ceil(100) - 1];
    let max = *samples.last().expect("canonical policy samples");
    eprintln!("Canonical Hook AOT kernel micros: n={SAMPLES} p99={p99} max={max}");
    assert!(p99 < 1_000, "canonical Hook AOT p99={p99}us");
}
