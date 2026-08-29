use std::collections::{BTreeMap, HashSet};
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::sync::MutexGuard;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use agent_semantic_hook::{
    DecisionKind, DecisionRoute, DecisionRouteKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID,
    HOOK_DECISION_SCHEMA_VERSION, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision,
    ReasonKind, StdinMode, append_hook_event_state, has_recorded_subagent_context,
    latest_hook_session_agent_route, latest_hook_session_agent_route_for_root,
};
use fs2::FileExt;
use serde_json::Value;

#[test]
fn concurrent_hook_event_appends_write_valid_json_lines() {
    let project_root = unique_project_root();
    let state_home = unique_state_home(&project_root);
    let _state_home_guard = AspStateHomeGuard::activate(state_home);
    let run_id = project_root
        .file_name()
        .and_then(|name| name.to_str())
        .expect("temp project root name")
        .to_string();
    let run_prefix = format!("{run_id}_");
    let worker_count = 32;

    let handles = (0..worker_count)
        .map(|index| {
            let project_root = project_root.clone();
            let run_id = run_id.clone();
            thread::spawn(move || append_hook_event_state(&project_root, &decision(&run_id, index)))
        })
        .collect::<Vec<_>>();

    let event_paths = handles
        .into_iter()
        .map(|handle| handle.join().expect("hook event thread panicked").unwrap())
        .collect::<Vec<_>>();
    let event_path = event_paths.first().expect("event path");
    assert!(event_paths.iter().all(|path| path == event_path));

    let content = fs::read_to_string(event_path).expect("event log should exist");
    let lines = content
        .lines()
        .filter(|line| line.contains(&run_prefix))
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), worker_count);

    let mut seen_paths = HashSet::new();
    for line in lines {
        let event = serde_json::from_str::<Value>(line).expect("event line should be valid JSON");
        assert_eq!(event["schemaId"], "agent.semantic-protocols.hook.event");
        assert_eq!(event["protocolId"], HOOK_PROTOCOL_ID);
        assert_eq!(event["reasonKind"], "registered-source-route-required");
        let path = event["subject"]["paths"][0]
            .as_str()
            .expect("event path should be a string");
        assert!(path.starts_with(&run_prefix));
        seen_paths.insert(path.to_string());
    }
    assert_eq!(seen_paths.len(), worker_count);

    fs::remove_dir_all(&project_root).ok();
}

#[test]
fn event_writer_lock_contention_is_bounded_and_fail_closed() {
    let project_root = unique_project_root();
    let state_home = unique_state_home(&project_root);
    let _state_home_guard = AspStateHomeGuard::activate(state_home);
    let event_path = append_hook_event_state(&project_root, &decision("lock-owner", 0))
        .expect("first event transaction should commit");
    let lock_path = event_path.with_file_name("events.jsonl.lock");
    assert!(
        lock_path.is_file(),
        "cross-process lock file must be durable"
    );
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&lock_path)
        .expect("open event writer lock");
    lock.lock_exclusive().expect("hold event writer lock");
    let started = Instant::now();
    let error = append_hook_event_state(&project_root, &decision("contended", 1))
        .expect_err("contended event transaction must fail closed");
    let elapsed = started.elapsed();
    assert!(error.contains("exceeded 100ms"), "{error}");
    assert!(
        elapsed >= Duration::from_millis(100) && elapsed < Duration::from_millis(500),
        "event writer lock boundary drifted: {elapsed:?}"
    );
    FileExt::unlock(&lock).expect("release event writer lock");

    fs::remove_dir_all(&project_root).ok();
}

#[test]
fn decision_event_projection_never_waits_for_a_contended_writer() {
    let project_root = unique_project_root();
    let state_home = unique_state_home(&project_root);
    let _state_home_guard = AspStateHomeGuard::activate(state_home);
    let event_path = append_hook_event_state(&project_root, &decision("lock-owner", 0))
        .expect("first event transaction should commit");
    let lock_path = event_path.with_file_name("events.jsonl.lock");
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&lock_path)
        .expect("open event writer lock");
    lock.lock_exclusive().expect("hold event writer lock");

    let started = Instant::now();
    let error = agent_semantic_hook::try_append_hook_event_state(
        &project_root,
        &decision("contended-decision", 1),
    )
    .expect_err("diagnostic projection must be dropped under contention");
    let elapsed = started.elapsed();
    assert!(error.contains("exceeded 0ms"), "{error}");
    assert!(
        elapsed < Duration::from_millis(20),
        "decision projection waited behind telemetry: {elapsed:?}"
    );
    FileExt::unlock(&lock).expect("release event writer lock");

    fs::remove_dir_all(&project_root).ok();
}

