//! Plugin command routing owned by the `asp` binary.

use super::hook_runtime::run_codex_plugin_install_args;

pub(crate) fn run_plugin_command(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("help" | "--help" | "-h") | None => {
            println!("{}", usage());
            Ok(())
        }
        Some("install") => run_plugin_install_command(&args[1..]),
        _ => Err(usage()),
    }
}

fn run_plugin_install_command(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("help" | "--help" | "-h") | None => {
            println!("{}", install_usage());
            Ok(())
        }
        Some("codex") => run_codex_plugin_install_args(&args[1..]),
        Some(target) => Err(format!(
            "unsupported plugin install target: {target}; expected codex"
        )),
    }
}

fn usage() -> String {
    "usage: asp plugin <install> ...".to_string()
}

fn install_usage() -> String {
    "usage: asp plugin install codex [PROJECT_ROOT] [--global|--global-plugin] [--subagent-model MODEL]".to_string()
}
