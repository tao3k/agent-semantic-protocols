use std::fs;
use std::time::{Duration, Instant};

use crate::run_provider_process;

use super::support::{script, spec, temp_dir};

#[test]
fn provider_process_hot_path_stays_inside_performance_gate() {
    let root = temp_dir("hot-path-performance");
    let program = script(&root, "provider.sh", "#!/bin/sh\nprintf ok\n");
    let iterations = 8;
    let median_gate = Duration::from_millis(750);
    let batch_gate = Duration::from_secs(8);
    let started_at = Instant::now();
    let mut run_times = Vec::with_capacity(iterations);

    for _ in 0..iterations {
        let output =
            run_provider_process(spec(program.clone(), root.clone())).expect("run provider");
        assert!(output.status.success());
        assert_eq!(output.stdout.as_ref(), b"ok");
        run_times.push(output.receipt.elapsed);
    }

    let elapsed = started_at.elapsed();
    let mut sorted_run_times = run_times.clone();
    sorted_run_times.sort();
    let median = sorted_run_times[sorted_run_times.len() / 2];
    let slowest = sorted_run_times[sorted_run_times.len() - 1];
    assert!(
        median < median_gate,
        "provider process median exceeded {median_gate:?}; median={median:?}; slowest={slowest:?}; runs={run_times:?}"
    );
    assert!(
        elapsed < batch_gate,
        "{iterations} provider process runs exceeded {batch_gate:?}; elapsed={elapsed:?}; runs={run_times:?}"
    );
    let _ = fs::remove_dir_all(root);
}
