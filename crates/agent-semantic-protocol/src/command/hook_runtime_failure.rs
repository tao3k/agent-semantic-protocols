use std::any::Any;
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::task::Poll;

use agent_semantic_hook::{HookExecutionFailure, HookExecutionFailureKind, HookExecutionPhase};
use tracing::Instrument;

pub(super) async fn observe_hook_execution<F>(
    event: Option<String>,
    client: Option<String>,
    execution: F,
) -> Result<(), String>
where
    F: Future<Output = Result<(), String>>,
{
    let hook_span = tracing::info_span!(
        "asp.hook.execute",
        otel.kind = "INTERNAL",
        hook.event = ?event,
        hook.client = ?client,
        hook.runtime_artifact_fingerprint = %agent_semantic_hook::hook_runtime_artifact_fingerprint(),
    );
    let terminal = catch_future_unwind(execution)
        .instrument(hook_span.clone())
        .await;

    let failure = match terminal {
        Ok(Ok(())) => return Ok(()),
        Ok(Err(message)) => HookExecutionFailure::new(
            HookExecutionPhase::Bootstrap,
            HookExecutionFailureKind::RuntimeError,
            event,
            client,
            message,
        ),
        Err(panic_payload) => HookExecutionFailure::new(
            HookExecutionPhase::Bootstrap,
            HookExecutionFailureKind::RuntimePanic,
            event,
            client,
            panic_payload_message(panic_payload.as_ref()),
        ),
    };
    tracing::error!(
        parent: &hook_span,
        event.name = "asp.hook.execution_failure",
        otel.status_code = "ERROR",
        hook.failure = %failure,
        "Hook execution reached a typed failure terminal"
    );
    Err(failure.to_string())
}

async fn catch_future_unwind<F>(future: F) -> Result<F::Output, Box<dyn Any + Send>>
where
    F: Future,
{
    let mut future = Box::pin(future);
    std::future::poll_fn(move |context| {
        match catch_unwind(AssertUnwindSafe(|| future.as_mut().poll(context))) {
            Ok(Poll::Ready(output)) => Poll::Ready(Ok(output)),
            Ok(Poll::Pending) => Poll::Pending,
            Err(payload) => Poll::Ready(Err(payload)),
        }
    })
    .await
}

fn panic_payload_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_owned()
    } else {
        "Hook runtime panicked with a non-string payload".to_owned()
    }
}

#[cfg(test)]
#[path = "../../tests/unit/command/hook_runtime_failure.rs"]
mod tests;
