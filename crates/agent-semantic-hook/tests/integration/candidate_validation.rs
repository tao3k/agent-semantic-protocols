#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

use agent_semantic_hook::candidate_validation::{
    HookBinaryCandidateValidation, validate_hook_binary_candidate,
};

#[tokio::test]
async fn hook_binary_candidate_timeout_kills_and_reaps_process_tree() {
    let fixture = tempfile::tempdir().expect("fixture");
    let hook_binary = fixture.path().join("asp-hook");
    let shell_pid_path = fixture.path().join("hook-binary.pid");
    let child_pid_path = fixture.path().join("hook-binary-child.pid");
    std::fs::write(
        &hook_binary,
        format!(
            "#!/bin/sh\nprintf '%s' \"$$\" > {}\n/bin/sleep 30 &\nprintf '%s' \"$!\" > {}\nwait\n",
            shell_pid_path.display(),
            child_pid_path.display()
        ),
    )
    .expect("slow Hook binary");
    std::fs::set_permissions(&hook_binary, std::fs::Permissions::from_mode(0o755))
        .expect("executable Hook binary");
    let generation = fixture.path().join("generation");
    std::fs::create_dir(&generation).expect("generation");

    let started = Instant::now();
    let error = validate_hook_binary_candidate(HookBinaryCandidateValidation {
        hook_binary_path: &hook_binary,
        generation_path: &generation,
        generation_digest: "blake3-256:candidate-timeout",
        state_home: fixture.path(),
    })
    .await
    .expect_err("slow candidate must time out");
    assert!(started.elapsed() < Duration::from_millis(1_500));
    assert!(error.contains("killed and reaped"), "{error}");

    for pid_path in [&shell_pid_path, &child_pid_path] {
        let pid = match std::fs::read_to_string(pid_path) {
            Ok(pid) => pid,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                // The Host deadline may expire before the candidate is ever
                // scheduled into its script body. In that case there is no
                // observed process identity to probe; the process-group kill
                // and direct-child reap receipt above remain authoritative.
                continue;
            }
            Err(error) => panic!("read candidate process pid {}: {error}", pid_path.display()),
        };
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
