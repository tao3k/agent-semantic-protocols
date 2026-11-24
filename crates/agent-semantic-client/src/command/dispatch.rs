// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Top-level command dispatch for protocol subcommands.

use std::env;

use super::ast_patch::run_ast_patch_command;
use super::document_provider;
use super::healthcheck::run_healthcheck_command;
use super::install_provider::run_install_command;
use super::paths::run_paths_command;
use super::provider_dispatch::run_language_command;
use super::root_language_facade::{run_workspace_query, run_workspace_search_playbook};
use super::run_protocol_version_command;
use super::runtime_server::run_runtime_server_command;
use super::schema::run_schema_command;

pub(crate) async fn run_protocol_command(args: Vec<String>) -> Result<(), String> {
    run_protocol_command_started(args, tokio::time::Instant::now()).await
}

pub(crate) async fn run_protocol_command_started(
    args: Vec<String>,
    process_started: tokio::time::Instant,
) -> Result<(), String> {
    if super::cli_help::print_help_if_requested(&args)? {
        return Ok(());
    }
    match args.first().map(String::as_str) {
        Some("help" | "--help" | "-h") => {
            println!("{}", usage());
            Ok(())
        }
        Some("version" | "--version" | "-V") => run_protocol_version_command(&args[1..]),
        Some("providers" | "doctor" | "cache" | "clean" | "cloud" | "tools" | "wrap") => {
            run_client_command(args).await
        }
        Some("search") => run_workspace_search_playbook(&args[1..]).await,
        Some("query") => run_workspace_query(&args[1..]).await,
        Some("install") => run_install_command(&args[1..]).await,
        Some("paths") => run_paths_command(&args[1..]),
        Some("healthcheck") => run_healthcheck_command(&args[1..]).await,
        Some("server") => run_runtime_server_command(&args[1..]).await,
        Some("schema") => run_schema_command(&args[1..]).await,
        Some("ast-patch") => run_ast_patch_command(&args[1..]),
        Some(document_id) if document_provider::is_document_language(document_id) => {
            document_provider::run_language_command(document_id, &args[1..]).await
        }
        Some(language_id) => run_language_command(language_id, &args[1..], process_started).await,
        _ => Err(usage()),
    }
}

fn usage() -> String {
    "usage: asp [--help|--version] <providers|tools|wrap|cache|clean|cloud|install|paths|healthcheck|server|schema|ast-patch|search|query|gerbil-scheme|julia|md|org|python|rust|typescript> ...".to_string()
}

async fn run_client_command(args: Vec<String>) -> Result<(), String> {
    let cwd = env::current_dir()
        .map_err(|error| format!("failed to resolve current directory: {error}"))?;
    agent_semantic_client::run_client_cli_args(None, args, cwd).await
}
