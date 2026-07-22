use std::collections::{BTreeMap, HashSet};
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_hook::{
    DecisionKind, DecisionRoute, DecisionRouteKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID,
    HOOK_DECISION_SCHEMA_VERSION, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision,
    ReasonKind, StdinMode, append_hook_event_state, has_recorded_subagent_context,
};
use serde_json::Value;

static ASP_STATE_HOME_ENV_LOCK: Mutex<()> = Mutex::new(());

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
        assert_eq!(event["reasonKind"], "direct-source-read");
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
        has_recorded_subagent_context(&project_root, Some(session_id), None)
            .expect("lookup by session")
    );
    assert!(
        has_recorded_subagent_context(&project_root, None, Some(transcript_path))
            .expect("lookup by transcript")
    );

    append_hook_event_state(
        &project_root,
        &lifecycle_decision("subagent-stop", session_id, transcript_path),
    )
    .expect("record subagent stop");

    assert!(
        !has_recorded_subagent_context(&project_root, Some(session_id), Some(transcript_path))
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
        let guard = ASP_STATE_HOME_ENV_LOCK
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
        let guard = ASP_STATE_HOME_ENV_LOCK
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
        reason_kind: ReasonKind::DirectSourceRead,
        language_ids: vec!["rust".to_string()],
        subject: DecisionSubject {
            tool_name: Some("Read".to_string()),
            command: None,
            paths: vec![format!("{run_id}_event_state_{index}.rs")],
        },
        routes: vec![DecisionRoute {
            language_id: "rust".to_string(),
            provider_id: "rs-harness".to_string(),
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
fn configured_resident_dispatch_requires_complete_canonical_fields() {
    let mut decision = decision("configured-resident-dispatch", 0);
    assert!(!decision.has_configured_resident_dispatch());

    insert_configured_resident_dispatch(&mut decision);
    assert!(decision.has_configured_resident_dispatch());
    let command = decision
        .configured_resident_interactive_command()
        .expect("configured resident command");
    assert_eq!(
        command.argv,
        [
            "asp",
            "agent",
            "session",
            "bootstrap",
            "--name",
            "asp-testing",
            "--root-session-id",
            "root-session-test",
        ]
    );
    assert_eq!(
        decision
            .configured_resident_interactive_command_line()
            .as_deref(),
        Some("asp agent session bootstrap --name asp-testing --root-session-id root-session-test")
    );

    decision.fields.insert(
        "receiptKind".to_string(),
        serde_json::Value::String(String::new()),
    );
    assert!(!decision.has_configured_resident_dispatch());
}

fn insert_configured_resident_dispatch(decision: &mut HookDecision) {
    for (field, value) in [
        ("agentSessionAction", "dispatch-configured-resident"),
        ("transport", "resident-agent"),
        ("residentName", "asp-testing"),
        ("receiptKind", "asp-testing-execution-v1"),
        ("targetAgentName", "asp_testing"),
        ("sessionId", "root-session-test"),
    ] {
        decision.fields.insert(
            field.to_string(),
            serde_json::Value::String(value.to_string()),
        );
    }
}

#[test]
fn source_access_replay_preserves_configured_resident_dispatch() {
    let _state_home = AspStateHomeGuard::activate_isolated();
    let project_root = unique_project_root();
    let mut decision = decision("configured-resident-replay", 0);
    let original_message = "Route the exact command to ASP Testing.".to_string();
    decision.message = original_message.clone();
    insert_configured_resident_dispatch(&mut decision);

    assert!(
        !agent_semantic_hook::apply_repeated_deny_replay(&project_root, &mut decision).unwrap()
    );
    assert_eq!(decision.message, original_message);
    assert_eq!(
        decision
            .fields
            .get("denyReplayMessagePolicy")
            .and_then(serde_json::Value::as_str),
        Some("preserve-agent-session-route")
    );
    assert!(!decision.fields.contains_key("completionReceipt"));

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
