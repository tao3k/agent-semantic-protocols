#![deny(dead_code)]
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Cargo-owned Live Corpus qualification test process.

fn main() -> std::process::ExitCode {
    let runtime =
        match agent_semantic_workspace_scheduler::RuntimeServerRuntimeBuilder::new_client()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                eprintln!("failed to create Live Corpus test runtime: {error}");
                return std::process::ExitCode::from(2);
            }
        };
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match runtime.block_on(agent_semantic_client::run_live_corpus_test(args)) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            std::process::ExitCode::from(2)
        }
    }
}