#[test]
fn recorded_subagent_context_tracks_latest_lifecycle_event() {
    let project_root = unique_project_root();
    let state_home = unique_state_home(&project_root);
    let _state_home_guard = AspStateHomeGuard::activate(state_home);
    let session_id = "subagent-session-123";
    let transcript_path = "/tmp/subagent-session-123.jsonl";

    append_hook_event_state(
        &project_root,
        &lifecycle_decision("subagent-start", session_id, transcript_path),
    )
    .expect("record subagent start");

    assert!(
        has_recorded_subagent_context(&project_root, Some(session_id.into()), None)
            .expect("lookup by session")
    );
    assert!(
        has_recorded_subagent_context(&project_root, None, Some(transcript_path.into()))
            .expect("lookup by transcript")
    );

    append_hook_event_state(
        &project_root,
        &lifecycle_decision("subagent-stop", session_id, transcript_path),
    )
    .expect("record subagent stop");

    assert!(
        !has_recorded_subagent_context(
            &project_root,
            Some(session_id.into()),
            Some(transcript_path.into()),
        )
        .expect("latest matching lifecycle event wins")
    );

    fs::remove_dir_all(&project_root).ok();
}

#[test]
fn oversized_hook_event_state_is_truncated_before_append() {
    let project_root = unique_project_root();
    let state_home = unique_state_home(&project_root);
    let _state_home_guard = AspStateHomeGuard::activate(state_home);
    let mut state_path =
        append_hook_event_state(&project_root, &decision("seed", 0)).expect("seed event");

    for _ in 0..8 {
        fs::write(&state_path, "x".repeat(5 * 1024 * 1024)).expect("write oversized state");
        let appended_path = append_hook_event_state(&project_root, &decision("oversized", 1))
            .expect("append event");
        if appended_path != state_path {
            state_path = appended_path;
            continue;
        }

        let content = fs::read_to_string(&state_path).expect("read state");
        let lines = content.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 1, "{content}");
        let event =
            serde_json::from_str::<Value>(lines[0]).expect("event line should be valid JSON");
        assert_eq!(event["schemaId"], "agent.semantic-protocols.hook.event");
        assert_eq!(event["subject"]["paths"][0], "oversized_event_state_1.rs");

        fs::remove_dir_all(&project_root).ok();
        return;
    }

    panic!("hook event state path changed repeatedly while testing oversized truncation");
}

#[test]
fn oversized_hook_event_state_preserves_recent_valid_events() {
    let project_root = unique_project_root();
    let state_home = unique_state_home(&project_root);
    let _state_home_guard = AspStateHomeGuard::activate(state_home);
    let state_path =
        append_hook_event_state(&project_root, &decision("retained", 0)).expect("seed event");
    let seed = fs::read_to_string(&state_path).expect("read seed event");
    let repeat_count = (5 * 1024 * 1024 / seed.len()) + 1;
    fs::write(&state_path, seed.repeat(repeat_count)).expect("write oversized valid state");

    append_hook_event_state(&project_root, &decision("current", 1)).expect("append event");

    let content = fs::read_to_string(&state_path).expect("read compacted state");
    let lines = content.lines().collect::<Vec<_>>();
    assert!(lines.len() > 1, "recent events should survive compaction");
    assert!(
        lines.len() < repeat_count,
        "oversized history should be bounded"
    );
    for line in &lines {
        serde_json::from_str::<Value>(line).expect("retained event line should be valid JSON");
    }
    let latest =
        serde_json::from_str::<Value>(lines.last().expect("latest event")).expect("latest JSON");
    assert_eq!(latest["subject"]["paths"][0], "current_event_state_1.rs");

    fs::remove_dir_all(&project_root).ok();
}

