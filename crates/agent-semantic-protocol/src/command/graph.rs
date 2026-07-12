//! `asp graph` command adapter.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use agent_semantic_provider_transport::{
    OutputMode, ProviderProcessLimits, ProviderProcessSpec, StdinMode, run_provider_process,
};
use serde_json::Value;

use super::search_pipe_evidence_projection::rank_frontier_has_only_owner_or_topology_nodes;
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
    if is_graph_turbo_request(&packet) {
        if let Some(output) = render_graph_turbo_packet(&packet_bytes)? {
            if request.frontier_receipt_out.is_some() {
                write_graph_turbo_receipt(
                    &packet_bytes,
                    &GraphTurboReceiptCapture {
                        out_path: request
                            .frontier_receipt_out
                            .as_deref()
                            .ok_or_else(|| "missing frontier receipt output path".to_string())?,
                        receipt_id: request
                            .receipt_id
                            .as_deref()
                            .ok_or_else(|| "missing receipt id".to_string())?,
                        task_fingerprint: request
                            .task_fingerprint
                            .as_deref()
                            .ok_or_else(|| "missing task fingerprint".to_string())?,
                        command_fingerprint: request
                            .command_fingerprint
                            .as_deref()
                            .ok_or_else(|| "missing command fingerprint".to_string())?,
                        capture_source: "asp graph render",
                        extra_args: &[],
                    },
                )?;
            }
            io::stdout()
                .write_all(output.as_ref())
                .map_err(|error| format!("failed to write asp-graph-turbo stdout: {error}"))?;
            return Ok(());
        }
        if request.frontier_receipt_out.is_some() {
            return Err(
                "--frontier-receipt-out requires the asp-graph-turbo receipt backend".to_string(),
            );
        }
        let output = render_graph_turbo_value_rust_compact(&packet)?;
        io::stdout().write_all(output.as_ref()).map_err(|error| {
            format!("failed to write rust graph-turbo fallback stdout: {error}")
        })?;
        return Ok(());
    }
    if request.frontier_receipt_out.is_some() {
        return Err("--frontier-receipt-out requires a graph-turbo request packet".to_string());
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
    frontier_receipt_out: Option<PathBuf>,
    receipt_id: Option<String>,
    task_fingerprint: Option<String>,
    command_fingerprint: Option<String>,
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
        let frontier_receipt_out = flag_value(args, "--frontier-receipt-out").map(PathBuf::from);
        let receipt_id = flag_value(args, "--receipt-id");
        let task_fingerprint = flag_value(args, "--task-fingerprint");
        let command_fingerprint = flag_value(args, "--command-fingerprint");
        if frontier_receipt_out.is_some()
            && (receipt_id.is_none() || task_fingerprint.is_none() || command_fingerprint.is_none())
        {
            return Err(
                "--frontier-receipt-out requires --receipt-id, --task-fingerprint, and --command-fingerprint"
                    .to_string(),
            );
        }
        Ok(Self {
            packet_path: PathBuf::from(packet_path),
            view,
            seed_limit,
            frontier_receipt_out,
            receipt_id,
            task_fingerprint,
            command_fingerprint,
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

pub(super) fn render_graph_turbo_value_rust_compact(packet: &Value) -> Result<Vec<u8>, String> {
    if let Some(route) = packet.get("route")
        && let Some(output) = render_graph_route_compact(packet, route)
    {
        return Ok(output.into_bytes());
    }
    let nodes = packet
        .pointer("/graph/nodes")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let edges = packet
        .pointer("/graph/edges")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let alias_map = compact_node_aliases(nodes);
    let alias_index = compact_alias_index(nodes, &alias_map);
    let mut output = String::new();
    output.push_str(&format!(
        "[graph-frontier] profile={} alg={} seed={} budget={}\n",
        compact_json_str(packet.get("profile")).unwrap_or("owner-query"),
        compact_json_str(packet.get("algorithm")).unwrap_or("typed-ppr-diverse"),
        compact_seed_aliases(packet.get("seedIds"), &alias_map),
        packet.get("budget").and_then(Value::as_u64).unwrap_or(10),
    ));
    let ranked = compact_ranked_aliases(nodes, edges, &alias_map, &alias_index);
    let visible_aliases = ranked.iter().cloned().collect::<HashSet<_>>();
    let visible_node_kinds = compact_ranked_node_kinds(&ranked, nodes, &alias_index);
    if !rank_frontier_has_only_owner_or_topology_nodes(&visible_node_kinds) {
        output.push_str("rank=");
        output.push_str(&ranked.join(","));
        output.push('\n');
        output.push_str("frontier=");
        output.push_str(
            &ranked
                .iter()
                .filter_map(|alias| {
                    let node = compact_node_for_alias(alias, nodes, &alias_index)?;
                    Some(format!(
                        "{alias}.{}",
                        compact_json_str(node.get("action")).unwrap_or("evidence")
                    ))
                })
                .collect::<Vec<_>>()
                .join(","),
        );
        output.push('\n');
    }
    for alias in &ranked {
        if let Some(node) = compact_node_for_alias(alias, nodes, &alias_index) {
            output.push_str(&compact_node_line(alias, node));
            output.push('\n');
        }
    }
    for line in compact_edge_lines(edges, &alias_map, &visible_aliases) {
        output.push_str(&line);
        output.push('\n');
    }
    Ok(output.into_bytes())
}

fn render_graph_route_compact(packet: &Value, route: &Value) -> Option<String> {
    let profile = compact_json_str(packet.get("profile")).unwrap_or("owner-query");
    let algorithm = compact_json_str(packet.get("algorithm")).unwrap_or("typed-ppr-diverse");
    let relation = compact_json_str(route.get("relation")).unwrap_or("cohesive");
    let route_kind = compact_json_str(route.get("routeKind")).unwrap_or("owner");
    let covered = route
        .get("coveredQueryCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let query_count = route.get("queryCount").and_then(Value::as_u64).unwrap_or(0);
    let budget = packet.get("budget").and_then(Value::as_u64).unwrap_or(10);
    let owner_path = route
        .pointer("/owner/path")
        .and_then(Value::as_str)
        .filter(|path| !path.is_empty())?;
    let score = route
        .pointer("/owner/score/total")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let hits = route
        .pointer("/owner/localHits")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let symbols = route
        .pointer("/owner/symbols")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(compact_token)
        .collect::<Vec<_>>();
    let language_id = route
        .pointer("/nextAction/languageId")
        .and_then(Value::as_str)
        .unwrap_or("rust");
    let query = route
        .pointer("/nextAction/query")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let avoid = route
        .get("avoid")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    let mut output = String::new();
    output.push_str(&format!(
        "[graph-route] profile={profile} alg={algorithm} relation={relation} route={route_kind} covered={covered}/{query_count} budget={budget}\n"
    ));
    output.push_str(&format!(
        "owner=path({}) score={score} hits={hits} symbols={}\n",
        compact_token(owner_path),
        if symbols.is_empty() {
            "-".to_string()
        } else {
            symbols.join(",")
        }
    ));
    output.push_str(&format!(
        "next=asp {language_id} search owner {} items --query {} --workspace <root> --view seeds\n",
        shell_quote(owner_path),
        shell_quote(query)
    ));
    if !avoid.is_empty() {
        output.push_str("avoid=");
        output.push_str(&avoid.join(","));
        output.push('\n');
    }
    Some(output)
}

fn compact_ranked_node_kinds(
    ranked: &[String],
    nodes: &[Value],
    alias_index: &HashMap<String, usize>,
) -> HashMap<String, String> {
    ranked
        .iter()
        .filter_map(|alias| {
            let kind = compact_node_for_alias(alias, nodes, alias_index)
                .and_then(|node| compact_json_str(node.get("kind")))?;
            Some((alias.clone(), kind.to_string()))
        })
        .collect()
}

fn compact_node_aliases(nodes: &[Value]) -> HashMap<String, String> {
    let mut aliases = HashMap::new();
    let mut counts: HashMap<&'static str, usize> = HashMap::new();
    for node in nodes {
        let Some(id) = compact_json_str(node.get("id")) else {
            continue;
        };
        let base = compact_alias_base(compact_json_str(node.get("kind")).unwrap_or("node"));
        let count = counts.entry(base).or_insert(0);
        *count += 1;
        let alias = if *count == 1 {
            base.to_string()
        } else {
            format!("{base}{count}")
        };
        aliases.insert(id.to_string(), alias);
    }
    aliases
}

fn compact_alias_index(
    nodes: &[Value],
    aliases: &HashMap<String, String>,
) -> HashMap<String, usize> {
    nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| {
            let alias = compact_json_str(node.get("id")).and_then(|id| aliases.get(id))?;
            Some((alias.clone(), index))
        })
        .collect()
}

fn compact_alias_base(kind: &str) -> &'static str {
    match kind {
        "query" => "Q",
        "owner" => "O",
        "item" => "I",
        "hot" => "H",
        "test" => "T",
        "dependency" => "D",
        "dependency-version" => "V",
        "workspace" => "W",
        "provider-root" => "P",
        "submodule" => "S",
        _ => "N",
    }
}

fn compact_seed_aliases(seed_ids: Option<&Value>, aliases: &HashMap<String, String>) -> String {
    let seeds = seed_ids
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter_map(|id| aliases.get(id))
        .cloned()
        .collect::<Vec<_>>();
    if seeds.is_empty() {
        "-".to_string()
    } else {
        seeds.join(",")
    }
}

fn compact_ranked_aliases(
    nodes: &[Value],
    edges: &[Value],
    aliases: &HashMap<String, String>,
    alias_index: &HashMap<String, usize>,
) -> Vec<String> {
    let mut ranked = nodes
        .iter()
        .filter(|node| compact_json_str(node.get("kind")) != Some("query"))
        .filter_map(|node| compact_json_str(node.get("id")).and_then(|id| aliases.get(id)))
        .take(10)
        .cloned()
        .collect::<Vec<_>>();
    append_topology_support_aliases(&mut ranked, nodes, edges, aliases, alias_index, 6);
    ranked
}

fn append_topology_support_aliases(
    ranked: &mut Vec<String>,
    nodes: &[Value],
    edges: &[Value],
    aliases: &HashMap<String, String>,
    alias_index: &HashMap<String, usize>,
    limit: usize,
) {
    let mut visible = ranked.iter().cloned().collect::<HashSet<_>>();
    let mut added = 0;
    while added < limit {
        let mut changed = false;
        for edge in edges {
            if added >= limit {
                break;
            }
            let Some(candidate) =
                compact_topology_support_alias(edge, &visible, nodes, aliases, alias_index)
            else {
                continue;
            };
            if visible.contains(&candidate) {
                continue;
            }
            ranked.push(candidate.clone());
            visible.insert(candidate);
            added += 1;
            changed = true;
        }
        if !changed {
            break;
        }
    }
}

fn compact_topology_support_alias(
    edge: &Value,
    visible: &HashSet<String>,
    nodes: &[Value],
    aliases: &HashMap<String, String>,
    alias_index: &HashMap<String, usize>,
) -> Option<String> {
    let relation = compact_json_str(edge.get("relation")).unwrap_or("rel");
    let source = compact_json_str(edge.get("source")).and_then(|id| aliases.get(id))?;
    let target = compact_json_str(edge.get("target")).and_then(|id| aliases.get(id))?;
    let source_kind = compact_alias_kind(source, nodes, alias_index)?;
    let target_kind = compact_alias_kind(target, nodes, alias_index)?;
    match (relation, source_kind, target_kind) {
        ("contains", "submodule", "owner") if visible.contains(target) => Some(source.clone()),
        ("has_submodule", "workspace", "submodule") if visible.contains(target) => {
            Some(source.clone())
        }
        ("has_provider_root", "workspace", "provider-root") if visible.contains(source) => {
            Some(target.clone())
        }
        _ => None,
    }
}

fn compact_alias_kind<'a>(
    alias: &str,
    nodes: &'a [Value],
    alias_index: &HashMap<String, usize>,
) -> Option<&'a str> {
    compact_node_for_alias(alias, nodes, alias_index)
        .and_then(|node| compact_json_str(node.get("kind")))
}

