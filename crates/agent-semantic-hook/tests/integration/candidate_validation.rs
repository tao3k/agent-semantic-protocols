#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

use agent_semantic_hook::candidate_validation::{
    HookEvaluatorCandidateValidation, validate_hook_evaluator_candidate,
};

#[tokio::test]
async fn hook_evaluator_candidate_timeout_kills_and_reaps_process_tree() {
    let fixture = tempfile::tempdir().expect("fixture");
    let evaluator = fixture.path().join("asp-hook-evaluator");
    let shell_pid_path = fixture.path().join("evaluator.pid");
    let child_pid_path = fixture.path().join("evaluator-child.pid");
    std::fs::write(
        &evaluator,
        format!(
            "#!/bin/sh\nprintf '%s' \"$$\" > {}\n/bin/sleep 30 &\nprintf '%s' \"$!\" > {}\nwait\n",
            shell_pid_path.display(),
            child_pid_path.display()
        ),
    )
    .expect("slow evaluator");
    std::fs::set_permissions(&evaluator, std::fs::Permissions::from_mode(0o755))
        .expect("executable evaluator");
    let generation = fixture.path().join("generation");
    std::fs::create_dir(&generation).expect("generation");

    let started = Instant::now();
    let error = validate_hook_evaluator_candidate(HookEvaluatorCandidateValidation {
        evaluator_path: &evaluator,
        generation_path: &generation,
        generation_digest: "blake3-256:candidate-timeout",
        state_home: fixture.path(),
    })
    .await
    .expect_err("slow candidate must time out");
    assert!(started.elapsed() < Duration::from_millis(1_500));
    assert!(error.contains("killed and reaped"), "{error}");

    for pid_path in [&shell_pid_path, &child_pid_path] {
        let pid = std::fs::read_to_string(pid_path).expect("candidate process pid");
        let status = std::process::Command::new("/bin/kill")
            .args(["-0", pid.trim()])
            .status()
            .expect("probe candidate process");
        assert!(
            !status.success(),
            "timed out candidate process {} still exists",
            pid.trim()
        );
    }
}
