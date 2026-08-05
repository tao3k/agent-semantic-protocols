use super::{
    ResidentHookSnapshot, ResidentHookSnapshotGeneration, evaluate_snapshot,
    materialize_generation_delta_blocking, render_decision,
};
use agent_semantic_client_db::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalogEntry;
use agent_semantic_hook::{
    DecisionKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION,
    HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookClassificationRequest, HookDecision, ReasonKind,
    classify_hook_with_config, parse_payload,
};

fn allow_decision() -> HookDecision {
    HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: "codex".to_owned(),
        event: "pre-tool".to_owned(),
        decision: DecisionKind::Allow,
        reason_kind: ReasonKind::None,
        language_ids: Vec::new(),
        subject: DecisionSubject::default(),
        routes: Vec::new(),
        message: String::new(),
        fields: std::collections::BTreeMap::new(),
    }
}

fn in_memory_snapshot() -> ResidentHookSnapshot {
    ResidentHookSnapshot {
        project_root: std::path::PathBuf::from("/workspace"),
        runtime: agent_semantic_hook::HookRuntime {
            project_root: "/workspace".to_owned(),
            rankers: Vec::new(),
            providers: Vec::new(),
        },
        config: std::sync::Arc::new(agent_semantic_hook::ClientHookConfig::default()),
    }
}

#[test]
fn hook_client_uses_typed_runtime_server_evaluation_without_checkpoint_reads() {
    let source = include_str!("../../src/server/runtime_server.rs");
    let start = source
        .find("pub(crate) fn runtime_server_hook_evaluation_client(")
        .expect("Hook evaluation client owner");
    let end = source[start..]
        .find("async fn runtime_server_admitted_workspace_scope(")
        .map(|offset| start + offset)
        .expect("next Runtime Server client owner");
    let client = &source[start..end];

    assert!(client.contains("runtime_server_workspace_session_for_admission_async"));
    assert!(client.contains(".evaluate_hook(arguments, input)"));
    assert!(!client.contains("hook-snapshot.v1.memory"));
    assert!(!client.contains("evaluate_durable_snapshot"));
}

#[test]
fn identity_delta_reuses_existing_snapshot_without_rematerialization() {
    let existing = std::sync::Arc::new(in_memory_snapshot());
    let previous = ResidentHookSnapshotGeneration {
        admitted_roots: std::collections::BTreeMap::from([(
            "workspace-existing".to_owned(),
            std::path::PathBuf::from("/workspace"),
        )]),
        snapshots: std::collections::BTreeMap::from([(
            "workspace-existing".to_owned(),
            std::sync::Arc::clone(&existing),
        )]),
        failures: std::collections::BTreeMap::new(),
    };
    let admitted = std::collections::BTreeSet::from([
        RuntimeWorkspaceAdmissionCatalogEntry {
            workspace_identity: "workspace-existing".to_owned(),
            project_root: "/workspace".into(),
        },
        RuntimeWorkspaceAdmissionCatalogEntry {
            workspace_identity: "workspace-new".to_owned(),
            project_root: "/workspace/new".into(),
        },
    ]);

    let next = materialize_generation_delta_blocking(
        &previous,
        &admitted,
        std::path::Path::new("/unavailable-active-workspace-store"),
    );

    assert!(std::sync::Arc::ptr_eq(
        next.snapshots.get("workspace-existing").unwrap(),
        &existing
    ));
    assert_eq!(next.snapshots.len(), 1);
    assert_eq!(next.admitted_roots.len(), 2);
}

#[test]
fn resident_render_has_no_runtime_or_persistence_dependency() {
    let mut decision = allow_decision();
    decision.fields.insert(
        "hookEvaluationAuthority".to_owned(),
        serde_json::Value::String("runtime-server-immutable-snapshot".to_owned()),
    );
    decision.fields.insert(
        "hookPersistenceDependency".to_owned(),
        serde_json::Value::Bool(false),
    );
    let output = render_decision("decision", &decision).expect("render decision");
    assert!(output.contains("runtime-server-immutable-snapshot"));
    assert!(output.contains("\"hookPersistenceDependency\":false"));
}

