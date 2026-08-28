#[path = "aot_evaluator.rs"]
mod aot_evaluator;
#[path = "reader_probe_core.rs"]
mod reader_probe;

use std::io::Read as _;
use std::path::PathBuf;

pub fn main_entry() {
    if inherited_no_agent_bypass() {
        println!(
            "{{\"schemaId\":\"agent.semantic-protocols.hook.decision\",\"schemaVersion\":1,\"decision\":\"allow\",\"reasonKind\":\"process-no-agent-bypass\",\"terminal\":\"bypassed-before-hook-generation-load\"}}"
        );
        return;
    }
    if std::env::args_os().any(|argument| argument == "--version") {
        println!("asp-hook-evaluator schema=1");
        return;
    }
    let mut invocation = std::env::args_os().skip(1);
    let first = invocation
        .next()
        .and_then(|argument| argument.into_string().ok());
    let event = if first.as_deref() == Some("hook") {
        invocation
            .next()
            .and_then(|argument| argument.into_string().ok())
    } else {
        first
    };
    match event.as_deref() {
        Some("pre-tool") => {}
        Some("permission") | Some("permission-request") => {
            println!(
                "{{\"schemaId\":\"agent.semantic-protocols.hook.decision\",\"schemaVersion\":1,\"decision\":\"deny\",\"reasonKind\":\"permission-request-denied\",\"terminal\":\"permission-request-denied\",\"processLaunched\":false}}"
            );
            return;
        }
        _ => return,
    }
    if let Err(error) = evaluate() {
        let receipt = serde_json::json!({
            "schemaId": "agent.semantic-protocols.hook.execution-failure",
            "schemaVersion": 1,
            "decision": "deny",
            "state": "failed",
            "phase": "bootstrap",
            "reasonKind": "hook-generation-unavailable",
            "terminal": "hook-generation-unavailable",
            "processLaunched": false,
            "message": error,
        });
        println!("{receipt}");
        std::process::exit(2);
    }
}

fn inherited_no_agent_bypass() -> bool {
    std::env::var_os("ASP_NO_AGENT").is_some_and(|value| value == "1")
}

fn evaluate() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let mut generation_path = None;
    let mut host_matcher = None;
    while let Some(argument) = args.next() {
        match argument.to_str() {
            Some("--generation") => generation_path = args.next().map(PathBuf::from),
            Some("--host-match") => {
                host_matcher = args.next().and_then(|value| value.into_string().ok());
            }
            _ => {}
        }
    }
    let generation_path = generation_path
        .or_else(|| std::env::var_os("ASP_HOOK_GENERATION_ROOT").map(PathBuf::from))
        .map(Ok)
        .unwrap_or_else(implicit_generation_path)?;
    let host_matcher = host_matcher.ok_or("Host matcher is not configured")?;
    let generation = load_generation(&generation_path)?;
    let mut payload_json = String::new();
    std::io::stdin()
        .read_to_string(&mut payload_json)
        .map_err(|error| format!("failed to read Hook payload: {error}"))?;
    if let Some(request) =
        aot_evaluator::reader_probe_request(&generation, &payload_json, &host_matcher)?
    {
        let mut payload: serde_json::Value = serde_json::from_str(&payload_json)
            .map_err(|error| format!("decode Hook payload for Reader probe: {error}"))?;
        let observation =
            reader_probe::diagnose_reader_probe(request.command_tokens, request.subject);
        reader_probe::bind_reader_probe_observation(&mut payload, observation.as_ref())?;
        payload_json = serde_json::to_string(&payload)
            .map_err(|error| format!("encode Hook payload with Reader probe: {error}"))?;
    }
    if let Some(decision) =
        aot_evaluator::evaluate_pre_tool(&generation, &payload_json, &host_matcher)?
    {
        let encoded = serde_json::to_string(&decision).map_err(|error| error.to_string())?;
        println!("{encoded}");
    }
    Ok(())
}

fn implicit_generation_path() -> Result<PathBuf, String> {
    let launcher_executable = std::env::current_exe()
        .map_err(|error| format!("resolve Hook evaluator executable: {error}"))?;
    let immutable_evaluator = std::fs::canonicalize(&launcher_executable).map_err(|error| {
        format!(
            "resolve Hook evaluator immutable candidate {}: {error}",
            launcher_executable.display()
        )
    })?;
    let metadata = std::fs::symlink_metadata(&immutable_evaluator).map_err(|error| {
        format!(
            "inspect Hook evaluator immutable candidate {}: {error}",
            immutable_evaluator.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "Hook evaluator immutable candidate {} is not a regular non-symlink file",
            immutable_evaluator.display()
        ));
    }
    let parent = immutable_evaluator.parent().ok_or_else(|| {
        format!(
            "Hook evaluator immutable candidate {} has no parent directory",
            immutable_evaluator.display()
        )
    })?;
    Ok(parent.join("compiled-hook-generation.json"))
}

fn load_generation(path: &std::path::Path) -> Result<String, String> {
    let current = path
        .parent()
        .ok_or("HookGeneration path has no current directory")?;
    let current_metadata = std::fs::symlink_metadata(current)
        .map_err(|error| format!("failed to inspect HookGeneration current directory: {error}"))?;
    let generation_metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("failed to inspect compiled HookGeneration: {error}"))?;
    if !current_metadata.file_type().is_dir()
        || current_metadata.file_type().is_symlink()
        || !generation_metadata.file_type().is_file()
        || generation_metadata.file_type().is_symlink()
    {
        return Err("active HookGeneration is not a regular immutable publication".to_owned());
    }
    std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read HookGeneration {}: {error}", path.display()))
}
