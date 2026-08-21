#[cfg(unix)]
#[tokio::test]
async fn timed_out_hook_process_is_killed_and_reaped() {
    use std::ffi::OsString;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::time::Duration;

    let temporary = tempfile::tempdir().expect("temporary hook process root");
    let pid_path = temporary.path().join("pid");
    let script_path = temporary.path().join("hang.sh");
    std::fs::write(
        &script_path,
        format!("#!/bin/sh\nprintf '%s' \"$$\" > {}\nexec sleep 30\n", pid_path.display()),
    )
    .expect("write hanging hook fixture");
    let mut permissions = std::fs::metadata(&script_path)
        .expect("fixture metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&script_path, permissions).expect("make fixture executable");

    let output = agent_semantic_runtime::hook_process_runtime::run_hook_process(
        agent_semantic_runtime::hook_process_runtime::HookProcessRequest {
            executable: Path::new("/bin/sh"),
            args: &[OsString::from(script_path.as_os_str())],
            stdin: &[],
            timeout: Duration::from_millis(100),
            current_dir: Some(temporary.path()),
            environment: None,
            discard_stdout: true,
        },
    )
    .await
    .expect_err("hanging hook process must time out");
    assert!(output.contains("killed/reaped"), "{output}");

    let pid = std::fs::read_to_string(&pid_path)
        .expect("fixture recorded child pid")
        .parse::<i32>()
        .expect("numeric child pid");
    let status = unsafe { libc::kill(pid, 0) };
    assert_eq!(status, -1, "timed-out child must no longer exist");
    assert_eq!(std::io::Error::last_os_error().raw_os_error(), Some(libc::ESRCH));
}
