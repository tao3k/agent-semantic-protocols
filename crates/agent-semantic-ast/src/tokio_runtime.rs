// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Dedicated Tokio runtime boundary owned by `agent-semantic-ast` for blocking
//! transport hops to external native language providers.

use crate::types::AstEngineError;

/// Execute one bounded async provider transport call on a dedicated Tokio
/// runtime, isolating callers that already execute inside an active Tokio task.
pub(crate) fn run_provider_io<MakeFut, Fut, T>(make_fut: MakeFut) -> Result<T, AstEngineError>
where
    MakeFut: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<T, AstEngineError>>,
    T: Send + 'static,
{
    if tokio::runtime::Handle::try_current().is_ok() {
        return std::thread::spawn(move || execute_on_current_thread_runtime(make_fut))
            .join()
            .map_err(|_| {
                AstEngineError::ProviderError("provider I/O worker panicked".to_owned())
            })?;
    }
    execute_on_current_thread_runtime(make_fut)
}

fn execute_on_current_thread_runtime<MakeFut, Fut, T>(
    make_fut: MakeFut,
) -> Result<T, AstEngineError>
where
    MakeFut: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, AstEngineError>>,
{
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| AstEngineError::ProviderError(err.to_string()))?
        .block_on(make_fut())
}
