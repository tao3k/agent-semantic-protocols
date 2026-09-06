//! Reader-route classification, dynamic observation, and cache-bound contracts.

use super::{
    Duration, GENERATION, ReaderProbeAccess, ReaderProbeObservation, bind_reader_probe_observation,
    canonical_generation, diagnose_reader_probe, diagnose_reader_probe_with_state_home,
    evaluate_pre_tool, materialize_reader_probe_fixture, reader_probe_request,
};

#[test]
fn unknown_reader_observation_does_not_produce_a_read_action() {
    let mut payload = serde_json::json!({
        "session_id": "testkit-reader-unknown",
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": { "command": "future-source-consumer src/lib.rs" }
    });
    let observation = ReaderProbeObservation {
        subject: "src/lib.rs".to_owned(),
        access: ReaderProbeAccess::Unknown,
        backend: "permission-differential".to_owned(),
        terminal: "probe-unknown".to_owned(),
        elapsed_micros: 0,
        probe_process_launched: true,
        cleanup_verified: true,
        cache_hit: false,
        behavior_key: None,
    };
    bind_reader_probe_observation(&mut payload, Some(&observation))
        .expect("bind typed Unknown observation");
    let payload = payload.to_string();
    assert!(
        evaluate_pre_tool(GENERATION, &payload, "Bash")
            .expect("evaluate typed Unknown observation")
            .is_none(),
        "Unknown SourceAccess must not be promoted to Read"
    );

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
    assert_eq!(explicit_read.subject.as_deref(), Some("src/lib.rs"));
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
fn static_reader_catalog_routes_git_show_to_asp() {
    let generation = canonical_generation();
    let payload = serde_json::json!({
        "session_id": "testkit-static-git-show",
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": { "command": "git show HEAD:src/lib.rs" }
    });
    let payload_json = payload.to_string();
    let decision = evaluate_pre_tool(&generation, &payload_json, "Bash")
        .expect("evaluate git show")
        .expect("git show must deny");
    assert_eq!(decision.decision, "deny");
    assert_eq!(decision.config_rule_id, "git-history-inspection-dispatch");
}

#[test]
fn static_reader_catalog_routes_wrapped_absolute_git_show_to_asp() {
    let generation = canonical_generation();
    let payload = serde_json::json!({
        "session_id": "testkit-static-wrapped-git-show",
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": { "command": ".devenv/devenv-profile-exec /usr/bin/git show HEAD:src/lib.rs" }
    });
    let payload_json = payload.to_string();
    let decision = evaluate_pre_tool(&generation, &payload_json, "Bash")
        .expect("evaluate wrapped absolute git show")
        .expect("wrapped absolute git show must route");
    assert_eq!(decision.decision, "deny");
    assert_eq!(decision.config_rule_id, "git-history-inspection-dispatch");
    assert!(!decision.probe_process_launched);
}

#[test]
fn static_reader_catalog_routes_git_diff_to_asp() {
    let generation = canonical_generation();
    let payload = serde_json::json!({
        "session_id": "testkit-static-git-diff",
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": { "command": "git diff -- src/lib.rs" }
    });
    let payload_json = payload.to_string();
    let decision = evaluate_pre_tool(&generation, &payload_json, "Bash")
        .expect("evaluate git diff")
        .expect("git diff must deny");
    assert_eq!(decision.decision, "deny");
    assert_eq!(decision.config_rule_id, "git-history-inspection-dispatch");
}

#[test]
fn git_subcommands_without_a_declared_reader_argument_do_not_inherit_read() {
    let generation = canonical_generation();
    let payload = serde_json::json!({
        "session_id": "testkit-git-non-reader",
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": { "command": "git cat-file -e HEAD:src/lib.rs" }
    });
    assert!(
        evaluate_pre_tool(&generation, &payload.to_string(), "Bash")
            .expect("evaluate non-reader git command")
            .is_none(),
        "a git executable name alone must not fabricate Read"
    );
}

#[test]
fn static_reader_catalog_routes_wrapped_gerbil_sed_to_asp() {
    let generation = canonical_generation();
    let command = ".devenv/devenv-profile-exec sed -n '130,180p;250,275p;318,365p' languages/asp-gerbil-scheme/src/language/evidence.ss";
    let payload = serde_json::json!({
        "session_id": "testkit-static-gerbil-sed",
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": { "command": command }
    });
    let payload_json = payload.to_string();
    let request = reader_probe_request(&generation, &payload_json, "Bash")
        .expect("build Reader observation request")
        .expect("static wrapped sed must require a Reader observation");
    let state_home = tempfile::tempdir().expect("state home");
    let observation = diagnose_reader_probe_with_state_home(
        request.command_tokens,
        request.subject,
        request.wrapped_command,
        request.reader_behavior_patterns,
        state_home.path(),
    )
    .expect("observe static wrapped sed");
    assert_eq!(observation.access, ReaderProbeAccess::Read);
    assert_eq!(observation.terminal, "reader-behavior-catalog-hit");
    assert!(!observation.probe_process_launched);

    let mut observed_payload = payload;
    bind_reader_probe_observation(&mut observed_payload, Some(&observation))
        .expect("bind static Reader observation");
    let observed_payload_json = observed_payload.to_string();
    let decision = evaluate_pre_tool(&generation, &observed_payload_json, "Bash")
        .expect("evaluate observed wrapped sed")
        .expect("registered Gerbil source read must deny");
    assert_eq!(decision.decision, "deny");
    assert_eq!(decision.config_rule_id, "route-read-to-asp-languages");
    assert_eq!(decision.language, Some("gerbil-scheme"));
    assert_eq!(decision.access, "read");
    assert_eq!(decision.backend, "hook-policy-bundle-reader-catalog");
}

#[test]
fn batched_static_reader_stages_route_the_entire_host_call() {
    let generation = canonical_generation();
    let payload = serde_json::json!({
        "session_id": "testkit-batched-static-readers",
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {
            "command": "sed -n '1,3p' crates/agent-semantic-hook/src/lib.rs && sed -n '1,3p' crates/agent-semantic-client/src/lib.rs"
        }
    });
    let request = reader_probe_request(&generation, &payload.to_string(), "Bash")
        .expect("build batched Reader observation request")
        .expect("one static Reader stage must govern the complete Host call");
    assert_eq!(
        request.command_tokens.first().map(String::as_str),
        Some("sed")
    );
    let state_home = tempfile::tempdir().expect("state home");
    let observation = diagnose_reader_probe_with_state_home(
        request.command_tokens,
        request.subject,
        request.wrapped_command,
        request.reader_behavior_patterns,
        state_home.path(),
    )
    .expect("observe batched static Reader");
    assert_eq!(observation.access, ReaderProbeAccess::Read);
    assert!(!observation.probe_process_launched);
    let mut observed = payload;
    bind_reader_probe_observation(&mut observed, Some(&observation))
        .expect("bind batched Reader observation");
    let observed_json = observed.to_string();
    let decision = evaluate_pre_tool(&generation, &observed_json, "Bash")
        .expect("evaluate batched static Reader")
        .expect("batched registered source read must deny");
    assert_eq!(decision.config_rule_id, "route-read-to-asp-languages");
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
fn declared_reader_behavior_pattern_routes_without_process_launch() {
    const STATIC_GENERATION: &str = r#"{"schemaId":"agent.semantic-protocols.hook-policy-bundle","schemaVersion":1,"generationDigest":"blake3-256:static-reader","commandActionPatterns":[{"action":"read","argvPatternAny":[["BATT","-s"]]}],"rules":[{"id":"route-source","matchers":["Bash"],"wrappedCommand":true,"actions":["read"],"registeredExtensions":["rs"],"decision":"deny","reasonKind":"registered-source-route-required","message":"Use ASP."}]}"#;
    let mut payload = serde_json::json!({
        "session_id": "testkit-static-reader",
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "BATT -s src/lib.rs"}
    });
    let request = reader_probe_request(STATIC_GENERATION, &payload.to_string(), "Bash")
        .expect("project static Reader request")
        .expect("static Reader request");
    let observation = diagnose_reader_probe(
        request.command_tokens,
        request.subject,
        request.wrapped_command,
        request.reader_behavior_patterns,
    )
    .expect("static Reader observation");
    assert_eq!(observation.access, ReaderProbeAccess::Read);
    assert!(!observation.probe_process_launched);
    assert_eq!(observation.backend, "hook-policy-bundle-reader-catalog");
    bind_reader_probe_observation(&mut payload, Some(&observation))
        .expect("bind static Reader observation");
    let payload_json = payload.to_string();
    let decision = evaluate_pre_tool(STATIC_GENERATION, &payload_json, "Bash")
        .expect("evaluate static Reader")
        .expect("static Reader deny");
    assert_eq!(decision.evidence, "reader-behavior-static-catalog");
    assert_eq!(decision.decision, "deny");

    let wrapped = diagnose_reader_probe(
        vec![
            "future-wrapper".to_owned(),
            "BATT".to_owned(),
            "-s".to_owned(),
            "src/lib.rs".to_owned(),
        ],
        "src/lib.rs".to_owned(),
        true,
        vec![vec!["BATT".to_owned(), "-s".to_owned()]],
    )
    .expect("wrapped static Reader observation");
    assert_eq!(wrapped.access, ReaderProbeAccess::Read);
    assert_eq!(wrapped.terminal, "reader-behavior-catalog-hit");
    assert!(!wrapped.probe_process_launched);
}

#[test]
fn reader_probe_permission_differential_authorizes_only_read_behavior() {
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
    let state_home = tempfile::tempdir().expect("isolated Reader State Home");
    for (mode, expected_access, denied) in [
        ("read", ReaderProbeAccess::Read, true),
        ("write", ReaderProbeAccess::Unknown, false),
        ("read-write", ReaderProbeAccess::Unknown, false),
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
        let observation = diagnose_reader_probe_with_state_home(
            request.command_tokens,
            request.subject,
            request.wrapped_command,
            request.reader_behavior_patterns,
            state_home.path(),
        )
        .unwrap_or_else(|| panic!("Reader probe produced no {mode} observation"));
        if observation.access == ReaderProbeAccess::Unknown
            && matches!(
                observation.terminal.as_str(),
                "probe-deferred" | "probe-timeout"
            )
        {
            bind_reader_probe_observation(&mut payload, Some(&observation))
                .unwrap_or_else(|error| panic!("bind deferred {mode} observation: {error}"));
            let payload_json = payload.to_string();
            let decision = evaluate_pre_tool(GENERATION, &payload_json, "Bash")
                .expect("evaluate deferred Reader observation")
                .expect("indeterminate registered-source Reader must fail closed");
            assert_eq!(decision.decision, "deny");
            assert_eq!(decision.evidence, "reader-probe-indeterminate-fail-closed");
            continue;
        }
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
        if let Some(decision) = decision {
            assert_eq!(decision.access, "read");
            assert_eq!(decision.access_mode, "read-permission");
            if denied {
                assert_eq!(decision.evidence, "reader-probe-read-permission");
                assert!(decision.cleanup_verified);
            } else {
                assert_eq!(
                    decision.evidence, "reader-probe-indeterminate-fail-closed",
                    "mode={mode} terminal={}",
                    observation.terminal
                );
            }
        } else {
            assert!(!denied, "confirmed read must deny: mode={mode}");
        }
        if mode == "read" {
            let request = reader_probe_request(GENERATION, &payload_json, "Bash")
                .expect("project cached Reader request")
                .expect("cached Reader request");
            let cached = diagnose_reader_probe_with_state_home(
                request.command_tokens,
                request.subject,
                request.wrapped_command,
                request.reader_behavior_patterns,
                state_home.path(),
            )
            .expect("cached Reader observation");
            assert_eq!(cached.access, ReaderProbeAccess::Read);
            assert!(cached.cache_hit);
            assert!(!cached.probe_process_launched);
            assert_eq!(cached.backend, "process-memory-reader-catalog");
        }
    }
}

#[test]
fn concurrent_dynamic_cache_hits_are_submillisecond_and_process_free() {
    #[cfg(target_os = "macos")]
    {
        const WORKERS: usize = 32;
        let fixture = materialize_reader_probe_fixture().expect("Reader behavior fixture");
        let state_home = tempfile::tempdir().expect("isolated Reader State Home");
        let tokens = vec![
            fixture.to_string_lossy().into_owned(),
            "read".to_owned(),
            "fixture.rs".to_owned(),
        ];
        let cold = diagnose_reader_probe_with_state_home(
            tokens.clone(),
            "fixture.rs".to_owned(),
            false,
            Vec::new(),
            state_home.path(),
        )
        .expect("cold Reader observation");
        if cold.access == ReaderProbeAccess::Unknown {
            assert!(
                matches!(cold.terminal.as_str(), "probe-deferred" | "probe-timeout"),
                "cold={cold:?}"
            );
            assert_eq!(
                cold.probe_process_launched,
                cold.terminal == "probe-timeout",
                "cold={cold:?}"
            );
            assert!(cold.cleanup_verified, "cold={cold:?}");
            return;
        }
        assert_eq!(cold.access, ReaderProbeAccess::Read);
        assert!(cold.probe_process_launched);

        let barrier = std::sync::Arc::new(std::sync::Barrier::new(WORKERS));
        let workers = (0..WORKERS)
            .map(|_| {
                let barrier = barrier.clone();
                let tokens = tokens.clone();
                let state_home = state_home.path().to_owned();
                std::thread::spawn(move || {
                    barrier.wait();
                    let observation = diagnose_reader_probe_with_state_home(
                        tokens,
                        "fixture.rs".to_owned(),
                        false,
                        Vec::new(),
                        &state_home,
                    )
                    .expect("cached Reader observation");
                    (
                        Duration::from_micros(observation.elapsed_micros),
                        observation,
                    )
                })
            })
            .collect::<Vec<_>>();
        let mut receipts = workers
            .into_iter()
            .map(|worker| worker.join().expect("Reader cache worker"))
            .collect::<Vec<_>>();
        assert!(receipts.iter().all(|(_, observation)| {
            observation.access == ReaderProbeAccess::Read
                && observation.cache_hit
                && !observation.probe_process_launched
        }));
        receipts.sort_by_key(|(elapsed, _)| *elapsed);
        let p99 = receipts[WORKERS * 99 / 100].0;
        eprintln!("Reader dynamic cache hit concurrency: n={WORKERS} p99={p99:?}");
        assert!(p99 < Duration::from_millis(1), "cache-hit p99={p99:?}");
    }
}
