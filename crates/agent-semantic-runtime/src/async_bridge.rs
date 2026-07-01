//! Runtime-owned bridge for synchronous callers that must drive async work.

use std::future::Future;

/// Run one async operation on a runtime-owned current-thread Tokio runtime.
pub fn runtime_block_on_current_thread<F>(future: F) -> Result<F::Output, String>
where
    F: Future,
{
    if tokio::runtime::Handle::try_current().is_ok() {
        return Err("runtime async bridge called from an active Tokio runtime".to_string());
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("failed to build runtime async bridge: {error}"))?;
    Ok(runtime.block_on(future))
}
