// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Compatibility example wrapper delegating directly to ASP's production
//! supervised gateway runtime (`asp-gateway`).

use std::path::{Path, PathBuf};

use agent_semantic_ast::{GatewayLaunchConfig, run_supervised_asp_gateway};

fn asp_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("ASP repo root")
        .to_path_buf()
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let config = GatewayLaunchConfig::production(asp_root());
    run_supervised_asp_gateway(config).await
}
