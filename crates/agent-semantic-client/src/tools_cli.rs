//! Tool health diagnostics for ASP-owned search helpers.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

const REQUIRED_TOOLS: &[&str] = &["fd", "rg", "fzf", "eza", "graph-turbo"];

pub(crate) fn run_tools(project_root: &Path, args: &[String]) -> Result<(), String> {
    match args {
        [] => {
            print_tools_doctor(project_root, std::env::var_os("PATH"));
            Ok(())
        }
        [subcommand] if subcommand == "doctor" => {
            print_tools_doctor(project_root, std::env::var_os("PATH"));
            Ok(())
        }
        [subcommand, root] if subcommand == "doctor" => {
            print_tools_doctor(&project_root.join(root), std::env::var_os("PATH"));
            Ok(())
        }
        _ => Err("usage: asp tools doctor [PROJECT_ROOT]".to_string()),
    }
}

pub(crate) fn tools_summary_line() -> String {
    let statuses = tool_statuses(std::env::var_os("PATH"));
    let missing = statuses
        .iter()
        .filter(|status| status.path.is_none())
        .count();
    let status = if missing == 0 { "ok" } else { "missing" };
    format!(
        "|tools status={status} required={} missing={missing}",
        REQUIRED_TOOLS.join(",")
    )
}

fn print_tools_doctor(project_root: &Path, path: Option<OsString>) {
    let statuses = tool_statuses(path);
    let missing = statuses
        .iter()
        .filter(|status| status.path.is_none())
        .count();
    let status = if missing == 0 { "ok" } else { "missing" };
    println!(
        "[asp-tools] status={status} required={} root={}",
        REQUIRED_TOOLS.join(","),
        project_root.display()
    );
    for tool in statuses {
        match tool.path {
            Some(path) => println!(
                "|tool name={} status=ok path={}",
                tool.name,
                compact_path(&path)
            ),
            None => println!("|tool name={} status=missing", tool.name),
        }
    }
    if missing > 0 {
        println!("|cmd doctor=asp tools doctor");
        println!("|rule fallback=disabled installRequired=true");
    }
}

fn tool_statuses(path: Option<OsString>) -> Vec<ToolStatus> {
    REQUIRED_TOOLS
        .iter()
        .map(|tool| ToolStatus {
            name: tool,
            path: find_executable(tool, path.as_ref()),
        })
        .collect()
}

fn find_executable(tool: &str, path: Option<&OsString>) -> Option<PathBuf> {
    let path = path?;
    for dir in std::env::split_paths(path) {
        let candidate = dir.join(tool);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn compact_path(path: &Path) -> String {
    path.to_string_lossy().replace(' ', "\\ ")
}

struct ToolStatus {
    name: &'static str,
    path: Option<PathBuf>,
}
