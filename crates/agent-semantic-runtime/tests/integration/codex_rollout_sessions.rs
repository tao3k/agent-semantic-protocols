use std::{
    env, fs,
    sync::Mutex,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use agent_semantic_runtime::{
    codex_rollout_session_index_for_sessions, codex_rollout_session_metadata,
};

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn codex_rollout_session_metadata_uses_targeted_session_lookup() {
    let _env_guard = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let session_id = "019f2dc6-3ed6-73b3-809d-62c4a3802ffb";
    let root_session_id = "019f1f1a-5389-7223-a150-77dcb5ea8dd4";
    let root = temp_codex_home("codex-rollout-targeted-lookup");
    let rollout_dir = root.join("sessions").join("2026").join("07").join("04");
    fs::create_dir_all(&rollout_dir).expect("create rollout dir");
    let rollout_path = rollout_dir.join(format!("rollout-2026-07-04T08-36-35-{session_id}.jsonl"));
    fs::write(
        &rollout_path,
        format!(
            r#"{{"type":"session_meta","payload":{{"id":"{session_id}","session_id":"{root_session_id}","parent_thread_id":"{root_session_id}","thread_source":"subagent","agent_role":"asp_explorer","source":{{"subagent":{{"thread_spawn":{{"parent_thread_id":"{root_session_id}","agent_role":"asp_explorer","agent_nickname":"ASP selector","depth":1}}}}}},"model_provider":"openai","cli_version":"0.142.5","cwd":"/tmp/project"}}}}
{{"type":"turn_context","payload":{{"model":"gpt-5.4-mini","sandbox_policy":{{"type":"danger-full-access"}},"approval_policy":"never","permission_profile":{{"type":"disabled"}}}}}}
"#
        ),
    )
    .expect("write rollout");

    let previous_codex_home = env::var_os("CODEX_HOME");
    unsafe {
        env::set_var("CODEX_HOME", &root);
    }
    let timer = Timer::start();
    let metadata = codex_rollout_session_metadata(session_id)
        .expect("lookup metadata")
        .expect("metadata hit");
    let elapsed = timer.elapsed();
    restore_codex_home(previous_codex_home);
    fs::remove_dir_all(&root).ok();

    assert_eq!(metadata.session_id, session_id);
    assert_eq!(metadata.root_session_id.as_deref(), Some(root_session_id));
    assert_eq!(metadata.parent_thread_id.as_deref(), Some(root_session_id));
    assert_eq!(metadata.agent_role.as_deref(), Some("asp_explorer"));
    assert!(
        elapsed <= Duration::from_millis(5),
        "targeted rollout lookup exceeded the 5ms gate: {elapsed:?}"
    );
}

#[test]
fn codex_rollout_session_metadata_stays_header_bounded_for_resident_history() {
    let _env_guard = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let session_id = "019f2dc6-3ed6-73b3-809d-62c4a3802ffb";
    let root_session_id = "019f1f1a-5389-7223-a150-77dcb5ea8dd4";
    let root = temp_codex_home("codex-rollout-resident-history");
    let rollout_dir = root.join("sessions").join("2026").join("07").join("04");
    fs::create_dir_all(&rollout_dir).expect("create rollout dir");
    let rollout_path = rollout_dir.join(format!("rollout-2026-07-04T08-36-35-{session_id}.jsonl"));
    let mut rollout = format!(
        r#"{{"type":"session_meta","payload":{{"id":"{session_id}","session_id":"{root_session_id}","parent_thread_id":"{root_session_id}","thread_source":"subagent","agent_role":"asp_explorer","source":{{"subagent":{{"thread_spawn":{{"parent_thread_id":"{root_session_id}","agent_role":"asp_explorer","agent_nickname":"ASP selector","depth":1}}}}}},"model_provider":"openai","cli_version":"0.142.5","cwd":"/tmp/project"}}}}
{{"type":"turn_context","payload":{{"model":"gpt-5.4-mini","sandbox_policy":{{"type":"danger-full-access"}},"approval_policy":"never","permission_profile":{{"type":"disabled"}}}}}}
"#
    );
    for index in 0..20_000 {
        rollout.push_str(&format!(
            r#"{{"timestamp":"2026-07-04T08:37:00Z","type":"event_msg","payload":{{"type":"agent_message","turn_id":"turn-{index}","message":"resident history line {index}"}}}}
"#
        ));
    }
    fs::write(&rollout_path, rollout).expect("write resident rollout");

    let previous_codex_home = env::var_os("CODEX_HOME");
    unsafe {
        env::set_var("CODEX_HOME", &root);
    }
    let timer = Timer::start();
    let metadata = codex_rollout_session_metadata(session_id)
        .expect("lookup metadata")
        .expect("metadata hit");
    let elapsed = timer.elapsed();
    restore_codex_home(previous_codex_home);
    fs::remove_dir_all(&root).ok();

    assert_eq!(metadata.session_id, session_id);
    assert_eq!(metadata.root_session_id.as_deref(), Some(root_session_id));
    assert_eq!(metadata.model.as_deref(), Some("gpt-5.4-mini"));
    assert!(
        elapsed <= Duration::from_millis(5),
        "resident rollout metadata lookup must stay header-bounded; elapsed={elapsed:?}"
    );
}

#[test]
fn codex_rollout_session_index_uses_direct_pure_rust_session_lookup() {
    let _env_guard = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let root_session_id = "019f2dc6-3ed5-73b3-809d-62c4a3802ffa";
    let child_session_id = "019f2dc6-3ed6-73b3-809d-62c4a3802ffb";
    let root = temp_codex_home("codex-rollout-index-direct-lookup");
    let empty_path = root.join("empty-path");
    let rollout_dir = root.join("sessions").join("2026").join("07").join("04");
    fs::create_dir_all(&empty_path).expect("create empty PATH dir");
    fs::create_dir_all(&rollout_dir).expect("create rollout dir");
    for index in 0..256 {
        fs::write(
            rollout_dir.join(format!("rollout-irrelevant-{index}.jsonl")),
            format!(
                r#"{{"type":"session_meta","payload":{{"id":"irrelevant-{index}","session_id":"irrelevant-root-{index}","parent_thread_id":"irrelevant-parent-{index}"}}}}
"#
            ),
        )
        .expect("write irrelevant rollout");
    }
    fs::write(
        rollout_dir.join(format!(
            "rollout-2026-07-04T08-09-21-{root_session_id}.jsonl"
        )),
        format!(
            r#"{{"type":"session_meta","payload":{{"id":"{root_session_id}","session_id":"{root_session_id}","parent_thread_id":"{root_session_id}","thread_source":"user","agent_role":"user"}}}}
{{"timestamp":"2026-06-29T08:09:21Z","type":"response_item","payload":{{"type":"thread_spawn","id":"{child_session_id}","parent_thread_id":"{root_session_id}","agent_role":"subagent"}}}}
"#
        ),
    )
    .expect("write root rollout");
    fs::write(
        rollout_dir.join(format!(
            "rollout-2026-07-04T08-09-22-{child_session_id}.jsonl"
        )),
        format!(
            r#"{{"type":"session_meta","payload":{{"id":"{child_session_id}","session_id":"{root_session_id}","parent_thread_id":"{root_session_id}","thread_source":"subagent","agent_role":"subagent"}}}}
{{"timestamp":"2026-06-29T08:09:23Z","type":"event_msg","payload":{{"type":"agent_message","turn_id":"turn-child"}}}}
"#
        ),
    )
    .expect("write child rollout");

    let previous_codex_home = env::var_os("CODEX_HOME");
    let previous_path = env::var_os("PATH");
    unsafe {
        env::set_var("CODEX_HOME", &root);
        env::set_var("PATH", &empty_path);
    }
    let child_metadata = codex_rollout_session_metadata(child_session_id)
        .expect("lookup child metadata")
        .expect("child metadata hit");
    assert_eq!(child_metadata.session_id, child_session_id);
    let timer = Timer::start();
    let index = codex_rollout_session_index_for_sessions(root_session_id, [child_session_id])
        .expect("index rollout sessions")
        .expect("index hit");
    let elapsed = timer.elapsed();
    restore_codex_home(previous_codex_home);
    restore_path(previous_path);
    fs::remove_dir_all(&root).ok();

    assert_eq!(index.scanned_rollout_count, 2, "{index:#?}");
    assert_eq!(index.skipped_rollout_count, 0, "{index:#?}");
    assert_eq!(index.activity_by_session.len(), 2, "{index:#?}");
    assert!(index.activity_by_session.contains_key(root_session_id));
    assert!(index.activity_by_session.contains_key(child_session_id));
    assert!(
        elapsed <= Duration::from_millis(10),
        "direct rollout session index exceeded the 10ms algorithm gate: {elapsed:?}"
    );
}

#[test]
fn codex_app_server_runtime_observation_supplies_reasoning_effort() {
    let _env_guard = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let session_id = "019f2dc6-3ed6-73b3-809d-62c4a3802ffb";
    let root_session_id = "019f1f1a-5389-7223-a150-77dcb5ea8dd4";
    let root = temp_codex_home("codex-app-server-runtime-observation");
    let rollout_dir = root.join("sessions").join("2026").join("07").join("04");
    fs::create_dir_all(&rollout_dir).expect("create rollout dir");
    let rollout_path = rollout_dir.join(format!("rollout-2026-07-04T08-36-35-{session_id}.jsonl"));
    fs::write(
        &rollout_path,
        format!(
            r#"{{"type":"session_meta","payload":{{"id":"{session_id}","session_id":"{root_session_id}","parent_thread_id":"{root_session_id}","thread_source":"subagent","agent_role":"asp_explorer","source":{{"subagent":{{"thread_spawn":{{"parent_thread_id":"{root_session_id}","agent_role":"asp_explorer","depth":1}}}}}},"model_provider":"openai","cli_version":"0.142.5","cwd":"/tmp/project"}}}}
{{"type":"turn_context","payload":{{"model":"gpt-5.4-mini"}}}}
"#
        ),
    )
    .expect("write rollout");

    let fake_codex = root.join("fake-codex");
    fs::write(
        &fake_codex,
        format!(
            r#"#!/bin/sh
IFS= read -r initialize
IFS= read -r initialized
IFS= read -r request
input="$initialize$initialized$request"
case "$input" in
  *'"thread/list"'*)
    printf '%s\n' '{{"id":2,"result":{{"data":[{{"id":"{session_id}","parentThreadId":"{root_session_id}","agentRole":"asp_explorer","source":{{"subAgent":{{"threadSpawn":{{"parentThreadId":"{root_session_id}","agentPath":"/root/asp_explorer","depth":1}}}}}}}}]}}}}'
    ;;
  *)
    printf '%s\n' '{{"id":2,"result":{{"thread":{{"id":"{session_id}"}},"model":"gpt-5.4-mini","reasoningEffort":"low"}}}}'
    ;;
esac
"#
        ),
    )
    .expect("write fake codex");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&fake_codex, fs::Permissions::from_mode(0o755))
            .expect("make fake codex executable");
    }

    let previous_codex_home = env::var_os("CODEX_HOME");
    let previous_codex_bin = env::var_os("ASP_CODEX_BIN");
    unsafe {
        env::set_var("CODEX_HOME", &root);
        env::set_var("ASP_CODEX_BIN", &fake_codex);
    }
    let records = agent_semantic_runtime::codex_app_server_child_session_metadata(root_session_id)
        .expect("read app-server child metadata");
    restore_codex_home(previous_codex_home);
    unsafe {
        match previous_codex_bin {
            Some(value) => env::set_var("ASP_CODEX_BIN", value),
            None => env::remove_var("ASP_CODEX_BIN"),
        }
    }
    fs::remove_dir_all(&root).ok();

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].session_id, session_id);
    assert_eq!(records[0].model.as_deref(), Some("gpt-5.4-mini"));
    assert_eq!(records[0].reasoning_effort.as_deref(), Some("low"));
}

