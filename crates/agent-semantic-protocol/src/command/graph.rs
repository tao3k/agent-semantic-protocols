//! `asp graph` command adapter.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use agent_semantic_provider_transport::{
    OutputMode, ProviderProcessLimits, ProviderProcessSpec, StdinMode, run_provider_process,
};
use serde_json::Value;

use crate::graph::{GraphRenderOptions, render_search_graph_packet};

const GRAPH_TURBO_REQUEST_SCHEMA_ID: &str = "agent.semantic-protocols.semantic-graph-turbo-request";

pub(crate) fn run_graph_command(args: &[String]) -> Result<(), String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(usage());
    };
    match command {
        "render" => run_graph_render_command(&args[1..]),
        "help" | "--help" | "-h" => Err(usage()),
        _ => Err(usage()),
    }
}

fn run_graph_render_command(args: &[String]) -> Result<(), String> {
    let request = GraphRenderRequest::parse(args)?;
    if request.view != "seeds" {
        return Err("graph render currently supports only --view seeds".to_string());
    }
    let packet_bytes = read_packet_bytes(&request.packet_path)?;
    let packet = parse_packet(&packet_bytes)?;
    if is_graph_turbo_request(&packet)
        && let Some(output) = render_graph_turbo_packet(&packet_bytes)?
    {
        io::stdout()
            .write_all(output.as_ref())
            .map_err(|error| format!("failed to write asp-graph-turbo stdout: {error}"))?;
        return Ok(());
    }
    let output = render_search_graph_packet(
        &packet,
        GraphRenderOptions {
            seed_limit: request.seed_limit,
        },
    );
    print!("{output}");
    Ok(())
}

struct GraphRenderRequest {
    packet_path: PathBuf,
    view: String,
    seed_limit: Option<usize>,
}

impl GraphRenderRequest {
    fn parse(args: &[String]) -> Result<Self, String> {
        let packet_path = flag_value(args, "--packet")
            .ok_or_else(|| "missing required --packet <path-or->".to_string())?;
        let view = flag_value(args, "--view").unwrap_or_else(|| "seeds".to_string());
        let seed_limit = flag_value(args, "--seeds")
            .map(|value| {
                value
                    .parse::<usize>()
                    .map_err(|error| format!("invalid --seeds value: {error}"))
            })
            .transpose()?;
        Ok(Self {
            packet_path: PathBuf::from(packet_path),
            view,
            seed_limit,
        })
    }
}

fn read_packet_bytes(path: &PathBuf) -> Result<Vec<u8>, String> {
    let mut contents = Vec::new();
    if path.as_os_str() == "-" {
        io::stdin()
            .read_to_end(&mut contents)
            .map_err(|error| format!("failed to read graph packet from stdin: {error}"))?;
    } else {
        contents = fs::read(path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    }
    Ok(contents)
}

fn parse_packet(contents: &[u8]) -> Result<Value, String> {
    serde_json::from_slice(contents).map_err(|error| format!("invalid graph packet JSON: {error}"))
}

fn is_graph_turbo_request(packet: &Value) -> bool {
    packet.get("schemaId").and_then(Value::as_str) == Some(GRAPH_TURBO_REQUEST_SCHEMA_ID)
        || packet.get("packetKind").and_then(Value::as_str) == Some("graph-turbo-request")
}

pub(super) fn render_graph_turbo_packet(packet_bytes: &[u8]) -> Result<Option<Vec<u8>>, String> {
    let cwd = std::env::current_dir()
        .map_err(|error| format!("failed to resolve current directory: {error}"))?;
    let output = match run_provider_process(ProviderProcessSpec {
        program: graph_turbo_program(),
        args: vec![
            "rank".to_string(),
            "-".to_string(),
            "--format".to_string(),
            "compact".to_string(),
        ],
        cwd,
        env: BTreeMap::new(),
        stdin: StdinMode::bytes(packet_bytes.to_vec()),
        stdout: OutputMode::Capture,
        stderr: OutputMode::Capture,
        limits: ProviderProcessLimits::default(),
    }) {
        Ok(output) => output,
        Err(error) => {
            eprintln!(
                "[asp-graph] fallback=rust-render reason=asp-graph-turbo-unavailable detail={error}"
            );
            return Ok(None);
        }
    };
    if !output.stderr.is_empty() {
        io::stderr()
            .write_all(output.stderr.as_ref())
            .map_err(|error| format!("failed to write asp-graph-turbo stderr: {error}"))?;
    }
    if !output.status.success() {
        eprintln!(
            "[asp-graph] fallback=rust-render reason=asp-graph-turbo-exit status={}",
            output.status.code().unwrap_or(1)
        );
        return Ok(None);
    }
    Ok(Some(output.stdout.to_vec()))
}

fn graph_turbo_program() -> String {
    match std::env::current_exe()
        .ok()
        .and_then(|current_exe| sibling_graph_turbo_program(&current_exe))
    {
        Some(program) => program.display().to_string(),
        None => "asp-graph-turbo".to_string(),
    }
}

fn sibling_graph_turbo_program(current_exe: &Path) -> Option<PathBuf> {
    let candidate = current_exe
        .parent()?
        .join(format!("asp-graph-turbo{}", std::env::consts::EXE_SUFFIX));
    candidate.is_file().then_some(candidate)
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|window| window[0] == flag)
        .map(|window| window[1].clone())
}

fn usage() -> String {
    "usage: asp graph render --packet <path-or-> [--view seeds] [--seeds N]".to_string()
}
