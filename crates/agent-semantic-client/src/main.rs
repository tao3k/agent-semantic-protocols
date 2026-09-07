#![deny(dead_code)]
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Process entry for the public `asp` client.
//!
//! Host Hook events belong exclusively to the sibling `asp-hook` executable.

fn main() -> std::process::ExitCode {
    let daemon = std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("server"))
        && std::env::args_os().nth(2).as_deref() == Some(std::ffi::OsStr::new("daemon"));
    let mut runtime_builder = if daemon {
        agent_semantic_client_db::runtime_server_runtime::RuntimeServerRuntimeBuilder::new_daemon()
    } else {
        agent_semantic_client_db::runtime_server_runtime::RuntimeServerRuntimeBuilder::new_client()
    };
    let runtime = match runtime_builder.enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!(
                "{}",
                agent_semantic_client::cli_failure::materialize_cli_failure(&format!(
                    "failed to create ASP CLI runtime: {error}"
                ))
            );
            return std::process::ExitCode::from(2);
        }
    };
    let command = runtime.block_on(agent_semantic_client::run_binary_from_env());
    match command {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(message) => {
            eprintln!(
                "{}",
                agent_semantic_client::cli_failure::materialize_cli_failure(&message)
            );
            std::process::ExitCode::from(2)
        }
    }
}