#[test]
fn missing_runtime_reasoning_preserves_rollout_reasoning_effort() {
    let _env_guard = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let session_id = "019f2dc6-3ed6-73b3-809d-62c4a3802ffc";
    let root_session_id = "019f1f1a-5389-7223-a150-77dcb5ea8ddc";
    let root = temp_codex_home("codex-runtime-null-preserves-rollout-reasoning");
    let rollout_dir = root.join("sessions").join("2026").join("07").join("04");
    fs::create_dir_all(&rollout_dir).expect("create rollout dir");
    fs::write(
        rollout_dir.join(format!("rollout-2026-07-04T08-36-35-{session_id}.jsonl")),
        format!(
            r#"{{"type":"session_meta","payload":{{"id":"{session_id}","session_id":"{root_session_id}","parent_thread_id":"{root_session_id}","thread_source":"subagent","agent_role":"asp_explorer","source":{{"subagent":{{"thread_spawn":{{"parent_thread_id":"{root_session_id}","agent_role":"asp_explorer","depth":1}}}}}},"model_provider":"openai","cli_version":"0.142.5","cwd":"/tmp/project"}}}}
{{"type":"turn_context","payload":{{"model":"gpt-5.4-mini","reasoning_effort":"low"}}}}
"#
        ),
    )
    .expect("write rollout");

    let fake_codex = root.join("fake-codex");
    fs::write(
        &fake_codex,
        format!(
            r#"#!/bin/sh
IFS= read -r initialize
IFS= read -r initialized
IFS= read -r request
input="$initialize$initialized$request"
case "$input" in
  *'"thread/list"'*)
    printf '%s\n' '{{"id":2,"result":{{"data":[{{"id":"{session_id}","parentThreadId":"{root_session_id}","agentRole":"asp_explorer","source":{{"subAgent":{{"threadSpawn":{{"parentThreadId":"{root_session_id}","agentPath":"/root/asp_explorer","depth":1}}}}}}}}]}}}}'
    ;;
  *)
    printf '%s\n' '{{"id":2,"result":{{"thread":{{"id":"{session_id}"}},"model":"gpt-5.4-mini","reasoningEffort":null}}}}'
    ;;
esac
"#
        ),
    )
    .expect("write fake codex");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&fake_codex, fs::Permissions::from_mode(0o755))
            .expect("make fake codex executable");
    }

    let previous_codex_home = env::var_os("CODEX_HOME");
    let previous_codex_bin = env::var_os("ASP_CODEX_BIN");
    unsafe {
        env::set_var("CODEX_HOME", &root);
        env::set_var("ASP_CODEX_BIN", &fake_codex);
    }
    let records = agent_semantic_runtime::codex_app_server_child_session_metadata(root_session_id)
        .expect("read app-server child metadata");
    restore_codex_home(previous_codex_home);
    unsafe {
        match previous_codex_bin {
            Some(value) => env::set_var("ASP_CODEX_BIN", value),
            None => env::remove_var("ASP_CODEX_BIN"),
        }
    }
    fs::remove_dir_all(&root).ok();

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].reasoning_effort.as_deref(), Some("low"));
}

fn temp_codex_home(label: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_nanos();
    env::temp_dir().join(format!("{label}-{}-{unique}", std::process::id()))
}

struct Timer {
    started_at: Instant,
}

impl Timer {
    fn start() -> Self {
        Self {
            started_at: Instant::now(),
        }
    }

    fn elapsed(&self) -> Duration {
        self.started_at.elapsed()
    }
}

fn restore_codex_home(previous: Option<std::ffi::OsString>) {
    unsafe {
        if let Some(value) = previous {
            env::set_var("CODEX_HOME", value);
        } else {
            env::remove_var("CODEX_HOME");
        }
    }
}

fn restore_path(previous: Option<std::ffi::OsString>) {
    unsafe {
        if let Some(value) = previous {
            env::set_var("PATH", value);
        } else {
            env::remove_var("PATH");
        }
    }
}
