use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_hook::host_native_handoff::{
    HookHostNativeHandoffEvaluation, argv_digest, evaluate_hook_phase,
    publish_from_post_tool_payload,
};
use serde_json::{Value, json};

fn environment_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct ScenarioRoot {
    path: PathBuf,
    prior_state_home: Option<std::ffi::OsString>,
}

impl ScenarioRoot {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "asp-hook-host-native-handoff-{}-{label}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(path.join("workspace")).expect("scenario workspace");
        let prior_state_home = std::env::var_os("ASP_STATE_HOME");
        // SAFETY: all tests that mutate ASP_STATE_HOME hold environment_lock.
        unsafe { std::env::set_var("ASP_STATE_HOME", path.join("state")) };
        Self {
            path,
            prior_state_home,
        }
    }

    fn workspace(&self) -> String {
        self.path
            .join("workspace")
            .canonicalize()
            .expect("canonical workspace")
            .to_string_lossy()
            .into_owned()
    }
}

impl Drop for ScenarioRoot {
    fn drop(&mut self) {
        match self.prior_state_home.take() {
            Some(value) => {
                // SAFETY: all tests that mutate ASP_STATE_HOME hold environment_lock.
                unsafe { std::env::set_var("ASP_STATE_HOME", value) }
            }
            None => {
                // SAFETY: all tests that mutate ASP_STATE_HOME hold environment_lock.
                unsafe { std::env::remove_var("ASP_STATE_HOME") }
            }
        }
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn deferred_receipt(argv: &[String]) -> Value {
    json!({
        "schemaId": "agent.semantic-protocols.host-native-execution-required",
        "schemaVersion": "1",
        "state": "deferred",
        "reasonKind": "host-local-ipc-permission-denied",
        "executionAuthority": "host-native",
        "retryPolicy": "do-not-retry-in-current-sandbox",
        "argv": argv,
        "commandDigest": argv_digest(argv).expect("argv digest"),
        "message": "execute with Host-native authority",
        "cause": "Operation not permitted",
    })
}

fn post_tool_payload(workspace: &str, root: &str, command: &str) -> Value {
    let argv = vec![
        "asp".to_owned(),
        "live-corpus".to_owned(),
        "qualify".to_owned(),
    ];
    json!({
        "hook_event_name": "PostToolUse",
        "cwd": workspace,
        "session_id": "testing-thread",
        "root_session_id": root,
        "parent_session_id": root,
        "child_session_id": "testing-thread",
        "agent_id": "testing-thread",
        "agent_role": "asp_testing",
        "tool_name": "Bash",
        "tool_input": { "command": command },
        "tool_response": {
            "exit_code": 2,
            "output": deferred_receipt(&argv).to_string()
        }
    })
}

fn hook_phase_payload(workspace: &str, root: &str, command: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "hook_event_name": "PreToolUse",
        "cwd": workspace,
        "session_id": root,
        "turn_id": "host-turn",
        "tool_use_id": "host-tool-use",
        "tool_name": "Bash",
        "tool_input": { "command": command }
    }))
    .expect("pre-tool payload")
}

#[test]
fn verified_testing_deferred_receipt_is_same_root_one_shot_authority() {
    let _environment = environment_lock().lock().expect("environment lock");
    let scenario = ScenarioRoot::new("one-shot");
    let workspace = scenario.workspace();
    let command = "asp live-corpus qualify";
    let capability = publish_from_post_tool_payload(&post_tool_payload(
        &workspace,
        "root-thread",
        "rtk --ultra-compact err asp live-corpus qualify",
    ))
    .expect("publish")
    .expect("deferred receipt");
    assert_eq!(capability.receipt_kind, "asp-testing-execution-v1");

    assert!(matches!(
        evaluate_hook_phase(
            &hook_phase_payload(&workspace, "root-thread", command),
            "pre-tool"
        ),
        HookHostNativeHandoffEvaluation::Authorized(_)
    ));
    assert!(matches!(
        evaluate_hook_phase(
            &hook_phase_payload(&workspace, "root-thread", command),
            "pre-tool"
        ),
        HookHostNativeHandoffEvaluation::NotRequested
    ));
    let mut wrong_turn: Value =
        serde_json::from_slice(&hook_phase_payload(&workspace, "root-thread", command))
            .expect("permission payload");
    wrong_turn["turn_id"] = Value::String("other-turn".to_owned());
    assert!(matches!(
        evaluate_hook_phase(
            &serde_json::to_vec(&wrong_turn).expect("wrong-turn payload"),
            "permission-request"
        ),
        HookHostNativeHandoffEvaluation::Rejected(_)
    ));
    assert!(matches!(
        evaluate_hook_phase(
            &hook_phase_payload(&workspace, "root-thread", command),
            "permission-request"
        ),
        HookHostNativeHandoffEvaluation::Authorized(_)
    ));
    assert!(matches!(
        evaluate_hook_phase(
            &hook_phase_payload(&workspace, "root-thread", command),
            "permission-request"
        ),
        HookHostNativeHandoffEvaluation::NotRequested
    ));
}

#[test]
fn different_root_cannot_consume_testing_handoff() {
    let _environment = environment_lock().lock().expect("environment lock");
    let scenario = ScenarioRoot::new("wrong-root");
    let workspace = scenario.workspace();
    let command = "asp live-corpus qualify";
    publish_from_post_tool_payload(&post_tool_payload(&workspace, "root-thread", command))
        .expect("publish")
        .expect("deferred receipt");
    assert!(matches!(
        evaluate_hook_phase(
            &hook_phase_payload(&workspace, "other-root", command),
            "pre-tool"
        ),
        HookHostNativeHandoffEvaluation::NotRequested
    ));
}

#[test]
fn forged_deferred_command_digest_is_rejected_before_publication() {
    let _environment = environment_lock().lock().expect("environment lock");
    let scenario = ScenarioRoot::new("forged-digest");
    let workspace = scenario.workspace();
    let mut payload = post_tool_payload(&workspace, "root-thread", "asp live-corpus qualify");
    let forged = payload
        .pointer_mut("/tool_response/output")
        .and_then(|value| value.as_str())
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
        .map(|mut receipt| {
            receipt["commandDigest"] = Value::String(format!("blake3-256:{}", "0".repeat(64)));
            receipt.to_string()
        })
        .expect("forged receipt");
    payload["tool_response"]["output"] = Value::String(forged);
    let error = publish_from_post_tool_payload(&payload).expect_err("forged digest must fail");
    assert!(error.contains("command digest mismatch"));
}
