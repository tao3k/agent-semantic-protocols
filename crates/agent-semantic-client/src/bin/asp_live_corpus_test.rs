// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Binary root for the feature-gated, test-owned Live Corpus runner.

fn main() -> std::process::ExitCode {
    agent_semantic_client::live_corpus_test::run_isolated_live_corpus_process()
}
