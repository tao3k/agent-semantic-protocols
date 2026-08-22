use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use agent_semantic_hook_testkit::{HookScenario, HookTestKitError, run_scenarios_with};
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
