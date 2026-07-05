//! Runtime-owned timeout and cancellation receipt policy.

/// Timeout policy for one runtime operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeOperationTimeoutPolicy {
    pub(crate) operation: String,
    pub(crate) max_elapsed_ms: u128,
    pub(crate) cancel_after_ms: u128,
}

impl RuntimeOperationTimeoutPolicy {
    /// Create a timeout policy for one named runtime operation.
    #[must_use]
    pub fn new(operation: impl Into<String>, max_elapsed_ms: u128, cancel_after_ms: u128) -> Self {
        Self {
            operation: operation.into(),
            max_elapsed_ms,
            cancel_after_ms,
        }
    }
}

/// Runtime-owned receipt for timeout and cancellation accounting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeOperationTimeoutReceipt {
    pub operation: String,
    pub elapsed_ms: u128,
    pub max_elapsed_ms: u128,
    pub cancel_after_ms: u128,
    pub timed_out: bool,
    pub cancellation_required: bool,
}

/// Build a runtime timeout receipt without coupling timeout policy to commands.
#[must_use]
pub fn runtime_operation_timeout_receipt(
    policy: &RuntimeOperationTimeoutPolicy,
    elapsed_ms: u128,
) -> RuntimeOperationTimeoutReceipt {
    RuntimeOperationTimeoutReceipt {
        operation: policy.operation.clone(),
        elapsed_ms,
        max_elapsed_ms: policy.max_elapsed_ms,
        cancel_after_ms: policy.cancel_after_ms,
        timed_out: elapsed_ms > policy.max_elapsed_ms,
        cancellation_required: elapsed_ms >= policy.cancel_after_ms,
    }
}
