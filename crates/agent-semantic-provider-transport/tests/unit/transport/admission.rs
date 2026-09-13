// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::time::Duration;

use crate::ProviderProcessSupervisor;

use super::support::script;
use super::support::spec;
use super::support::temp_dir;

#[test]
fn provider_admission_capacity_is_machine_and_memory_adaptive() {
    assert_eq!(
        super::super::provider_process_admission_slots_for(
            16,
            Some(32 * 1024 * 1024 * 1024),
            Some(1024 * 1024 * 1024),
        ),
        16
    );
    assert_eq!(
        super::super::provider_process_admission_slots_for(
            16,
            Some(3 * 1024 * 1024 * 1024),
            Some(1024 * 1024 * 1024),
        ),
        3
    );
    assert_eq!(
        super::super::provider_process_admission_slots_for(
            1,
            Some(512 * 1024 * 1024),
            Some(1024 * 1024 * 1024),
        ),
        1
    );
}

#[cfg(unix)]
#[tokio::test]
async fn cancelling_async_transport_kills_the_provider_process() {
    let root = temp_dir("cancel-kills-provider");
    let program = script(
        &root,
        "provider.sh",
        "#!/bin/sh\nexec /usr/bin/tail -f /dev/null\n",
    );
    let supervisor = ProviderProcessSupervisor::default();
    let mut started = supervisor.subscribe_started();
    let task_root = root.clone();
    let task = tokio::spawn({
        let supervisor = supervisor.clone();
        async move { supervisor.run(spec(program, task_root)).await }
    });
    let pid = tokio::time::timeout(Duration::from_secs(2), started.recv())
        .await
        .expect("provider must publish a start event")
        .expect("provider start event channel must remain open")
        .process_id
        .and_then(|pid| i32::try_from(pid).ok())
        .expect("provider start event must carry a valid pid");

    task.abort();
    assert!(
        task.await
            .expect_err("provider task must cancel")
            .is_cancelled()
    );
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let alive = unsafe { libc::kill(pid, 0) } == 0;
            if !alive {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("cancelled provider must exit");
    let _ = std::fs::remove_dir_all(root);
}
