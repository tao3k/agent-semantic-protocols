// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

pub(crate) async fn run_runtime_server_command(args: &[String]) -> Result<(), String> {
    crate::server::runtime_server::run_runtime_server_command(args).await
}
