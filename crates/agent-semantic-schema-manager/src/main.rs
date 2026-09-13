// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_schema_manager::run_cli;

#[tokio::main]
async fn main() {
    if let Err(error) = run_cli(std::env::args().skip(1)).await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