#[test]
fn oversized_hook_event_state_skips_partial_utf8_tail_line() {
    let project_root = unique_project_root();
    let state_home = unique_state_home(&project_root);
    let _state_home_guard = AspStateHomeGuard::activate(state_home);
    let state_path =
        append_hook_event_state(&project_root, &decision("retained", 0)).expect("seed event");
    let seed = fs::read(&state_path).expect("read seed event");
    let tail_bytes = 1024 * 1024;
    let mut tail = vec![0x80, b'\n'];
    tail.extend(std::iter::repeat_n(
        b'x',
        tail_bytes - tail.len() - seed.len() - 1,
    ));
    tail.push(b'\n');
    tail.extend_from_slice(&seed);
    assert_eq!(tail.len(), tail_bytes);
    let mut oversized = vec![b'x'; 4 * 1024 * 1024];
    oversized.extend_from_slice(&tail);
    fs::write(&state_path, oversized).expect("write oversized state with partial UTF-8 tail");

    append_hook_event_state(&project_root, &decision("current", 1)).expect("append event");

    let content = fs::read_to_string(&state_path).expect("read compacted state");
    let lines = content.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 2, "{content}");
    for line in &lines {
        serde_json::from_str::<Value>(line).expect("compacted event line should be valid JSON");
    }
    let latest =
        serde_json::from_str::<Value>(lines.last().expect("latest event")).expect("latest JSON");
    assert_eq!(latest["subject"]["paths"][0], "current_event_state_1.rs");

    fs::remove_dir_all(&project_root).ok();
}

#[test]
fn empty_hook_event_state_recovers_on_next_append() {
    let project_root = unique_project_root();
    let state_home = unique_state_home(&project_root);
    let _state_home_guard = AspStateHomeGuard::activate(state_home);
    let state_path =
        append_hook_event_state(&project_root, &decision("seed", 0)).expect("seed event");
    fs::write(&state_path, "").expect("empty state");

    append_hook_event_state(&project_root, &decision("recovered", 1)).expect("append event");

    let content = fs::read_to_string(&state_path).expect("read recovered state");
    let lines = content.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 1, "{content}");
    let event = serde_json::from_str::<Value>(lines[0]).expect("event line should be valid JSON");
    assert_eq!(event["subject"]["paths"][0], "recovered_event_state_1.rs");

    fs::remove_dir_all(&project_root).ok();
}

fn unique_project_root() -> PathBuf {
    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    let unique = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    let project_root = std::env::temp_dir().join(format!(
        "asp-hook-event-state-{}-{timestamp}-{unique}",
        std::process::id(),
    ));
    fs::create_dir_all(&project_root).expect("temp project root should be created");
    fs::create_dir_all(project_root.join(".git")).expect("temp git marker should be created");
    project_root
}

fn unique_state_home(project_root: &std::path::Path) -> PathBuf {
    project_root.join(".agent-semantic-protocols-test-state")
}

pub(super) struct AspStateHomeGuard {
    _guard: MutexGuard<'static, ()>,
    previous: Option<OsString>,
}

impl AspStateHomeGuard {
    pub(super) fn activate_isolated() -> Self {
        let guard = crate::test_process_env::ASP_STATE_HOME_ENV_LOCK
            .lock()
            .expect("lock ASP_STATE_HOME test environment");
        let previous = std::env::var_os("ASP_STATE_HOME");
        let state_home = std::env::temp_dir().join(format!(
            "asp-hook-drift-state-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock after unix epoch")
                .as_nanos()
        ));
        unsafe {
            std::env::set_var("ASP_STATE_HOME", state_home);
        }
        Self {
            _guard: guard,
            previous,
        }
    }

    pub(super) fn activate(path: PathBuf) -> Self {
        let guard = crate::test_process_env::ASP_STATE_HOME_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = std::env::var_os("ASP_STATE_HOME");
        unsafe {
            std::env::set_var("ASP_STATE_HOME", path);
        }
        Self {
            _guard: guard,
            previous,
        }
    }
}

impl Drop for AspStateHomeGuard {
    fn drop(&mut self) {
        unsafe {
            if let Some(previous) = &self.previous {
                std::env::set_var("ASP_STATE_HOME", previous);
            } else {
                std::env::remove_var("ASP_STATE_HOME");
            }
        }
    }
}

