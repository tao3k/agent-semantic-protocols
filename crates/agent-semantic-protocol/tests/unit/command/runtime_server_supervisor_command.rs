use super::{Command, SupervisorCommandError, resolve_supervisor_command};

#[test]
fn command_injection_requires_an_absolute_path() {
    let error = resolve_supervisor_command(std::ffi::OsStr::new("relative-command"))
        .expect_err("relative supervisor commands must fail closed");
    assert!(error.contains("must be launchctl, systemctl, or an absolute path"));
}

#[tokio::test(flavor = "current_thread")]
async fn absolute_true_fixture_completes_without_touching_host_services() {
    let output = Command::new("/usr/bin/true")
        .output()
        .await
        .expect("absolute true fixture must execute");
    assert!(output.status.success());
}

#[tokio::test(flavor = "current_thread")]
async fn supervisor_command_timeout_is_typed_and_bounded() {
    let started = std::time::Instant::now();
    let error = Command::new("/bin/sleep")
        .args(["2"])
        .output()
        .await
        .expect_err("sleep must exceed the supervisor command boundary");
    assert!(matches!(error, SupervisorCommandError::Timeout(_)));
    assert!(started.elapsed() < std::time::Duration::from_millis(1_200));
}

#[cfg(target_os = "macos")]
#[test]
fn launchd_loaded_program_requires_the_exact_runtime_artifact() {
    let desired = std::path::Path::new("/state/runtime/artifacts/blake3-256/current/asp");
    let matching = b"service = {\n\tprogram = /state/runtime/artifacts/blake3-256/current/asp\n}\n";
    let stale = b"service = {\n\tprogram = /state/runtime/artifacts/blake3-256/stale/asp\n}\n";

    assert!(super::launchd_loaded_program_matches(matching, desired));
    assert!(!super::launchd_loaded_program_matches(stale, desired));
    assert!(!super::launchd_loaded_program_matches(
        b"service = { malformed }",
        desired
    ));
    assert!(!super::launchd_loaded_program_matches(&[0xff], desired));
}
