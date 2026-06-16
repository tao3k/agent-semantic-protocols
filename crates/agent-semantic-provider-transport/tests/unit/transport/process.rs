use std::fs;

use crate::{StdinMode, run_provider_process};

use super::support::{script, spec, temp_dir};

#[test]
fn captures_stdout_stderr_and_exit_status() {
    let root = temp_dir("capture-status");
    let program = script(
        &root,
        "provider.sh",
        "#!/bin/sh\nprintf 'out'\nprintf 'err' >&2\nexit 7\n",
    );
    let output = run_provider_process(spec(program, root.clone())).expect("run provider");

    assert_eq!(output.status.code(), Some(7));
    assert_eq!(output.stdout.as_ref(), b"out");
    assert_eq!(output.stderr.as_ref(), b"err");
    assert_eq!(output.stdout_lossy(), "out");
    assert_eq!(output.stderr_lossy(), "err");
    assert_eq!(output.receipt.status_code, Some(7));
    assert!(!output.receipt.status_success);
    assert_eq!(output.receipt.stdout_bytes, 3);
    assert_eq!(output.receipt.stderr_bytes, 3);
    assert_eq!(
        output.receipt.stdout_sha256.as_deref(),
        Some("762069bc07a6e1b5df123a5ae7bd91c10daa04694fbaa17fba0cd6a8dcce8f22")
    );
    assert_eq!(
        output.receipt.stderr_sha256.as_deref(),
        Some("d9eb253e06987fa74a5d3189f73d9f7a8104cca786fafbb52bc9555972f5477f")
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn writes_bytes_to_stdin() {
    let root = temp_dir("stdin-bytes");
    let program = script(&root, "provider.sh", "#!/bin/sh\ncat\n");
    let mut process = spec(program, root.clone());
    process.stdin = StdinMode::bytes("payload");
    let output = run_provider_process(process).expect("run provider");

    assert!(output.status.success());
    assert_eq!(output.stdout.as_ref(), b"payload");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn stdin_broken_pipe_still_reports_provider_output() {
    let root = temp_dir("stdin-broken-pipe");
    let program = script(&root, "provider.sh", "#!/bin/sh\nprintf 'ready'\nexit 0\n");
    let mut process = spec(program, root.clone());
    process.stdin = StdinMode::bytes(vec![b'x'; 1024 * 1024]);
    let output = run_provider_process(process).expect("run provider");

    assert!(output.status.success());
    assert_eq!(output.stdout.as_ref(), b"ready");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn passes_cwd_and_env() {
    let root = temp_dir("cwd-env");
    let program = script(
        &root,
        "provider.sh",
        "#!/bin/sh\nprintf 'cwd=%s env=%s' \"$(pwd)\" \"$ASP_TEST_VALUE\"\n",
    );
    let mut process = spec(program, root.clone());
    process.env.insert("ASP_TEST_VALUE".into(), "ok".into());
    let output = run_provider_process(process).expect("run provider");
    let stdout = output.stdout_lossy();

    assert!(
        stdout.contains(&format!("cwd={}", root.display())),
        "{stdout}"
    );
    assert!(stdout.contains("env=ok"), "{stdout}");
    let _ = fs::remove_dir_all(root);
}
