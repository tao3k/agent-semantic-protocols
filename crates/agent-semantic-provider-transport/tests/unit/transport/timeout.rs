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
