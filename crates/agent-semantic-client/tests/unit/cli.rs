use crate::run_cli_args;
use std::path::PathBuf;

const DOCTOR_USAGE: &str = "usage: asp doctor\nrooted health: asp tools doctor [PROJECT_ROOT]";

#[test]
fn doctor_rejects_workspace_before_running_diagnostics() {
    let cwd = PathBuf::from("/tmp/asp-doctor-invocation");
    let error = run_cli_args(
        None,
        vec![
            "doctor".to_string(),
            "--workspace".to_string(),
            "/tmp/project".to_string(),
        ],
        cwd,
    )
    .expect_err("doctor must not reinterpret a project root");

    assert_eq!(error, DOCTOR_USAGE);
}

#[test]
fn doctor_rejects_global_receipt_flags() {
    let cwd = PathBuf::from("/tmp/asp-doctor-invocation");
    for args in [
        vec!["doctor".to_string(), "--receipt-json".to_string()],
        vec![
            "doctor".to_string(),
            "--frontier-receipt-out".to_string(),
            "/tmp/receipt.json".to_string(),
        ],
    ] {
        let error = run_cli_args(None, args, cwd.clone())
            .expect_err("doctor must reject unsupported receipt flags");
        assert_eq!(error, DOCTOR_USAGE);
    }
}
