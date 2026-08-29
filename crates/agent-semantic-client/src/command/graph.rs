//! `asp graph` command adapter.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use serde_json::Value;

const GRAPH_TURBO_REQUEST_SCHEMA_ID: &str = "agent.semantic-protocols.semantic-graph-turbo-request";

pub(crate) async fn run_graph_command(args: &[String]) -> Result<(), String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(usage());
    };
    match command {
        "render" => run_graph_render_command(&args[1..]).await,
        "artifact" => run_graph_artifact_command(&args[1..]).await,
        "help" | "--help" | "-h" => Err(usage()),
        _ => Err(usage()),
    }
}

async fn run_graph_artifact_command(args: &[String]) -> Result<(), String> {
    if args.first().map(String::as_str) != Some("publish") {
        return Err(usage());
    }
    let executable = flag_value(&args[1..], "--executable")
        .ok_or_else(|| "graph artifact publish requires --executable <path>".to_owned())?;
    let command_arguments = flag_values(&args[1..], "--argument");
    if command_arguments.is_empty() {
        return Err(
            "graph artifact publish requires at least one --argument and exactly one ${socketPath} placeholder"
                .to_owned(),
        );
    }
    let expected_generation = flag_value(&args[1..], "--expected-generation")
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|error| format!("invalid --expected-generation: {error}"))
        })
        .transpose()?;
    let state_home = agent_semantic_runtime::state_core::resolve_state_home()?;
    let receipt =
        agent_semantic_runtime_server::asp_python_graphs_artifact::publish_asp_python_graphs_artifact(
            &state_home,
            Path::new(&executable),
            command_arguments,
            expected_generation,
        )
        .await?;
    println!(
        "{}",
        serde_json::to_string(&receipt)
            .map_err(|error| format!("encode graph artifact publication receipt: {error}"))?
    );
    Ok(())
}

async fn run_graph_render_command(args: &[String]) -> Result<(), String> {
    let request = GraphRenderRequest::parse(args)?;
    if request.view != "seeds" {
        return Err("graph render currently supports only --view seeds".to_string());
    }
    let packet_bytes = read_packet_bytes(&request.packet_path)?;
    let packet = parse_packet(&packet_bytes)?;
    if is_graph_turbo_request(&packet) {
        let project_root = std::env::current_dir()
            .map_err(|error| format!("failed to resolve graph project root: {error}"))?;
        let ranked_packet = rank_graph_turbo_packet(&project_root, &packet_bytes).await?;
        let mut projection_request =
            agent_semantic_search_projection::SearchProjectionRequestV1::new(
                "ranked-frontier",
                agent_semantic_search_projection::SearchProjectionDensityV1::Terse,
            );
        projection_request.max_rows = request.seed_limit;
        let output = agent_semantic_search_projection::SearchProjectionRenderer::render(
            &agent_semantic_search_projection::RankedFrontierSearchProjectionRenderer,
            &ranked_packet,
            &projection_request,
        )
        .map_err(|error| error.to_string())?;
        print!("{}", output.content());
        return Ok(());
    }
    let packet = agent_semantic_search_projection::SemanticSearchPacketV1::from_value(packet)
        .map_err(|error| error.to_string())?;
    let mut projection_request = agent_semantic_search_projection::SearchProjectionRequestV1::new(
        "topology",
        agent_semantic_search_projection::SearchProjectionDensityV1::Terse,
    );
    projection_request.max_rows = request.seed_limit;
    let output = agent_semantic_search_projection::SearchProjectionRenderer::render(
        &agent_semantic_search_projection::TopologySearchProjectionRenderer,
        &packet,
        &projection_request,
    )
    .map_err(|error| error.to_string())?;
    print!("{}", output.content());
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

pub(super) async fn rank_graph_turbo_packet(
    project_root: &Path,
    packet_bytes: &[u8],
) -> Result<agent_semantic_search_projection::GraphTurboResultPacketV1, String> {
    let state_home = agent_semantic_runtime::state_core::resolve_state_home()?;
    let client = crate::AspClient::new(state_home, project_root);
    let request = resident_graph_turbo_message(packet_bytes)?;
    client.graphs_evaluate(request.into_value()).await
}

fn resident_graph_turbo_message(
    packet_bytes: &[u8],
) -> Result<agent_semantic_search_projection::GraphTurboEvaluationRequest, String> {
    let message = serde_json::from_slice::<serde_json::Value>(packet_bytes)
        .map_err(|error| format!("failed to decode typed Graph Turbo request: {error}"))?;
    agent_semantic_search_projection::GraphTurboEvaluationRequest::from_value(message)
        .map_err(|error| format!("invalid Graph Turbo rank intent: {error}"))
}

#[cfg(test)]
#[path = "../../tests/unit/command/graph_runtime_resident.rs"]
mod graph_runtime_resident_tests;

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|window| window[0] == flag)
        .map(|window| window[1].clone())
}

fn flag_values(args: &[String], flag: &str) -> Vec<String> {
    args.windows(2)
        .filter(|window| window[0] == flag)
        .map(|window| window[1].clone())
        .collect()
}

fn usage() -> String {
    "usage: asp graph render --packet <path-or-> [--view seeds] [--seeds N]\n       asp graph artifact publish --executable <absolute-path> --argument <arg>... [--expected-generation N]".to_string()
}
