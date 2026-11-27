// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Thin production `asp-gateway` daemon entrypoint.

use agent_semantic_ast::run_asp_gateway_from_args;

#[tokio::main]
async fn main() -> Result<(), String> {
    run_asp_gateway_from_args(std::env::args().skip(1)).await
}
