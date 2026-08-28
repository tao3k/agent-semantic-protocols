use std::sync::mpsc;
use std::time::{Duration, Instant};

use agent_semantic_hook::aot_evaluator::{evaluate_pre_tool, reader_probe_request};
use agent_semantic_hook::{
    ReaderProbeAccess, bind_reader_probe_observation, diagnose_reader_probe,
    materialize_reader_probe_fixture,
};

const GENERATION: &str = r#"{"schemaId":"agent.semantic-protocols.hook-generation","schemaVersion":1,"generationDigest":"blake3-256:testkit-perf","rules":[{"id":"route-source","matchers":["Bash"],"actions":["read"],"registeredExtensions":["rs"],"decision":"deny","reasonKind":"registered-source-route-required","message":"Use ASP."}]}"#;

#[test]
fn borrowed_aot_evaluator_meets_submillisecond_p99_without_probe() {
    const SAMPLES: usize = 128;
    const TEST_DEADLINE: Duration = Duration::from_secs(1);
    const PAYLOAD: &str = r#"{"session_id":"testkit-perf","cwd":".","hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"< src/lib.rs"}}"#;

    let (sender, receiver) = mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        let mut samples = Vec::with_capacity(SAMPLES);
        for _ in 0..SAMPLES {
            let started = Instant::now();
            let decision = evaluate_pre_tool(GENERATION, PAYLOAD, "Bash")
                .expect("evaluate borrowed HookGeneration")
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
fn bare_registered_operands_do_not_authorize_a_read_decision() {
    let unknown_source_witnesses = [
        "cat src/lib.rs",
        "sed -n '1p' src/lib.rs",
        "grep needle src/lib.rs",
        "rg needle src/lib.rs",
        "git show HEAD:src/lib.rs",
        "git diff -- src/lib.rs",
    ];
    for command in unknown_source_witnesses {
        let payload = serde_json::json!({
            "session_id": "testkit-reader-matrix",
            "cwd": ".",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": { "command": command }
        })
        .to_string();
        assert!(
            evaluate_pre_tool(GENERATION, &payload, "Bash")
                .unwrap_or_else(|error| panic!("evaluate {command:?}: {error}"))
                .is_none(),
            "bare operand {command:?} must not be promoted to Read"
        );
    }

    let explicit_read_payload = serde_json::json!({
        "session_id": "testkit-reader-redirection",
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": { "command": "< src/lib.rs" }
    })
    .to_string();
    let explicit_read = evaluate_pre_tool(GENERATION, &explicit_read_payload, "Bash")
        .expect("evaluate explicit read redirection")
        .expect("explicit registered source read must deny");
    assert_eq!(explicit_read.decision, "deny");
    assert_eq!(explicit_read.config_rule_id, "route-source");
    assert_eq!(explicit_read.subject, Some("src/lib.rs"));
    assert_eq!(explicit_read.access, "read");
    assert_eq!(explicit_read.evidence, "shell-redirection-read");
    assert!(!explicit_read.probe_process_launched);
    assert_eq!(explicit_read.elapsed_micros, 0);
    assert!(explicit_read.policy_fast_path);

    for command in ["just --list | rg hook", "rg hook", "git status --short"] {
        let payload = serde_json::json!({
            "session_id": "testkit-reader-allow",
            "cwd": ".",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": { "command": command }
        })
        .to_string();
        assert!(
            evaluate_pre_tool(GENERATION, &payload, "Bash")
                .unwrap_or_else(|error| panic!("evaluate allow witness {command:?}: {error}"))
                .is_none(),
            "metadata witness {command:?} unexpectedly matched a source rule"
        );
    }
}

#[test]
fn unknown_command_names_allow_without_confirmed_read_evidence() {
    for command in ["BATT -s src/lib.rs", "BATT-random-7f3 -s src/lib.rs"] {
        let payload = serde_json::json!({
            "session_id": "testkit-unknown-command",
            "cwd": ".",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": { "command": command }
        })
        .to_string();
        assert!(
            evaluate_pre_tool(GENERATION, &payload, "Bash")
                .unwrap_or_else(|error| panic!("evaluate {command:?}: {error}"))
                .is_none(),
            "unknown command name {command:?} must remain Allow(None)"
        );
    }
}

#[test]
fn reader_probe_open_flags_authorize_only_read_only_access() {
    let fixture = materialize_reader_probe_fixture().expect("Reader behavior fixture");
    let random_root = tempfile::tempdir().expect("random Reader fixture root");
    let random_fixture = random_root.path().join("BATT-random-7f3");
    std::fs::hard_link(&fixture, &random_fixture).expect("link random Reader fixture");
    std::fs::set_permissions(
        &random_fixture,
        std::fs::metadata(&fixture)
            .expect("Reader fixture metadata")
            .permissions(),
    )
    .expect("random Reader fixture mode");
    for (mode, expected_access, denied) in [
        ("read", ReaderProbeAccess::Read, true),
        ("write", ReaderProbeAccess::NotRead, false),
        ("read-write", ReaderProbeAccess::NotRead, false),
    ] {
        let mut payload = serde_json::json!({
            "session_id": format!("testkit-reader-{mode}"),
            "cwd": ".",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": {
                "command": format!("{} {mode} src/lib.rs", random_fixture.display())
            }
        });
        let payload_json = payload.to_string();
        let request = reader_probe_request(GENERATION, &payload_json, "Bash")
            .unwrap_or_else(|error| panic!("project {mode} Reader request: {error}"))
            .unwrap_or_else(|| panic!("Reader request was ambiguous for {mode}"));
        let observation = diagnose_reader_probe(request.command_tokens, request.subject)
            .unwrap_or_else(|| panic!("Reader probe produced no {mode} observation"));
        assert_eq!(
            observation.access, expected_access,
            "{mode}: backend={} terminal={} elapsedMicros={}",
            observation.backend, observation.terminal, observation.elapsed_micros
        );
        assert!(observation.cleanup_verified, "{mode}");
        bind_reader_probe_observation(&mut payload, Some(&observation))
            .unwrap_or_else(|error| panic!("bind {mode} observation: {error}"));
        let payload = payload.to_string();
        let decision = evaluate_pre_tool(GENERATION, &payload, "Bash")
            .unwrap_or_else(|error| panic!("evaluate {mode} observation: {error}"));
        assert_eq!(decision.is_some(), denied, "{mode}");
        if let Some(decision) = decision {
            assert_eq!(decision.access, "read");
            assert_eq!(decision.access_mode, "O_RDONLY");
            assert_eq!(decision.evidence, "reader-probe-open-read-only");
            assert!(decision.cleanup_verified);
        }
    }
}
