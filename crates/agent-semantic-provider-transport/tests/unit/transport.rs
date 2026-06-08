use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{
    OutputFraming, OutputMode, ProviderProcessError, ProviderProcessFraming, ProviderProcessLimits,
    ProviderProcessSpec, StdinMode, byte_text, run_provider_process,
    run_provider_process_with_framing,
};

fn temp_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("agent-provider-transport-{name}-{unique}"));
    fs::create_dir_all(&path).expect("create temp dir");
    path.canonicalize().unwrap_or(path)
}

fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, body).expect("write script");
    let mut permissions = fs::metadata(&path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("chmod");
    path
}

fn spec(program: PathBuf, cwd: PathBuf) -> ProviderProcessSpec {
    ProviderProcessSpec {
        program: program.display().to_string(),
        args: Vec::new(),
        cwd,
        env: BTreeMap::new(),
        stdin: StdinMode::Closed,
        stdout: OutputMode::Capture,
        stderr: OutputMode::Capture,
        limits: ProviderProcessLimits::default(),
    }
}

#[test]
fn byte_text_helpers_preserve_byte_oriented_boundaries() {
    assert_eq!(
        byte_text::split_lf_or_nul_records(b" src/lib.rs \n\ttests/a.rs\0").collect::<Vec<_>>(),
        vec![
            b"src/lib.rs".as_slice(),
            b"tests/a.rs".as_slice(),
            b"".as_slice()
        ]
    );
    assert_eq!(
        byte_text::split_lf_lines(b"first\r\nsecond\n").collect::<Vec<_>>(),
        vec![b"first".as_slice(), b"second".as_slice(), b"".as_slice()]
    );
    assert_eq!(
        byte_text::line_slices(b"first\r\nsecond"),
        vec![b"first".as_slice(), b"second".as_slice()]
    );
    assert_eq!(byte_text::find_byte(b':', b"path:12:text"), Some(4));
    assert!(byte_text::lossy_string(b"\xffterm").contains("term"));
}

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

#[test]
fn truncates_captured_streams_but_counts_full_bytes() {
    let root = temp_dir("truncate");
    let program = script(
        &root,
        "provider.sh",
        "#!/bin/sh\nprintf 'abcdef'\nprintf '123456' >&2\n",
    );
    let mut process = spec(program, root.clone());
    process.limits.max_stdout_bytes = Some(3);
    process.limits.max_stderr_bytes = Some(2);
    let output = run_provider_process(process).expect("run provider");

    assert_eq!(output.stdout.as_ref(), b"abc");
    assert_eq!(output.stderr.as_ref(), b"12");
    assert_eq!(output.receipt.stdout_bytes, 6);
    assert_eq!(output.receipt.stderr_bytes, 6);
    assert!(output.receipt.stdout_truncated);
    assert!(output.receipt.stderr_truncated);
    assert_eq!(
        output.receipt.stdout_sha256.as_deref(),
        Some("bef57ec7f53a6d40beb640a780a639c83bc29ac8a9816f1fc6c5c6dcd93c4721")
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn tee_mode_still_retains_captured_bytes() {
    let root = temp_dir("tee-capture");
    let program = script(&root, "provider.sh", "#!/bin/sh\nprintf 'tee-out'\n");
    let mut process = spec(program, root.clone());
    process.stdout = OutputMode::Tee;
    let output = run_provider_process(process).expect("run provider");

    assert!(output.status.success());
    assert_eq!(output.stdout.as_ref(), b"tee-out");
    assert_eq!(output.receipt.stdout_bytes, 7);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn line_framing_normalizes_line_payloads() {
    let root = temp_dir("line-framing");
    let program = script(
        &root,
        "provider.sh",
        "#!/bin/sh\nprintf 'first\\nsecond'\nprintf 'warn\\n' >&2\n",
    );
    let output = run_provider_process_with_framing(
        spec(program, root.clone()),
        ProviderProcessFraming {
            stdout: OutputFraming::Lines,
            stderr: OutputFraming::Lines,
        },
    )
    .expect("run provider");

    assert!(output.status.success());
    assert_eq!(output.stdout.as_ref(), b"first\nsecond\n");
    assert_eq!(output.stderr.as_ref(), b"warn\n");
    assert_eq!(output.receipt.stdout_bytes, "first\nsecond\n".len());
    assert_eq!(output.receipt.stderr_bytes, "warn\n".len());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn length_delimited_framing_captures_payload_bytes() {
    let root = temp_dir("length-delimited-framing");
    let program = script(
        &root,
        "provider.sh",
        "#!/bin/sh\nprintf '\\000\\000\\000\\005hello\\000\\000\\000\\005world'\n",
    );
    let output = run_provider_process_with_framing(
        spec(program, root.clone()),
        ProviderProcessFraming {
            stdout: OutputFraming::LengthDelimited,
            stderr: OutputFraming::Bytes,
        },
    )
    .expect("run provider");

    assert!(output.status.success());
    assert_eq!(output.stdout.as_ref(), b"helloworld");
    assert_eq!(output.receipt.stdout_bytes, 10);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn handles_large_stdout_and_stderr_without_deadlock() {
    let root = temp_dir("large-stdio");
    let program = script(
        &root,
        "provider.sh",
        "#!/bin/sh\ni=0\nwhile [ $i -lt 2000 ]; do printf 'stdout-line-%s\\n' \"$i\"; printf 'stderr-line-%s\\n' \"$i\" >&2; i=$((i + 1)); done\n",
    );
    let output = run_provider_process(spec(program, root.clone())).expect("run provider");

    assert!(output.status.success());
    assert!(output.receipt.stdout_bytes > 20_000);
    assert!(output.receipt.stderr_bytes > 20_000);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn times_out_and_kills_child_process() {
    let root = temp_dir("timeout-kill");
    let marker = root.join("marker");
    let program = script(
        &root,
        "provider.sh",
        "#!/bin/sh\nsleep 1\nprintf done > marker\n",
    );
    let mut process = spec(program, root.clone());
    process.limits.timeout = Some(Duration::from_millis(50));

    let error = run_provider_process(process).expect_err("provider should time out");
    match error {
        ProviderProcessError::Timeout { receipt, .. } => {
            assert!(receipt.timed_out);
            assert_eq!(receipt.status_code, None);
            assert!(!receipt.status_success);
        }
        other => panic!("expected timeout error, got {other:?}"),
    }
    std::thread::sleep(Duration::from_millis(150));
    assert!(!marker.exists());
    let _ = fs::remove_dir_all(root);
}
