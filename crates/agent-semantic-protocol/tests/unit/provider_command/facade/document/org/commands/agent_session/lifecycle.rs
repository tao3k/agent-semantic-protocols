use crate::provider_command::support::{asp_command, temp_project_root};

#[test]
fn asp_agent_session_lifecycle_close_reconcile_and_gc_registry_rows() {
    let root = temp_project_root("agent-command-session-lifecycle-close-gc");

    let register_output = asp_command(&root)
        .args([
            "agent",
            "session",
            "register",
            "--name",
            "worker-a",
            "--child-session-id",
            "child-session-a",
            "--root-session-id",
            "root-session-a",
            "--role",
            "worker",
            "--model",
            "test-model",
            "--status",
            "active",
            "--json",
        ])
        .output()
        .expect("register lifecycle session");
    assert!(
        register_output.status.success(),
        "{}",
        String::from_utf8_lossy(&register_output.stderr)
    );

    let close_output = asp_command(&root)
        .args([
            "agent",
            "session",
            "close",
            "--name",
            "worker-a",
            "--root-session-id",
            "root-session-a",
            "--json",
        ])
        .output()
        .expect("close lifecycle session");
    assert!(
        close_output.status.success(),
        "{}",
        String::from_utf8_lossy(&close_output.stderr)
    );
    let close_stdout = String::from_utf8(close_output.stdout).expect("close stdout");
    assert!(
        close_stdout.contains("\"command\": \"close\""),
        "{close_stdout}"
    );
    assert!(close_stdout.contains("\"affected\": 1"), "{close_stdout}");
    assert!(
        close_stdout.contains("\"status\": \"archived\""),
        "{close_stdout}"
    );

    let reconcile_output = asp_command(&root)
        .args([
            "agent",
            "session",
            "reconcile",
            "--root-session-id",
            "root-session-a",
            "--json",
        ])
        .output()
        .expect("reconcile lifecycle sessions");
    assert!(
        reconcile_output.status.success(),
        "{}",
        String::from_utf8_lossy(&reconcile_output.stderr)
    );
    let reconcile_stdout = String::from_utf8(reconcile_output.stdout).expect("reconcile stdout");
    assert!(
        reconcile_stdout.contains("\"command\": \"reconcile\""),
        "{reconcile_stdout}"
    );
    assert!(
        reconcile_stdout.contains("\"affected\": 1"),
        "{reconcile_stdout}"
    );

    let gc_output = asp_command(&root)
        .args([
            "agent",
            "session",
            "gc",
            "--name",
            "worker-a",
            "--root-session-id",
            "root-session-a",
            "--json",
        ])
        .output()
        .expect("gc lifecycle session");
    assert!(
        gc_output.status.success(),
        "{}",
        String::from_utf8_lossy(&gc_output.stderr)
    );
    let gc_stdout = String::from_utf8(gc_output.stdout).expect("gc stdout");
    assert!(gc_stdout.contains("\"command\": \"gc\""), "{gc_stdout}");
    assert!(gc_stdout.contains("\"affected\": 1"), "{gc_stdout}");
    assert!(gc_stdout.contains("child-session-a"), "{gc_stdout}");

    let _ = std::fs::remove_dir_all(root);
}