fn decision(run_id: &str, index: usize) -> HookDecision {
    HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: "codex".to_string(),
        event: "pre-tool".to_string(),
        decision: DecisionKind::Deny,
        reason_kind: ReasonKind::RegisteredSourceRouteRequired,
        language_ids: vec!["rust".into()],
        subject: DecisionSubject {
            tool_name: Some("Read".to_string()),
            command: None,
            paths: vec![format!("{run_id}_event_state_{index}.rs")],
        },
        routes: vec![DecisionRoute {
            language_id: "rust".into(),
            provider_id: "asp-rust".into(),
            binary: "asp".to_string(),
            kind: DecisionRouteKind::Query,
            argv: vec!["asp".to_string(), "rust".to_string()],
            stdin_mode: Some(StdinMode::None),
        }],
        message: "read Rust source through asp query".to_string(),
        fields: BTreeMap::new(),
    }
}

#[test]
fn registered_agent_dispatch_requires_complete_canonical_fields() {
    let mut decision = decision("dispatch-registered-agent", 0);
    assert!(!decision.has_registered_agent_dispatch());

    insert_registered_agent_dispatch(&mut decision);
    assert!(decision.has_registered_agent_dispatch());
    let serialized = serde_json::to_value(&decision).expect("serialize configured dispatch");
    assert!(
        serialized.get("interactiveCommand").is_none(),
        "configured dispatch must not synthesize a Rust-owned ChoicePlane"
    );

    decision.fields.insert(
        "receiptKind".to_string(),
        serde_json::Value::String(String::new()),
    );
    assert!(!decision.has_registered_agent_dispatch());
}

fn insert_registered_agent_dispatch(decision: &mut HookDecision) {
    for (field, value) in [
        ("agentSessionAction", "dispatch-registered-agent"),
        ("transport", "host-agent"),
        ("receiptKind", "asp-testing-execution-v1"),
        ("targetAgent", "asp_testing"),
        ("configRuleId", "testing-role-dispatch"),
        ("commandDigest", "sha256:test-command"),
        ("sessionId", "root-session-test"),
    ] {
        decision.fields.insert(
            field.to_string(),
            serde_json::Value::String(value.to_string()),
        );
    }
}

#[test]
fn latest_session_route_is_read_only_and_config_selected() {
    let _state_home = AspStateHomeGuard::activate_isolated();
    let project_root = unique_project_root();
    let mut unrelated = decision("unrelated-route", 0);
    insert_registered_agent_dispatch(&mut unrelated);
    unrelated.fields.insert(
        "sessionId".to_string(),
        Value::String("another-root".to_string()),
    );
    append_hook_event_state(&project_root, &unrelated).expect("append unrelated route");

    let mut selected = decision("selected-route", 1);
    insert_registered_agent_dispatch(&mut selected);
    for preselected_field in [
        "transport",
        "residentName",
        "targetAgentName",
        "targetAgent",
        "agentSessionAction",
        "receiptKind",
    ] {
        selected.fields.remove(preselected_field);
    }
    selected.fields.insert(
        "agentWindowCommand".to_owned(),
        Value::String("asp session --agents choice-plane".to_owned()),
    );
    selected.fields.insert(
        "choicePlaneOwner".to_owned(),
        Value::String("org-contract:agent-interactive".to_owned()),
    );
    selected.fields.insert(
        "denyEvidenceRef".to_owned(),
        Value::String("/tmp/typed-deny-evidence.jsonl".to_owned()),
    );
    selected.subject.command = Some("cargo test".to_owned());
    append_hook_event_state(&project_root, &selected).expect("append selected route");

    let route = latest_hook_session_agent_route(&project_root)
        .expect("read route")
        .expect("configured route");
    assert_eq!(route.command_digest.as_deref(), Some("sha256:test-command"));
    assert_eq!(route.config_rule_id, "testing-role-dispatch");
    assert_eq!(route.root_session_id, "root-session-test");
    assert_eq!(route.subject_command.as_deref(), Some("cargo test"));
    assert_eq!(
        route.deny_evidence_ref.as_deref(),
        Some("/tmp/typed-deny-evidence.jsonl")
    );

    fs::remove_dir_all(project_root).ok();
}

