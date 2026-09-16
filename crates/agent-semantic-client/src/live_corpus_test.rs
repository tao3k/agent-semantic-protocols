// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Feature-gated namespace for the test-owned Live Corpus runner.

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
