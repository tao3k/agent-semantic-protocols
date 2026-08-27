use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use agent_semantic_hook_testkit::{
    HookProcessSpec, HookScenario, HookTestKitError, run_hook_process, run_scenarios_with,
};
use serde_json::json;

fn scenario(index: usize) -> HookScenario {
    HookScenario {
        scenario_id: format!("scenario-{index}"),
        payload: json!({"index": index}),
        expected_rule_id: Some(format!("rule-{index}")),
        forbidden_rule_id: None,
        expected_provider_id: None,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn scenario_concurrency_is_bounded_and_receipts_preserve_order() {
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let execute = {
        let active = Arc::clone(&active);
        let peak = Arc::clone(&peak);
        move |payload: serde_json::Value| {
            let active = Arc::clone(&active);
            let peak = Arc::clone(&peak);
            async move {
                let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(5)).await;
                active.fetch_sub(1, Ordering::SeqCst);
                let index = payload["index"].as_u64().expect("scenario index");
                Ok(json!({
                    "fields": {"configRuleId": format!("rule-{index}")},
                    "routes": [],
                }))
            }
        }
    };
    let receipts = run_scenarios_with(
        (0..16).map(scenario).collect(),
        4,
        Duration::from_millis(100),
        execute,
    )
    .await
    .expect("run concurrent scenarios");
    assert_eq!(peak.load(Ordering::SeqCst), 4);
    for (index, receipt) in receipts.iter().enumerate() {
        assert_eq!(receipt.scenario_id, format!("scenario-{index}"));
    }
}

#[tokio::test]
async fn stuck_async_scenario_is_cancelled_at_its_deadline() {
    let error = run_scenarios_with(vec![scenario(0)], 1, Duration::from_millis(10), |_| async {
        std::future::pending::<()>().await;
        Ok(json!({}))
    })
    .await
    .expect_err("stuck scenario must time out");
    assert!(matches!(error, HookTestKitError::Timeout { .. }));
}

#[cfg(unix)]
#[tokio::test]
async fn timed_out_hook_kills_process_group() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("temp dir");
    let script = dir.path().join("hook.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\nsleep 30 &\nchild=$!\nprintf '%s' \"$child\" > \"$CHILD_PID_FILE\"\nwait \"$child\"\n",
    )
    .expect("write hook script");
    let mut permissions = std::fs::metadata(&script)
        .expect("script metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&script, permissions).expect("chmod hook script");

    let pid_file = dir.path().join("child.pid");
    let mut spec = HookProcessSpec::new(&script, dir.path());
    spec.env
        .push(("CHILD_PID_FILE".to_owned(), pid_file.display().to_string()));
    // This fixture must first prove that a descendant exists before asserting
    // group cleanup. Keep the production default at two seconds; allow this
    // scheduler-pressure oracle a bounded five-second launch window.
    spec.timeout = Duration::from_secs(5);
    let result = run_hook_process(&spec, &json!({})).await;
    assert!(matches!(result, Err(HookTestKitError::Timeout { .. })));

    let child_pid: i32 = std::fs::read_to_string(pid_file)
        .expect("child pid")
        .parse()
        .expect("valid child pid");
    assert!(child_pid > 0);
    for _ in 0..20 {
        // SAFETY: signal 0 only probes the validated positive fixture pid.
        if unsafe { libc::kill(child_pid, 0) } != 0 {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("descendant process survived timeout cleanup: {child_pid}");
}
