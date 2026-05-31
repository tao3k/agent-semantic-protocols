use std::env;
use std::fs;
use std::io::{self, Read};
use std::process;

use semantic_agent_hook::{classify_hook, parse_payload, parse_profiles};

fn main() {
    if let Err(message) = run() {
        eprintln!("{message}");
        process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match args.first().map(String::as_str) {
        Some("hook") => run_hook(&args[1..]),
        Some("doctor") => run_doctor(&args[1..]),
        Some("install") => {
            Err("install is specified by RFC but not implemented in this scaffold".to_string())
        }
        _ => Err(
            "usage: semantic-agent-hook hook --client <client> <event> --profiles <path>"
                .to_string(),
        ),
    }
}

fn run_hook(args: &[String]) -> Result<(), String> {
    let client = flag_value(args, "--client")
        .ok_or_else(|| "missing required --client <client>".to_string())?;
    let profiles_path = flag_value(args, "--profiles")
        .ok_or_else(|| "missing required --profiles <path>".to_string())?;
    let event = first_positional(args).ok_or_else(|| "missing hook event".to_string())?;
    let registry = load_profiles(profiles_path)?;
    let mut stdin = String::new();
    io::stdin()
        .read_to_string(&mut stdin)
        .map_err(|error| format!("failed to read hook payload from stdin: {error}"))?;
    let payload =
        parse_payload(&stdin).map_err(|error| format!("invalid hook payload JSON: {error:?}"))?;
    let decision = classify_hook(&registry, client, event, &payload);
    let output = serde_json::to_string_pretty(&decision)
        .map_err(|error| format!("failed to serialize hook decision: {error}"))?;
    println!("{output}");
    Ok(())
}

fn run_doctor(args: &[String]) -> Result<(), String> {
    let profiles_path = flag_value(args, "--profiles")
        .ok_or_else(|| "missing required --profiles <path>".to_string())?;
    let registry = load_profiles(profiles_path)?;
    println!(
        "semantic-agent-hook profiles={} projectRoot={}",
        registry.profiles.len(),
        registry.project_root
    );
    Ok(())
}

fn load_profiles(path: &str) -> Result<semantic_agent_hook::ProfileRegistry, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read profile registry {path}: {error}"))?;
    parse_profiles(&contents).map_err(|error| format!("invalid profile registry JSON: {error:?}"))
}

fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|window| window[0] == flag)
        .map(|window| window[1].as_str())
}

fn first_positional(args: &[String]) -> Option<&str> {
    let mut skip_next = false;
    for arg in args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if matches!(arg.as_str(), "--client" | "--profiles") {
            skip_next = true;
            continue;
        }
        if !arg.starts_with('-') {
            return Some(arg.as_str());
        }
    }
    None
}
