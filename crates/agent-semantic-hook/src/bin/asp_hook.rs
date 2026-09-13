#![deny(dead_code)]
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Thin executable boundary for the canonical Runtime Hook evaluator.

fn main() -> std::process::ExitCode {
    agent_semantic_hook::run_hook_binary_from_env()
}