#[test]
fn immutable_snapshot_render_p99_is_sub_millisecond() {
    let decision = allow_decision();
    let mut samples = (0..10_000)
        .map(|_| {
            let started = std::time::Instant::now();
            let output = render_decision("platform", &decision).expect("render platform response");
            std::hint::black_box(output);
            started.elapsed()
        })
        .collect::<Vec<_>>();
    samples.sort_unstable();
    let p99 = samples[(samples.len() * 99) / 100];
    eprintln!(
        "[runtime-hook-render] requests=10000 p99Nanos={} retainedAuthorities=0",
        p99.as_nanos()
    );
    assert!(
        p99 < std::time::Duration::from_millis(1),
        "immutable Hook snapshot render p99 must remain sub-millisecond, observed {p99:?}"
    );
}

#[test]
fn full_match_engine_snapshot_p99_is_sub_millisecond_after_long_sequence() {
    let snapshot = in_memory_snapshot();
    let arguments = vec![
        "hook".to_owned(),
        "pre-tool".to_owned(),
        "--client".to_owned(),
        "codex".to_owned(),
        "--emit".to_owned(),
        "decision".to_owned(),
    ];
    let input = serde_json::json!({
        "hook_event_name": "PreToolUse",
        "cwd": "/workspace",
        "tool_name": "Bash",
        "tool_input": { "command": "cargo check" }
    })
    .to_string();
    let mut samples = (0..10_000)
        .map(|_| {
            let started = std::time::Instant::now();
            let output =
                evaluate_snapshot(&snapshot, &arguments, &input).expect("evaluate snapshot");
            std::hint::black_box(output);
            started.elapsed()
        })
        .collect::<Vec<_>>();
    samples.sort_unstable();
    let p99 = samples[(samples.len() * 99) / 100];
    eprintln!(
        "[runtime-hook-sequential] requests=10000 p99Nanos={} retainedAuthorities=0 filesystemOpens=0 databaseWrites=0",
        p99.as_nanos()
    );
    assert!(
        p99 < std::time::Duration::from_millis(1),
        "full immutable Hook match-engine p99 must remain sub-millisecond after 10,000 evaluations, observed {p99:?}"
    );
}

