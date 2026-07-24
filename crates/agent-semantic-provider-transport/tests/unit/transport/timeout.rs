use std::fs;
use std::time::Duration;

use crate::{ProviderProcessError, run_provider_process};

use super::support::{script, spec, temp_dir};

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
            assert!(receipt.abnormal_termination);
            assert_eq!(receipt.termination_reason, "timeout");
        }
        other => panic!("expected timeout error, got {other:?}"),
    }
    std::thread::sleep(Duration::from_millis(150));
    assert!(!marker.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn completed_child_wins_over_an_already_ready_deadline() {
    let root = temp_dir("completed-before-deadline-observation");
    let program = script(&root, "provider.sh", "#!/bin/sh\nexit 0\n");
    let mut process = spec(program, root.clone());
    process.limits.timeout = Some(Duration::ZERO);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build runtime");
    runtime.block_on(async {
        for _ in 0..32 {
            let start = std::time::Instant::now();
            let stdin_mode = process.stdin.clone();
            let stdout_mode = process.stdout.clone();
            let stderr_mode = process.stderr.clone();
            let limits = process.limits;
            let mut child = super::super::spawn_provider_process(&process, &stdin_mode)
                .await
                .expect("spawn provider");
            let tasks = super::super::spawn_provider_io_tasks(
                &mut child,
                stdin_mode,
                stdout_mode,
                stderr_mode,
                limits,
                super::super::ProviderProcessFraming::default(),
            )
            .expect("spawn provider I/O tasks");

            while child.try_wait().expect("observe child status").is_none() {
                tokio::task::yield_now().await;
            }

            let output = super::super::collect_provider_output(child, tasks, limits, start)
                .await
                .expect("completed child must not be classified as a timeout");
            assert!(output.status.success());
            assert!(!output.receipt.timed_out);
        }
    });
    let _ = fs::remove_dir_all(root);
}
