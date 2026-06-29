//! Tool health diagnostics for ASP-owned search helpers.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

const REQUIRED_TOOLS: &[&str] = &["fd", "rg", "eza", "asp-graph-turbo"];

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
        [subcommand, rest @ ..] if subcommand == "wrap" => run_wrap(rest),
        _ => Err(
            "usage: asp tools <doctor [PROJECT_ROOT]|wrap asp-graph-turbo [--] [ARGS...]>"
                .to_string(),
        ),
    }
}

pub(crate) fn run_wrap(args: &[String]) -> Result<(), String> {
    run_wrap_with_path(args, std::env::var_os("PATH"))
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
        required_tool_names().join(",")
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
        required_tool_names().join(","),
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

pub(crate) fn run_wrap_with_path(args: &[String], path: Option<OsString>) -> Result<(), String> {
    let (tool_name, tool_args) = args.split_first().ok_or_else(|| wrap_usage().to_string())?;
    let tool = wrapper_tool(tool_name)
        .ok_or_else(|| format!("asp wrap supports only asp-graph-turbo\n{}", wrap_usage()))?;
    let tool_args = if tool_args.first().is_some_and(|arg| arg == "--") {
        &tool_args[1..]
    } else {
        tool_args
    };
    let status = Command::new(
        find_executable(tool, path.as_ref()).ok_or_else(|| {
            "asp wrap asp-graph-turbo requires asp-graph-turbo on PATH; run just agent-tools-install-asp-graph-turbo <bin-dir>".to_string()
        })?,
    )
    .args(tool_args)
    .status()
    .map_err(|error| format!("failed to execute {tool}: {error}"))?;
    if status.success() {
        return Ok(());
    }
    match status.code() {
        Some(code) => Err(format!("{tool} exited with status {code}")),
        None => Err(format!("{tool} terminated by signal")),
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

fn wrapper_tool(tool_name: &str) -> Option<&'static str> {
    (tool_name == "asp-graph-turbo").then_some("asp-graph-turbo")
}

fn find_executable(executable: &str, path: Option<&OsString>) -> Option<PathBuf> {
    let path = path?;
    for dir in std::env::split_paths(path) {
        let candidate = dir.join(executable);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn required_tool_names() -> Vec<&'static str> {
    REQUIRED_TOOLS.to_vec()
}

fn wrap_usage() -> &'static str {
    "usage: asp wrap asp-graph-turbo [--] [ARGS...]"
}

fn compact_path(path: &Path) -> String {
    path.to_string_lossy().replace(' ', "\\ ")
}

struct ToolStatus {
    name: &'static str,
    path: Option<PathBuf>,
}