fn compact_node_for_alias<'a>(
    alias: &str,
    nodes: &'a [Value],
    alias_index: &HashMap<String, usize>,
) -> Option<&'a Value> {
    alias_index.get(alias).and_then(|index| nodes.get(*index))
}

fn compact_node_line(alias: &str, node: &Value) -> String {
    let kind = compact_json_str(node.get("kind")).unwrap_or("node");
    let role = compact_json_str(node.get("role")).unwrap_or("value");
    let value = compact_json_str(node.get("value"))
        .or_else(|| compact_json_str(node.get("symbol")))
        .or_else(|| compact_json_str(node.get("path")))
        .unwrap_or("-");
    let action = compact_json_str(node.get("action")).unwrap_or("evidence");
    let locator = compact_json_str(node.get("locator"))
        .or_else(|| compact_json_str(node.get("path")))
        .map(|locator| format!("@{locator}"))
        .unwrap_or_default();
    format!(
        "{alias}={kind}:{role}({}){locator}!{action}",
        compact_token(value)
    )
}

fn compact_edge_lines(
    edges: &[Value],
    aliases: &HashMap<String, String>,
    visible_aliases: &HashSet<String>,
) -> Vec<String> {
    let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for edge in edges {
        let Some(source) = compact_json_str(edge.get("source")).and_then(|id| aliases.get(id))
        else {
            continue;
        };
        let Some(target) = compact_json_str(edge.get("target")).and_then(|id| aliases.get(id))
        else {
            continue;
        };
        if !visible_aliases.contains(source) || !visible_aliases.contains(target) {
            continue;
        }
        let relation = compact_json_str(edge.get("relation")).unwrap_or("rel");
        grouped
            .entry(source.clone())
            .or_default()
            .push(format!("{target}:{relation}"));
    }
    grouped
        .into_iter()
        .map(|(source, targets)| format!("{source}>{{{}}}", targets.join(",")))
        .collect()
}