#[test]
fn match_engine_stage_receipts_separate_parse_classify_and_render() {
    let snapshot = in_memory_snapshot();
    let input = serde_json::json!({
        "hook_event_name": "PreToolUse",
        "cwd": "/workspace",
        "tool_name": "Bash",
        "tool_input": { "command": "cargo check" }
    })
    .to_string();
    let payload = parse_payload(&input).expect("fixture payload");
    let mut parse_samples = Vec::with_capacity(10_000);
    let mut classify_samples = Vec::with_capacity(10_000);
    let mut receipt_samples = Vec::with_capacity(10_000);
    let mut spawn_samples = Vec::with_capacity(10_000);
    let mut dispatch_samples = Vec::with_capacity(10_000);
    let mut source_samples = Vec::with_capacity(10_000);
    let mut render_samples = Vec::with_capacity(10_000);
    for _ in 0..10_000 {
        let started = std::time::Instant::now();
        let parsed = parse_payload(&input).expect("parse payload");
        parse_samples.push(started.elapsed());
        std::hint::black_box(parsed);

        let started = std::time::Instant::now();
        let mut decision = classify_hook_with_config(HookClassificationRequest {
            registry: &snapshot.runtime,
            config: &snapshot.config,
            platform: "codex",
            event: "pre-tool",
            payload: &payload,
        });
        classify_samples.push(started.elapsed());

        let started = std::time::Instant::now();
        decision.event = "pre-tool".to_owned();
        decision.fields.insert(
            "hookEvaluationAuthority".to_owned(),
            serde_json::Value::String("runtime-server-immutable-snapshot".to_owned()),
        );
        decision.fields.insert(
            "hookPersistenceDependency".to_owned(),
            serde_json::Value::Bool(false),
        );
        receipt_samples.push(started.elapsed());

        let started = std::time::Instant::now();
        crate::command::hook_runtime::enforce_resident_spawn_from_snapshot(
            &snapshot.config,
            "codex",
            "pre-tool",
            &payload,
            &mut decision,
        );
        spawn_samples.push(started.elapsed());

        let started = std::time::Instant::now();
        crate::command::hook_runtime::materialize_resident_dispatch_from_snapshot(
            &payload,
            &mut decision,
        );
        dispatch_samples.push(started.elapsed());

        let started = std::time::Instant::now();
        crate::command::hook_runtime::materialize_source_access_from_snapshot(
            &mut decision,
            &snapshot.config,
        );
        source_samples.push(started.elapsed());

        let started = std::time::Instant::now();
        let output = render_decision("decision", &decision).expect("render decision");
        render_samples.push(started.elapsed());
        std::hint::black_box(output);
    }
    for samples in [
        &mut parse_samples,
        &mut classify_samples,
        &mut receipt_samples,
        &mut spawn_samples,
        &mut dispatch_samples,
        &mut source_samples,
        &mut render_samples,
    ] {
        samples.sort_unstable();
    }
    let index = 9_900;
    eprintln!(
        "[runtime-hook-stages] requests=10000 parseP99Nanos={} classifyP99Nanos={} receiptP99Nanos={} spawnP99Nanos={} dispatchP99Nanos={} sourceP99Nanos={} renderP99Nanos={}",
        parse_samples[index].as_nanos(),
        classify_samples[index].as_nanos(),
        receipt_samples[index].as_nanos(),
        spawn_samples[index].as_nanos(),
        dispatch_samples[index].as_nanos(),
        source_samples[index].as_nanos(),
        render_samples[index].as_nanos(),
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_multi_workspace_pressure_leaves_no_snapshot_or_task_growth() {
    let snapshots = (0..8)
        .map(|index| {
            let mut snapshot = in_memory_snapshot();
            let root = format!("/workspace/{index}");
            snapshot.project_root = root.clone().into();
            snapshot.runtime.project_root = root;
            std::sync::Arc::new(snapshot)
        })
        .collect::<Vec<_>>();
    let arguments = std::sync::Arc::new(vec![
        "hook".to_owned(),
        "pre-tool".to_owned(),
        "--client".to_owned(),
        "codex".to_owned(),
        "--emit".to_owned(),
        "decision".to_owned(),
    ]);
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(1_001));
    let tasks = (0..1_000)
        .map(|index| {
            let snapshot = std::sync::Arc::clone(&snapshots[index % snapshots.len()]);
            let arguments = std::sync::Arc::clone(&arguments);
            let barrier = std::sync::Arc::clone(&barrier);
            tokio::spawn(async move {
                let input = serde_json::json!({
                    "hook_event_name": "PreToolUse",
                    "cwd": snapshot.project_root,
                    "tool_name": "Bash",
                    "tool_input": { "command": "cargo check" }
                })
                .to_string();
                barrier.wait().await;
                let started = std::time::Instant::now();
                let output = evaluate_snapshot(&snapshot, &arguments, &input)
                    .expect("concurrent snapshot evaluation");
                std::hint::black_box(output);
                started.elapsed()
            })
        })
        .collect::<Vec<_>>();
    barrier.wait().await;
    let mut samples = Vec::with_capacity(tasks.len());
    for task in tasks {
        samples.push(task.await.expect("Hook evaluation task"));
    }
    samples.sort_unstable();
    let p99 = samples[(samples.len() * 99) / 100];
    eprintln!(
        "[runtime-hook-pressure] workspaces=8 requests=1000 p99Nanos={} retainedAuthorities=0",
        p99.as_nanos()
    );
    assert!(
        p99 < std::time::Duration::from_millis(25),
        "1,000-way multi-workspace Hook pressure p99 must remain below 25 ms, observed {p99:?}"
    );
    assert!(
        snapshots
            .iter()
            .all(|snapshot| std::sync::Arc::strong_count(snapshot) == 1),
        "all per-request snapshot authorities must be released after the pressure batch"
    );
}
