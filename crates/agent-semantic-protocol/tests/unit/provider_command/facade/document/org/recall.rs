use crate::provider_command::support::{asp_command, temp_project_root};
use std::{path::Path, process::Command};

#[test]
fn asp_org_recall_plans_scans_in_rust_and_ranks_with_memory_engine() {
    let root = temp_project_root("org-document-command-recall-plans-rank");
    let org_artifacts = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("artifacts")
        .join("org");
    let plans = org_artifacts.join("flow").join("plans");
    std::fs::create_dir_all(&plans).expect("create plans dir");
    let hot_plan = plans.join("agent-plan-memory-engine-hot-path.org");
    let cold_plan = plans.join("agent-plan-unrelated-cold-path.org");
    std::fs::write(
        &hot_plan,
        "* TODO Stabilize memory engine recall flow [1/8] [12%] :agent:plan:\n:PROPERTIES:\n:CONTRACT_ORG: agent.plan.v1\n:ID: memory-engine-hot-path\n:SESSION_ID: codex-recall-thread\n:OBJECTIVE: Stabilize memory engine recall flow\n:NEXT_ACTION: continue memory engine sandtable\n:RECOVERY_REF: PLAN_ID=memory-engine-hot-path\n:END:\n** Checkpoints\n- [ ] Extract session task candidates from org plan\n- [X] Keep ranked plan row stable\n** TODO Verify task candidate rendering\n",
    )
    .expect("write hot plan");
    std::fs::write(
        &cold_plan,
        "* TODO Unrelated packaging cleanup :agent:plan:\n:PROPERTIES:\n:CONTRACT_ORG: agent.plan.v1\n:ID: unrelated-cold-path\n:OBJECTIVE: Unrelated packaging cleanup\n:NEXT_ACTION: continue unrelated cleanup\n:END:\n",
    )
    .expect("write cold plan");
    let state_path = root.join("memory-state.json");
    write_memory_rank_state(&root, &state_path, "memory-engine-hot-path");

    let output = asp_command(&root)
        .env("CODEX_THREAD_ID", "codex-recall-thread")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("CLAUDE_CODE_REMOTE_SESSION_ID")
        .env_remove("AGENT_SESSION_ID")
        .env_remove("SESSION_ID")
        .args([
            "org",
            "recall",
            "plans",
            "--artifacts-root",
            org_artifacts.to_str().unwrap(),
            "--state",
            state_path.to_str().unwrap(),
            "--project",
            "repo",
            "--top-k",
            "1",
            "--embedding-dim",
            "8",
        ])
        .output()
        .expect("run asp org recall plans");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("recall stdout");
    assert!(
        stdout.contains("[org-recall-plans] owner=rust session=\"codex-recall-thread\" hits=1"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "|next action=\"resume\" rank=1 plan=\"memory-engine-hot-path\" title=\"Stabilize memory engine recall flow\""
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "|why session=\"codex-recall-thread\" sessionMatched=true selectedBy=\"session+memory-engine+org-graph+recency\" taskHits=3 checkpointHits=0 alternatives=0"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "|query action=\"resume\" command=\"asp org query memory-engine-hot-path recovery evidence next-action\""
        ),
        "{stdout}"
    );
    assert!(
        !stdout.contains("memoryScore=") && !stdout.contains("artifactsRoot="),
        "{stdout}"
    );
    assert!(!stdout.contains("plan=\"unrelated-cold-path\""), "{stdout}");
    assert!(
        stdout.contains("|evidence rank=1 kind=\"property\" status=\"next-action\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("title=\"continue memory engine sandtable\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("kind=\"checklist\" status=\"unchecked\"")
            && stdout.contains("title=\"Extract session task candidates from org plan\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("kind=\"heading\" status=\"todo\"")
            && stdout.contains("title=\"Verify task candidate rendering\""),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "|candidate rank=1 action=\"resume\" plan=\"memory-engine-hot-path\" title=\"Stabilize memory engine recall flow\""
        ),
        "{stdout}"
    );
    assert!(
        !stdout.contains("resumeCommand=\"asp org recall plans\""),
        "{stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_org_recall_plans_uses_root_session_for_codex_subagent() {
    let root = temp_project_root("org-document-command-recall-plans-subagent-root-session");
    let org_artifacts = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("artifacts")
        .join("org");
    let plans = org_artifacts.join("flow").join("plans");
    std::fs::create_dir_all(&plans).expect("create plans dir");
    let parent_plan = plans.join("agent-plan-parent-session.org");
    let unrelated_plan = plans.join("agent-plan-unrelated-session.org");
    std::fs::write(
        &parent_plan,
        "* TODO Parent task plan :agent:plan:\n:PROPERTIES:\n:CONTRACT_ORG: agent.plan.v1\n:ID: parent-session-plan\n:SESSION_ID: codex-parent-thread\n:OBJECTIVE: Parent task plan\n:NEXT_ACTION: continue parent task\n:END:\n** Checkpoints\n- [ ] Continue parent task from child agent\n",
    )
    .expect("write parent plan");
    std::fs::write(
        &unrelated_plan,
        "* TODO Unrelated child fallback plan :agent:plan:\n:PROPERTIES:\n:CONTRACT_ORG: agent.plan.v1\n:ID: unrelated-child-fallback-plan\n:SESSION_ID: other-thread\n:OBJECTIVE: Unrelated child fallback plan\n:NEXT_ACTION: do not select this by recency\n:END:\n",
    )
    .expect("write unrelated plan");
    let state_path = root.join("memory-state.json");
    write_memory_rank_state(&root, &state_path, "parent-session-plan");

    let output = asp_command(&root)
        .env("CODEX_THREAD_ID", "codex-child-thread")
        .env("ASP_ROOT_SESSION_ID", "codex-parent-thread")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("CLAUDE_CODE_REMOTE_SESSION_ID")
        .env_remove("AGENT_SESSION_ID")
        .env_remove("SESSION_ID")
        .args([
            "org",
            "recall",
            "plans",
            "--artifacts-root",
            org_artifacts.to_str().unwrap(),
            "--state",
            state_path.to_str().unwrap(),
            "--project",
            "repo",
            "--top-k",
            "1",
            "--embedding-dim",
            "8",
        ])
        .output()
        .expect("run asp org recall plans from child with root session");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("recall stdout");
    assert!(
        stdout.contains("[org-recall-plans] owner=rust session=\"codex-parent-thread\" hits=1"),
        "{stdout}"
    );
    assert!(
        stdout.contains("plan=\"parent-session-plan\"") && stdout.contains("sessionMatched=true"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("plan=\"unrelated-child-fallback-plan\""),
        "{stdout}"
    );

    let output = asp_command(&root)
        .env("CODEX_THREAD_ID", "codex-child-thread")
        .env_remove("ASP_ROOT_SESSION_ID")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("CLAUDE_CODE_REMOTE_SESSION_ID")
        .env_remove("AGENT_SESSION_ID")
        .env_remove("SESSION_ID")
        .args([
            "org",
            "recall",
            "plans",
            "--artifacts-root",
            org_artifacts.to_str().unwrap(),
            "--state",
            state_path.to_str().unwrap(),
            "--project",
            "repo",
            "--top-k",
            "1",
            "--embedding-dim",
            "8",
        ])
        .output()
        .expect("run asp org recall plans from child without root session");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("recall stdout");
    assert!(
        stdout.contains("[org-recall-plans] owner=rust session=\"codex-child-thread\" hits=0"),
        "{stdout}"
    );
    assert!(!stdout.contains("plan=\"parent-session-plan\""), "{stdout}");
    assert!(
        !stdout.contains("plan=\"unrelated-child-fallback-plan\""),
        "{stdout}"
    );

    let register_child = asp_command(&root)
        .args([
            "agent",
            "session",
            "register",
            "--name",
            "asp-explore-code",
            "--child-session-id",
            "codex-child-thread",
            "--root-session-id",
            "codex-parent-thread",
            "--parent-session-id",
            "codex-parent-thread",
            "--roles",
            "subagent,search",
        ])
        .output()
        .expect("register child session in agent registry");
    assert!(
        register_child.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&register_child.stderr)
    );

    let output = asp_command(&root)
        .env("CODEX_THREAD_ID", "codex-child-thread")
        .env_remove("ASP_ROOT_SESSION_ID")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("CLAUDE_CODE_REMOTE_SESSION_ID")
        .env_remove("AGENT_SESSION_ID")
        .env_remove("SESSION_ID")
        .args([
            "org",
            "recall",
            "plans",
            "--artifacts-root",
            org_artifacts.to_str().unwrap(),
            "--state",
            state_path.to_str().unwrap(),
            "--project",
            "repo",
            "--top-k",
            "1",
            "--embedding-dim",
            "8",
        ])
        .output()
        .expect("run asp org recall plans from registered child session");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("recall stdout");
    assert!(
        stdout.contains("[org-recall-plans] owner=rust session=\"codex-parent-thread\" hits=1"),
        "{stdout}"
    );
    assert!(
        stdout.contains("plan=\"parent-session-plan\"") && stdout.contains("sessionMatched=true"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("plan=\"unrelated-child-fallback-plan\""),
        "{stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn asp_org_recall_plans_restarts_stale_socket_and_ignores_stale_project_binary() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_project_root("org-document-command-recall-plans-stale-runtime");
    let org_artifacts = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("artifacts")
        .join("org");
    let plans = org_artifacts.join("flow").join("plans");
    std::fs::create_dir_all(&plans).expect("create plans dir");
    let hot_plan = plans.join("agent-plan-stale-runtime-hot-path.org");
    std::fs::write(
        &hot_plan,
        "* TODO Recover stale memory runtime :agent:plan:\n:PROPERTIES:\n:CONTRACT_ORG: agent.plan.v1\n:ID: stale-runtime-hot-path\n:SESSION_ID: codex-stale-runtime-thread\n:OBJECTIVE: Recover stale memory runtime\n:NEXT_ACTION: continue stale runtime recovery\n:END:\n** Checkpoints\n- [ ] Drop stale worker response\n",
    )
    .expect("write hot plan");
    let state_path = root.join("memory-state.json");
    write_memory_rank_state(&root, &state_path, "stale-runtime-hot-path");

    let stale_binary = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("artifacts")
        .join("bin")
        .join("asp-memory-engine-current");
    std::fs::create_dir_all(stale_binary.parent().unwrap()).expect("create stale bin dir");
    std::fs::write(
        &stale_binary,
        r#"#!/usr/bin/env python3
import os
import socket
import sys

OLD = b'{"plans":[{"id":"stale-runtime-hot-path","score":1.0,"textScore":1.0,"intentScore":0.0,"recencyScore":0.0}]}\n'

if len(sys.argv) > 1 and sys.argv[1] == "worker":
    socket_path = sys.argv[3]
    try:
        os.unlink(socket_path)
    except FileNotFoundError:
        pass
    server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    server.bind(socket_path)
    server.listen(1)
    conn, _ = server.accept()
    with conn:
        conn.recv(65536)
        conn.sendall(OLD)
else:
    sys.stdout.buffer.write(OLD)
"#,
    )
    .expect("write stale memory engine");
    let mut permissions = std::fs::metadata(&stale_binary)
        .expect("stale bin metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&stale_binary, permissions).expect("chmod stale memory engine");

    let canonical_root = root.canonicalize().expect("canonical temp root");
    let socket_dir = Path::new("/tmp").join(format!(
        "asp-mem-stale-{}-{:x}",
        std::process::id(),
        stable_project_hash_for_test(&canonical_root) & 0xffff
    ));
    let socket_path = memory_engine_socket_for_test(&canonical_root, &socket_dir);
    let (stale_socket, stale_socket_accepted) =
        spawn_stale_rank_socket(&socket_path, "stale-runtime-hot-path");

    let output = asp_command(&root)
        .env("CODEX_THREAD_ID", "codex-stale-runtime-thread")
        .env("ASP_MEMORY_ENGINE_AUTO_SOCKET", "1")
        .env("ASP_MEMORY_ENGINE_SOCKET_DIR", socket_dir.to_str().unwrap())
        .env_remove("ASP_MEMORY_ENGINE")
        .env_remove("ASP_MEMORY_ENGINE_SOCKET")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("CLAUDE_CODE_REMOTE_SESSION_ID")
        .env_remove("AGENT_SESSION_ID")
        .env_remove("SESSION_ID")
        .args([
            "org",
            "recall",
            "plans",
            "--artifacts-root",
            org_artifacts.to_str().unwrap(),
            "--state",
            state_path.to_str().unwrap(),
            "--project",
            "repo",
            "--top-k",
            "1",
            "--embedding-dim",
            "8",
        ])
        .output()
        .expect("run asp org recall plans with stale memory runtime");
    if let Err(error) = stale_socket_accepted.recv_timeout(std::time::Duration::from_secs(5)) {
        let _ = std::os::unix::net::UnixStream::connect(&socket_path);
        let _ = stale_socket.join();
        panic!("stale socket was not used: {error}");
    }
    stale_socket
        .join()
        .expect("stale socket handled one request");
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("recall stdout");
    assert!(
        stdout.contains("memoryTransport=\"socket:auto\"")
            || stdout.contains("memoryTransport=\"process:auto-fallback\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("plan=\"stale-runtime-hot-path\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("selectedBy=\"session+memory-engine+org-graph+recency\""),
        "{stdout}"
    );
    assert!(!stdout.contains("contextScore="), "{stdout}");
    assert!(!stdout.contains("intentScore="), "{stdout}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_org_recall_checkpoint_sync_writes_current_session_task_candidates() {
    let root = temp_project_root("org-document-command-recall-checkpoint-sync");
    let org_artifacts = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("artifacts")
        .join("org");
    let plans = org_artifacts.join("flow").join("plans");
    std::fs::create_dir_all(&plans).expect("create plans dir");
    let hot_plan = plans.join("agent-plan-checkpoint-hot-path.org");
    let other_session_plan = plans.join("agent-plan-other-session.org");
    std::fs::write(
        &hot_plan,
        "* TODO Checkpoint current session tasks :agent:plan:\n:PROPERTIES:\n:CONTRACT_ORG: agent.plan.v1\n:ID: checkpoint-hot-path\n:SESSION_ID: codex-recall-thread\n:OBJECTIVE: Persist current session checkpoint tasks\n:NEXT_ACTION: continue checkpoint sync\n:END:\n** Checkpoints\n- [ ] Extract current session checkpoint\n** TODO Verify checkpoint recall\n",
    )
    .expect("write hot plan");
    std::fs::write(
        &other_session_plan,
        "* TODO Other session task :agent:plan:\n:PROPERTIES:\n:CONTRACT_ORG: agent.plan.v1\n:ID: checkpoint-other-session\n:SESSION_ID: other-thread\n:OBJECTIVE: Skip other session checkpoint tasks\n:NEXT_ACTION: do not persist this session\n:END:\n** Checkpoints\n- [ ] Do not sync other session checkpoint\n",
    )
    .expect("write other session plan");
    let state_path = root.join("memory-state.json");
    write_memory_rank_state(&root, &state_path, "checkpoint-hot-path");

    let output = asp_command(&root)
        .env("CODEX_THREAD_ID", "codex-recall-thread")
        .env("ASP_MEMORY_ENGINE_AUTO_SOCKET", "0")
        .env_remove("ASP_MEMORY_ENGINE_SOCKET")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("CLAUDE_CODE_REMOTE_SESSION_ID")
        .env_remove("AGENT_SESSION_ID")
        .env_remove("SESSION_ID")
        .args([
            "org",
            "recall",
            "plans",
            "--artifacts-root",
            org_artifacts.to_str().unwrap(),
            "--state",
            state_path.to_str().unwrap(),
            "--project",
            "repo",
            "--top-k",
            "2",
            "--embedding-dim",
            "8",
            "--checkpoint-sync",
        ])
        .output()
        .expect("run asp org recall plans checkpoint sync");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("recall stdout");
    assert!(
        stdout.contains("|checkpoint-sync checkpoints=3 skippedSessionPlans=1"),
        "{stdout}"
    );
    assert!(stdout.contains("memoryTransport=\"process\""), "{stdout}");

    let list_script = root.join("list-checkpoints.py");
    std::fs::write(
        &list_script,
        r#"import json
import sys
from asp_memory_engine import EpisodeStore, StoreConfig

store = EpisodeStore(StoreConfig(path=sys.argv[1], embedding_dim=8))
store.load_state(sys.argv[1])
items = [
    checkpoint.to_mapping()
    for checkpoint in store.list_checkpoints(project_id="repo", session_id="codex-recall-thread")
]
print(json.dumps(items, sort_keys=True))
"#,
    )
    .expect("write checkpoint list script");
    let packages_python = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/python");
    let list_output = Command::new("uv")
        .args(["run", "--project"])
        .arg(packages_python)
        .arg("--frozen")
        .arg("python")
        .arg(&list_script)
        .arg(&state_path)
        .current_dir(&root)
        .output()
        .expect("run checkpoint list script");
    assert!(
        list_output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&list_output.stdout),
        String::from_utf8_lossy(&list_output.stderr)
    );
    let checkpoint_json =
        String::from_utf8(list_output.stdout).expect("checkpoint list stdout is utf8");
    assert!(
        checkpoint_json.contains("continue checkpoint sync"),
        "{checkpoint_json}"
    );
    assert!(
        checkpoint_json.contains("Extract current session checkpoint"),
        "{checkpoint_json}"
    );
    assert!(
        checkpoint_json.contains("Verify checkpoint recall"),
        "{checkpoint_json}"
    );
    assert!(
        checkpoint_json.contains(
            "\"resume_command\": \"asp org query checkpoint-hot-path recovery evidence next-action\""
        ),
        "{checkpoint_json}"
    );
    assert!(
        !checkpoint_json.contains("asp org recall plans"),
        "{checkpoint_json}"
    );
    assert!(
        !checkpoint_json.contains("Do not sync other session checkpoint"),
        "{checkpoint_json}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_org_recall_checkpoint_sync_rejects_generic_session_env_fallback() {
    let root = temp_project_root("org-document-command-recall-checkpoint-sync-generic-session");
    let org_artifacts = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("artifacts")
        .join("org");
    let plans = org_artifacts.join("flow").join("plans");
    std::fs::create_dir_all(&plans).expect("create plans dir");
    let plan = plans.join("agent-plan-generic-session.org");
    std::fs::write(
        &plan,
        "* TODO Generic session should not imply agent session :agent:plan:\n:PROPERTIES:\n:CONTRACT_ORG: agent.plan.v1\n:ID: generic-session-plan\n:SESSION_ID: generic-session\n:OBJECTIVE: Avoid generic env fallback\n:NEXT_ACTION: require explicit session\n:END:\n** Checkpoints\n- [ ] Do not checkpoint from generic env\n",
    )
    .expect("write generic session plan");
    let state_path = root.join("memory-state.json");
    write_memory_rank_state(&root, &state_path, "generic-session-plan");

    let output = asp_command(&root)
        .env("AGENT_SESSION_ID", "generic-agent-thread")
        .env("SESSION_ID", "generic-session")
        .env("ASP_MEMORY_ENGINE_AUTO_SOCKET", "0")
        .env_remove("CODEX_THREAD_ID")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("CLAUDE_CODE_REMOTE_SESSION_ID")
        .env_remove("ASP_MEMORY_ENGINE_SOCKET")
        .args([
            "org",
            "recall",
            "plans",
            "--artifacts-root",
            org_artifacts.to_str().unwrap(),
            "--state",
            state_path.to_str().unwrap(),
            "--project",
            "repo",
            "--top-k",
            "1",
            "--embedding-dim",
            "8",
            "--checkpoint-sync",
        ])
        .output()
        .expect("run asp org recall plans checkpoint sync without platform session");
    assert!(
        !output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8(output.stderr).expect("recall stderr");
    assert!(
        stderr.contains("--checkpoint-sync requires --session or agent session env"),
        "{stderr}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_org_recall_checkpoint_sync_rejects_ambiguous_agent_session_envs() {
    let root = temp_project_root("org-document-command-recall-checkpoint-sync-ambiguous-session");
    let org_artifacts = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("artifacts")
        .join("org");
    let plans = org_artifacts.join("flow").join("plans");
    std::fs::create_dir_all(&plans).expect("create plans dir");
    let plan = plans.join("agent-plan-ambiguous-session.org");
    std::fs::write(
        &plan,
        "* TODO Ambiguous session should not imply agent session :agent:plan:\n:PROPERTIES:\n:CONTRACT_ORG: agent.plan.v1\n:ID: ambiguous-session-plan\n:SESSION_ID: codex-thread\n:OBJECTIVE: Avoid ordered env priority\n:NEXT_ACTION: require explicit session\n:END:\n** Checkpoints\n- [ ] Do not checkpoint from ambiguous env\n",
    )
    .expect("write ambiguous session plan");
    let state_path = root.join("memory-state.json");
    write_memory_rank_state(&root, &state_path, "ambiguous-session-plan");

    let output = asp_command(&root)
        .env("CODEX_THREAD_ID", "codex-thread")
        .env("CLAUDE_CODE_SESSION_ID", "claude-thread")
        .env("ASP_MEMORY_ENGINE_AUTO_SOCKET", "0")
        .env_remove("CLAUDE_CODE_REMOTE_SESSION_ID")
        .env_remove("ASP_MEMORY_ENGINE_SOCKET")
        .args([
            "org",
            "recall",
            "plans",
            "--artifacts-root",
            org_artifacts.to_str().unwrap(),
            "--state",
            state_path.to_str().unwrap(),
            "--project",
            "repo",
            "--top-k",
            "1",
            "--embedding-dim",
            "8",
            "--checkpoint-sync",
        ])
        .output()
        .expect("run asp org recall plans checkpoint sync with ambiguous platform sessions");
    assert!(
        !output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8(output.stderr).expect("recall stderr");
    assert!(
        stderr.contains("--checkpoint-sync requires --session or agent session env"),
        "{stderr}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_org_recall_plans_marks_done_records_archive_action() {
    let root = temp_project_root("org-document-command-recall-plans-archive-action");
    let org_artifacts = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("artifacts")
        .join("org");
    let plans = org_artifacts.join("flow").join("plans");
    std::fs::create_dir_all(&plans).expect("create plans dir");
    std::fs::write(
        plans.join("agent-plan-archive-ready.org"),
        "* DONE Archive ready plan [3/3] [100%] :agent:plan:\n:PROPERTIES:\n:CONTRACT_ORG: agent.plan.v1\n:ID: archive-ready-plan\n:OBJECTIVE: Archive ready plan\n:STATUS: complete\n:NEXT_ACTION: archive-ready\n:EVIDENCE_STATUS: validated\n:REVIEW_STATUS: passed\n:END:\n** Reflection\n| Question | Value | Evidence |\n| Did the task finish? | yes | [[#archive-ready-plan][plan evidence]] |\n| Did project scope drift? | no | [[#archive-ready-plan][plan root]] |\n| Are all checklist items done? | yes | [[#archive-ready-plan][plan root]] |\n",
    )
    .expect("write archive ready plan");

    let output = asp_command(&root)
        .args([
            "org",
            "recall",
            "plans",
            "--artifacts-root",
            org_artifacts.to_str().unwrap(),
            "--include-done",
            "--archive-dir",
            "closed",
            "--top-k",
            "1",
        ])
        .output()
        .expect("run asp org recall plans include done");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("recall stdout");
    assert!(
        stdout.contains("|next action=\"archive\" rank=1 plan=\"archive-ready-plan\""),
        "{stdout}"
    );
    assert!(
        stdout
            .contains("|query action=\"archive\" command=\"asp org archive done --artifacts-root"),
        "{stdout}"
    );
    assert!(stdout.contains("--archive-dir closed"), "{stdout}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_org_recall_plans_blocks_archive_until_reflection_answered() {
    let root = temp_project_root("org-document-command-recall-plans-reflection-gate");
    let org_artifacts = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("artifacts")
        .join("org");
    let plans = org_artifacts.join("flow").join("plans");
    std::fs::create_dir_all(&plans).expect("create plans dir");
    std::fs::write(
        plans.join("agent-plan-needs-reflection.org"),
        "* DONE Needs reflection plan [3/3] [100%] :agent:plan:\n:PROPERTIES:\n:CONTRACT_ORG: agent.plan.v1\n:ID: needs-reflection-plan\n:OBJECTIVE: Needs reflection plan\n:STATUS: complete\n:NEXT_ACTION: archive-ready\n:EVIDENCE_STATUS: validated\n:REVIEW_STATUS: passed\n:END:\n** Reflection\n| Question | Value | Evidence |\n| Did the task finish? | pending | [[#needs-reflection-plan][plan evidence]] |\n",
    )
    .expect("write needs reflection plan");

    let output = asp_command(&root)
        .args([
            "org",
            "recall",
            "plans",
            "--artifacts-root",
            org_artifacts.to_str().unwrap(),
            "--include-done",
            "--top-k",
            "1",
        ])
        .output()
        .expect("run asp org recall plans include done");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("recall stdout");
    assert!(
        stdout
            .contains("|next action=\"complete-reflection\" rank=1 plan=\"needs-reflection-plan\""),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "|query action=\"complete-reflection\" command=\"asp org query needs-reflection-plan recovery evidence next-action\""
        ),
        "{stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
fn spawn_stale_rank_socket(
    socket_path: &Path,
    plan_id: &str,
) -> (std::thread::JoinHandle<()>, std::sync::mpsc::Receiver<()>) {
    use std::io::{BufRead, Write};
    use std::os::unix::net::UnixListener;

    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent).expect("create socket dir");
    }
    let _ = std::fs::remove_file(socket_path);
    let listener = UnixListener::bind(socket_path).expect("bind stale memory worker socket");
    let response = format!(
        r#"{{"plans":[{{"id":"{plan_id}","score":1.0,"textScore":1.0,"intentScore":0.0,"recencyScore":0.0}}]}}"#
    );
    let (accepted_tx, accepted_rx) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept stale memory request");
        let _ = accepted_tx.send(());
        let mut request = Vec::new();
        let mut reader =
            std::io::BufReader::new(stream.try_clone().expect("clone stale memory stream"));
        let _ = reader.read_until(b'\n', &mut request);
        stream
            .write_all(response.as_bytes())
            .expect("write stale memory response");
        stream.write_all(b"\n").expect("write stale memory newline");
    });
    (handle, accepted_rx)
}

#[cfg(unix)]
fn memory_engine_socket_for_test(root: &Path, socket_dir: &Path) -> std::path::PathBuf {
    socket_dir.join(format!(
        "asp-memory-engine-{:016x}.sock",
        stable_project_hash_for_test(root)
    ))
}

#[cfg(unix)]
fn stable_project_hash_for_test(path: &Path) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in path.display().to_string().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn write_memory_rank_state(root: &Path, state_path: &Path, plan_id: &str) {
    let script = root.join("write-memory-state.py");
    std::fs::write(
        &script,
        format!(
            r#"from pathlib import Path
import sys
from asp_memory_engine import Episode, EpisodeDraft, EpisodeStore, PlanMemoryContext, StoreConfig

state = Path(sys.argv[1])
store = EpisodeStore(StoreConfig(path=str(state), embedding_dim=8))
context = PlanMemoryContext(project_id="repo", plan_id="{plan_id}")
store.store(Episode.new(EpisodeDraft(
    id="memory-engine-hot-episode",
    intent="stabilize memory engine recall flow",
    intent_embedding=store.encoder.encode("stabilize memory engine recall flow"),
    experience="continue memory engine sandtable",
    outcome="pending",
).with_plan_context(context, sharing="project")))
store.save_state(state)
"#
        ),
    )
    .expect("write memory state script");
    let packages_python = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/python");
    let output = Command::new("uv")
        .args(["run", "--project"])
        .arg(packages_python)
        .arg("--frozen")
        .arg("python")
        .arg(&script)
        .arg(state_path)
        .current_dir(root)
        .output()
        .expect("run memory state script");
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
