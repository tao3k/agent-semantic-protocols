use crate::{RuntimeOperationTimeoutPolicy, runtime_operation_timeout_receipt};

#[test]
fn runtime_timeout_receipt_records_within_budget_operation() {
    let policy = RuntimeOperationTimeoutPolicy {
        operation: "owner-items-provider".to_string(),
        max_elapsed_ms: 10,
        cancel_after_ms: 25,
    };

    let receipt = runtime_operation_timeout_receipt(&policy, 4);

    assert_eq!(receipt.operation, "owner-items-provider");
    assert_eq!(receipt.elapsed_ms, 4);
    assert!(!receipt.timed_out);
    assert!(!receipt.cancellation_required);
}

#[test]
fn runtime_timeout_receipt_separates_timeout_from_cancellation() {
    let policy = RuntimeOperationTimeoutPolicy {
        operation: "owner-items-provider".to_string(),
        max_elapsed_ms: 10,
        cancel_after_ms: 25,
    };

    let timeout = runtime_operation_timeout_receipt(&policy, 12);
    let cancellation = runtime_operation_timeout_receipt(&policy, 25);

    assert!(timeout.timed_out);
    assert!(!timeout.cancellation_required);
    assert!(cancellation.timed_out);
    assert!(cancellation.cancellation_required);
}
