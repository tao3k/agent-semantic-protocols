use std::fs;
use std::time::{Duration, Instant};

use crate::run_provider_process;

use super::support::{script, spec, temp_dir};

#[test]
fn provider_process_hot_path_stays_inside_performance_gate() {
    let root = temp_dir("hot-path-performance");
    let program = script(&root, "provider.sh", "#!/bin/sh\nprintf ok\n");
    let iterations = 8;
    let per_run_gate = Duration::from_secs(3);
    let batch_gate = Duration::from_secs(8);
    let started_at = Instant::now();

    for _ in 0..iterations {
        let output =
            run_provider_process(spec(program.clone(), root.clone())).expect("run provider");
        assert!(output.status.success());
        assert_eq!(output.stdout.as_ref(), b"ok");
        assert!(
            output.receipt.elapsed < per_run_gate,
            "provider process exceeded {per_run_gate:?}; receipt={:?}",
            output.receipt
        );
    }

    let elapsed = started_at.elapsed();
    assert!(
        elapsed < batch_gate,
        "{iterations} provider process runs exceeded {batch_gate:?}; elapsed={elapsed:?}"
    );
    let _ = fs::remove_dir_all(root);
}
