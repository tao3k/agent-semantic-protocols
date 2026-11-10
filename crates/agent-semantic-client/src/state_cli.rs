// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Thin state-command dispatch for the `asp` binary.
//!
//! Parsing and rendering live here. State materialization and contract
//! convergence are application services backed by Artifacts.

use std::ffi::OsString;

/// Run the `asp` binary with pre-dispatch for State Core commands.
pub async fn run_binary_from_env() -> Result<(), String> {
    run_binary_from_env_started(tokio::time::Instant::now()).await
}

async fn run_binary_from_env_started(process_started: tokio::time::Instant) -> Result<(), String> {
    if let Some(result) = run_state_command_from_env() {
        return result;
    }
    crate::cli::run_cli_from_env_started(process_started).await
}

fn run_state_command_from_env() -> Option<Result<(), String>> {
    let mut args = std::env::args_os();
    let _program = args.next();
    let command = args.next()?;
    if command != "state" {
        return None;
    }
    Some(run_state_command(args.collect()))
}

fn run_state_command(args: Vec<OsString>) -> Result<(), String> {
    let Some(subcommand) = args.first() else {
        return Err(state_usage());
    };
    let mut json = false;
    for arg in args.iter().skip(1) {
        match arg.to_str() {
            Some("--json") => json = true,
            Some("--help") | Some("-h") => {
                println!("{}", state_usage());
                return Ok(());
            }
            Some(other) => return Err(format!("unknown asp state option: {other}")),
            None => return Err("asp state received non-utf8 option".to_string()),
        }
    }

    match subcommand.to_str() {
        Some("locate") => render_locate(json),
        Some("sync") => render_sync(json),
        Some("--help" | "-h") => {
            println!("{}", state_usage());
            Ok(())
        }
        _ => Err(state_usage()),
    }
}

fn render_locate(json: bool) -> Result<(), String> {
    let report = crate::state_service::locate_current_workspace()?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|error| format!("serialize state locate report: {error}"))?
        );
    } else {
        println!("stateHome: {}", report.state_home.display());
        println!("repoId: {}", report.repo_id);
        println!("workspaceId: {}", report.workspace_id);
        println!("scopeId: {}", report.scope_id);
        println!("repoDisplayName: {}", report.repo_display_name);
        println!("workspaceDisplayName: {}", report.workspace_display_name);
        println!("checkoutRoot: {}", report.checkout_root.display());
        if let Some(git_toplevel) = &report.git_toplevel {
            println!("gitToplevel: {}", git_toplevel.display());
        }
        if let Some(git_dir) = &report.git_dir {
            println!("gitDir: {}", git_dir.display());
        }
        if let Some(remote_url) = &report.remote_url {
            println!("remoteUrl: {remote_url}");
        }
        println!("dbPath: {}", report.db_path.display());
        println!("artifactPath: {}", report.artifact_path.display());
        println!("manifestPath: {}", report.manifest_path.display());
        println!("backend: {}", report.backend);
        println!(
            "generationManifestPath: {}",
            report.generation_manifest_path.display()
        );
    }
    Ok(())
}

fn render_sync(json: bool) -> Result<(), String> {
    let receipt = crate::state_service::sync_state_home_contract()?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&receipt)
                .map_err(|error| format!("serialize state sync receipt: {error}"))?
        );
    } else {
        println!(
            "[asp-state-sync] state={} stateHome={} removedEntries={} removedBytes={}",
            receipt.state,
            receipt.state_home.display(),
            receipt.removed_entries.len(),
            receipt.removed_bytes
        );
    }
    Ok(())
}

fn state_usage() -> String {
    "usage: asp state <locate|sync> [--json]\nretention: asp cache clean --day <days>".to_string()
}
