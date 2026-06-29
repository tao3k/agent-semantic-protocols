//! Thin state-command dispatch for the `asp` binary.

use std::ffi::OsString;

/// Run the `asp` binary with pre-dispatch for State Core commands.
pub fn run_binary_from_env() -> Result<(), String> {
    if let Some(result) = run_state_command_from_env() {
        return result;
    }
    crate::run_cli_from_env()
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
    if subcommand != "locate" {
        return Err(state_usage());
    }

    let mut json = false;
    for arg in args.iter().skip(1) {
        match arg.to_str() {
            Some("--json") => json = true,
            Some("--help") | Some("-h") => {
                println!("{}", state_usage());
                return Ok(());
            }
            Some(other) => return Err(format!("unknown asp state locate option: {other}")),
            None => return Err("asp state locate received non-utf8 option".to_string()),
        }
    }

    let cwd = std::env::current_dir().map_err(|error| format!("resolve cwd: {error}"))?;
    let report = agent_semantic_client_core::state_core::locate_state(cwd, true)?;
    if json {
        let body = serde_json::to_string_pretty(&report)
            .map_err(|error| format!("serialize state locate report: {error}"))?;
        println!("{body}");
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
        println!("futureBackend: {}", report.future_backend);
        println!(
            "generationManifestPath: {}",
            report.generation_manifest_path.display()
        );
        match &report.project_local_cache {
            Some(cache) => println!(
                "projectLocalCache: {} exists={}",
                cache.path.display(),
                cache.exists
            ),
            None => println!("projectLocalCache: none"),
        }
    }

    Ok(())
}

fn state_usage() -> String {
    "usage: asp state locate [--json]".to_string()
}