#[test]
fn current_root_route_never_falls_back_to_a_newer_different_session() {
    let _state_home = AspStateHomeGuard::activate_isolated();
    let project_root = unique_project_root();
    for (rule, root) in [
        ("current-route", "current-root"),
        ("stale-route", "other-root"),
    ] {
        let mut selected = decision(rule, 1);
        insert_registered_agent_dispatch(&mut selected);
        selected
            .fields
            .insert("configRuleId".to_owned(), Value::String(rule.to_owned()));
        selected.fields.insert(
            "agentWindowCommand".to_owned(),
            Value::String("asp session --agents choice-plane".to_owned()),
        );
        selected.fields.insert(
            "choicePlaneOwner".to_owned(),
            Value::String("org-contract:agent-interactive".to_owned()),
        );
        selected.fields.insert(
            "hostRootSessionId".to_owned(),
            Value::String(root.to_owned()),
        );
        append_hook_event_state(&project_root, &selected).expect("append route");
    }

    let route = latest_hook_session_agent_route_for_root(&project_root, Some("current-root"))
        .expect("read current root route")
        .expect("current root route");
    assert_eq!(route.config_rule_id, "current-route");
    assert_eq!(route.root_session_id, "current-root");
    assert!(
        latest_hook_session_agent_route_for_root(&project_root, Some("missing-root"))
            .expect("read missing root route")
            .is_none()
    );
    fs::remove_dir_all(project_root).ok();
}

#[test]
fn source_access_replay_preserves_configured_resident_dispatch() {
    let _state_home = AspStateHomeGuard::activate_isolated();
    let project_root = unique_project_root();
    let mut decision = decision("configured-resident-replay", 0);
    let original_message = "Route the exact command to ASP Testing.".to_string();
    decision.message = original_message.clone();
    insert_registered_agent_dispatch(&mut decision);

    assert!(
        !agent_semantic_hook::apply_repeated_deny_replay(&project_root, &mut decision).unwrap()
    );
    assert_eq!(decision.message, original_message);
    assert_eq!(
        decision
            .fields
            .get("denyReplayMessagePolicy")
            .and_then(serde_json::Value::as_str),
        Some("preserve-parser-route")
    );
    assert!(!decision.fields.contains_key("completionReceipt"));

    fs::remove_dir_all(project_root).ok();
}

#[test]
fn source_access_replay_preserves_exact_parser_route_message() {
    let _state_home = AspStateHomeGuard::activate_isolated();
    let project_root = unique_project_root();
    let mut first = decision("parser-route-replay", 0);
    first.message = "Use parser evidence. ASP route: asp rust search owner src/lib.rs".to_string();
    let original_message = first.message.clone();

    assert!(!agent_semantic_hook::apply_repeated_deny_replay(&project_root, &mut first).unwrap());
    append_hook_event_state(&project_root, &first).expect("record first parser route denial");

    let mut repeated = decision("parser-route-replay", 0);
    repeated.message = original_message.clone();
    assert!(agent_semantic_hook::apply_repeated_deny_replay(&project_root, &mut repeated).unwrap());
    assert_eq!(repeated.message, original_message);
    assert_eq!(
        repeated
            .fields
            .get("denyReplayMessagePolicy")
            .and_then(Value::as_str),
        Some("preserve-parser-route")
    );

    fs::remove_dir_all(project_root).ok();
}

#[test]
fn source_access_replay_key_does_not_collapse_distinct_source_owners() {
    let _state_home = AspStateHomeGuard::activate_isolated();
    let project_root = unique_project_root();
    let mut rust = decision("distinct-source-owner", 0);
    assert!(!agent_semantic_hook::apply_repeated_deny_replay(&project_root, &mut rust).unwrap());
    append_hook_event_state(&project_root, &rust).expect("record Rust source denial");

    let mut python = decision("distinct-source-owner", 1);
    python.language_ids = vec!["python".into()];
    python.routes[0].language_id = "python".into();
    python.routes[0].provider_id = "asp-python".into();
    python.routes[0].argv = vec!["asp".to_string(), "python".to_string()];
    assert!(
        !agent_semantic_hook::apply_repeated_deny_replay(&project_root, &mut python).unwrap(),
        "a different language/path/route must start a distinct replay lane"
    );

    fs::remove_dir_all(project_root).ok();
}

fn lifecycle_decision(event: &str, session_id: &str, transcript_path: &str) -> HookDecision {
    let mut fields = BTreeMap::new();
    fields.insert(
        "sessionId".to_string(),
        Value::String(session_id.to_string()),
    );
    fields.insert(
        "transcriptPath".to_string(),
        Value::String(transcript_path.to_string()),
    );
    HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: "codex".to_string(),
        event: event.to_string(),
        decision: DecisionKind::Allow,
        reason_kind: ReasonKind::None,
        language_ids: Vec::new(),
        subject: DecisionSubject::default(),
        routes: Vec::new(),
        message: String::new(),
        fields,
    }
}
