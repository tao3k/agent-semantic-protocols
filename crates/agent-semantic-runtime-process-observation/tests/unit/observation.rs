// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use crate::{RuntimeSchedulerObservation, linux_statm_resident_bytes, observe_process_memory};

#[test]
fn linux_statm_pages_are_checked_before_conversion() {
    assert_eq!(
        linux_statm_resident_bytes("4096 1024 128 32 0 512 0\n", 4096),
        Some(4 * 1024 * 1024)
    );
    assert_eq!(linux_statm_resident_bytes("4096", 4096), None);
    assert_eq!(linux_statm_resident_bytes("4096 invalid", 4096), None);
    assert_eq!(
        linux_statm_resident_bytes("1 18446744073709551615", 2),
        None
    );
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn resident_process_observation_is_available() {
    let observation = observe_process_memory(
        0,
        RuntimeSchedulerObservation {
            worker_threads: 1,
            alive_tasks: 0,
            global_queue_depth: 0,
        },
    )
    .expect("supported platforms expose a process observation");
    assert!(observation.resident_bytes.is_some_and(|bytes| bytes > 0));
}
