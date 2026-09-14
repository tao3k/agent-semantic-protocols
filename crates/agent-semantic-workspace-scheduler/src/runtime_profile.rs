// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::future::Future;

pub struct RuntimeServerRuntime {
    runtime: tokio::runtime::Runtime,
}

impl RuntimeServerRuntime {
    pub fn block_on<F>(&self, future: F) -> F::Output
    where
        F: Future,
    {
        self.runtime.block_on(future)
    }
}

pub struct RuntimeServerRuntimeBuilder {
    builder: tokio::runtime::Builder,
}

/// Effective CPU capacity exposed to this process. The OS value accounts for
/// processor-set and cgroup limits.
#[must_use]
pub fn adaptive_tokio_worker_count() -> usize {
    std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1)
}

impl RuntimeServerRuntimeBuilder {
    pub fn new_client() -> Self {
        let mut builder = tokio::runtime::Builder::new_current_thread();
        builder.thread_name("asp-client");
        Self { builder }
    }

    pub fn new_hook_client() -> Self {
        let mut builder = tokio::runtime::Builder::new_current_thread();
        builder.thread_name("asp-hook-client");
        Self { builder }
    }

    pub fn new_daemon() -> Self {
        let worker_count = adaptive_tokio_worker_count();
        let mut builder = tokio::runtime::Builder::new_multi_thread();
        builder
            .worker_threads(worker_count)
            .max_blocking_threads(worker_count)
            .thread_name("asp-runtime-server");
        Self { builder }
    }

    pub fn enable_all(&mut self) -> &mut Self {
        self.builder.enable_all();
        self
    }

    pub fn build(&mut self) -> Result<RuntimeServerRuntime, std::io::Error> {
        self.builder
            .build()
            .map(|runtime| RuntimeServerRuntime { runtime })
    }
}

/// Borrows the caller's Tokio runtime for a short-lived control operation.
/// It owns no runtime, threads, or global state.
pub struct RuntimeServerClientExecutor(tokio::runtime::Handle);

impl RuntimeServerClientExecutor {
    pub fn get() -> Result<Self, String> {
        tokio::runtime::Handle::try_current()
            .map(Self)
            .map_err(|error| format!("ASP caller Tokio runtime is unavailable: {error}"))
    }

    pub fn block_on<F>(&self, future: F) -> F::Output
    where
        F: Future,
    {
        let handle = self.0.clone();
        tokio::task::block_in_place(move || handle.block_on(future))
    }
}
