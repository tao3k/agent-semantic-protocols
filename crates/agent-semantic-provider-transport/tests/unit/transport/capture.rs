use std::fs;

use crate::{OutputMode, run_provider_process};

use super::support::{script, spec, temp_dir};

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