fn compact_json_str(value: Option<&Value>) -> Option<&str> {
    value.and_then(Value::as_str)
}

fn compact_token(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric()
                || matches!(character, '_' | '-' | '.' | '/' | ':' | '@')
            {
                character
            } else {
                '-'
            }
        })
        .collect()
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(super) struct GraphTurboReceiptCapture<'a> {
    pub(super) out_path: &'a Path,
    pub(super) receipt_id: &'a str,
    pub(super) task_fingerprint: &'a str,
    pub(super) command_fingerprint: &'a str,
    pub(super) capture_source: &'a str,
    pub(super) extra_args: &'a [String],
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct GraphTurboReceiptRequest {
    pub(super) out_path: PathBuf,
    pub(super) extra_args: Vec<String>,
}

impl GraphTurboReceiptRequest {
    pub(super) fn new(out_path: PathBuf, extra_args: Vec<String>) -> Self {
        Self {
            out_path,
            extra_args,
        }
    }

    pub(super) fn has_extra_args(&self) -> bool {
        !self.extra_args.is_empty()
    }
}

pub(super) fn write_graph_turbo_receipt(
    packet_bytes: &[u8],
    capture: &GraphTurboReceiptCapture<'_>,
) -> Result<(), String> {
    let cwd = std::env::current_dir()
        .map_err(|error| format!("failed to resolve current directory: {error}"))?;
    let mut args = vec![
        "receipt".to_string(),
        "-".to_string(),
        "--receipt-id".to_string(),
        capture.receipt_id.to_string(),
        "--task-fingerprint".to_string(),
        capture.task_fingerprint.to_string(),
        "--command-fingerprint".to_string(),
        capture.command_fingerprint.to_string(),
    ];
    args.extend(capture.extra_args.iter().cloned());
    args.extend([
        "--field".to_string(),
        format!("captureSource={}", capture.capture_source),
    ]);
    let output = run_provider_process(ProviderProcessSpec {
        program: graph_turbo_program(),
        args,
        cwd,
        env: BTreeMap::new(),
        stdin: StdinMode::bytes(packet_bytes.to_vec()),
        stdout: OutputMode::Capture,
        stderr: OutputMode::Capture,
        limits: ProviderProcessLimits::default(),
    })
    .map_err(|error| format!("failed to run asp-graph-turbo receipt: {error}"))?;
    if !output.stderr.is_empty() {
        io::stderr()
            .write_all(output.stderr.as_ref())
            .map_err(|error| format!("failed to write asp-graph-turbo receipt stderr: {error}"))?;
    }
    if !output.status.success() {
        return Err(format!(
            "asp-graph-turbo receipt exited with status {}",
            output.status.code().unwrap_or(1)
        ));
    }
    if let Some(parent) = capture.out_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(capture.out_path, output.stdout)
        .map_err(|error| format!("failed to write {}: {error}", capture.out_path.display()))
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
    "usage: asp graph render --packet <path-or-> [--view seeds] [--seeds N] [--frontier-receipt-out PATH --receipt-id ID --task-fingerprint VALUE --command-fingerprint VALUE]".to_string()
}
