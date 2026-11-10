// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Feature-gated namespace for the test-owned Live Corpus runner.

#[path = "../tests/integration/live_corpus/runner.rs"]
mod runner;

/// Runs the feature-gated isolated Live Corpus process.
#[doc(hidden)]
pub fn run_isolated_live_corpus_process() -> std::process::ExitCode {
    runner::main()
}

/// Runs one isolated Live Corpus test operation outside the public `asp` CLI.
pub async fn run_live_corpus_test(args: Vec<String>) -> Result<(), String> {
    crate::command::live_corpus::run_live_corpus_test(&args).await
}

/// Runs qualification against a test-owned Runtime State Home.
pub async fn run_live_corpus_test_at(
    args: Vec<String>,
    isolated_runtime_state_home: &std::path::Path,
    resource_state_home: &std::path::Path,
) -> Result<(), String> {
    crate::command::live_corpus::run_live_corpus_test_at(
        &args,
        Some(isolated_runtime_state_home),
        Some(resource_state_home),
    )
    .await
}

/// Releases every idle Runtime data-plane session owned by this test process.
///
/// The isolated Server cannot prove a bounded clean drain while the test owner
/// itself retains a multiplexed session in the process-wide session registry.
pub async fn drain_runtime_sessions() -> usize {
    crate::runtime_language_client::drain_cached_runtime_sessions().await
}
