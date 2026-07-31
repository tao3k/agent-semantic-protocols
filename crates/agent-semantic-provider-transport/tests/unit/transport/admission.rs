use std::fs;
use std::time::Duration;

use crate::run_provider_process_async;

use super::support::{script, spec, temp_dir};

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
    let pid_path = root.join("provider.pid");
    let program = script(
        &root,
        "provider.sh",
        &format!(
            "#!/bin/sh\nprintf '%s' \"$$\" > '{}'\nexec sleep 30\n",
            pid_path.display()
        ),
    );
    let task = tokio::spawn(run_provider_process_async(spec(program, root.clone())));
    let pid = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(pid) = tokio::fs::read_to_string(&pid_path).await
                && let Ok(pid) = pid.parse::<i32>()
            {
                break pid;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("provider must publish pid");

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
    let _ = fs::remove_dir_all(root);
}
