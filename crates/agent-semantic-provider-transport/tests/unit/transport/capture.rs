use std::fs;

use crate::OutputMode;
use crate::ProviderProcessSupervisor;

use super::support::script;
use super::support::spec;
use super::support::temp_dir;

#[tokio::test]
async fn truncates_captured_streams_but_counts_full_bytes() {
    let root = temp_dir("truncate");
    let program = script(
        &root,
        "provider.sh",
        "#!/bin/sh\nprintf 'abcdef'\nprintf '123456' >&2\n",
    );
    let mut process = spec(program, root.clone());
    process.limits = process
        .limits
        .with_max_stdout_bytes(Some(3))
        .with_max_stderr_bytes(Some(2));
    let output = ProviderProcessSupervisor::default()
        .run(process)
        .await
        .expect("run provider");

    assert_eq!(output.stdout.as_ref(), b"abc");
    assert_eq!(output.stderr.as_ref(), b"12");
    assert_eq!(output.receipt.stdout_bytes(), 6);
    assert_eq!(output.receipt.stderr_bytes(), 6);
    assert!(output.receipt.stdout_truncated());
    assert!(output.receipt.stderr_truncated());
    assert_eq!(
        output.receipt.stdout_sha256(),
        Some("bef57ec7f53a6d40beb640a780a639c83bc29ac8a9816f1fc6c5c6dcd93c4721")
    );
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn tee_mode_still_retains_captured_bytes() {
    let root = temp_dir("tee-capture");
    let program = script(&root, "provider.sh", "#!/bin/sh\nprintf 'tee-out'\n");
    let mut process = spec(program, root.clone());
    process.stdout = OutputMode::Tee;
    let output = ProviderProcessSupervisor::default()
        .run(process)
        .await
        .expect("run provider");

    assert!(output.status.success());
    assert_eq!(output.stdout.as_ref(), b"tee-out");
    assert_eq!(output.receipt.stdout_bytes(), 7);
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn handles_large_stdout_and_stderr_without_deadlock() {
    let root = temp_dir("large-stdio");
    let program = script(
        &root,
        "provider.sh",
        "#!/bin/sh\ni=0\nwhile [ $i -lt 2000 ]; do printf 'stdout-line-%s\\n' \"$i\"; printf 'stderr-line-%s\\n' \"$i\" >&2; i=$((i + 1)); done\n",
    );
    let output = ProviderProcessSupervisor::default()
        .run(spec(program, root.clone()))
        .await
        .expect("run provider");

    assert!(output.status.success());
    assert!(output.receipt.stdout_bytes() > 20_000);
    assert!(output.receipt.stderr_bytes() > 20_000);
    let _ = fs::remove_dir_all(root);
}
