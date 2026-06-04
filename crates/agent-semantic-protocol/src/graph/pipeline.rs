use serde_json::Value;

use super::actions::query_term_count;
use super::aliases::{
    self, graph_aliases, graph_edge_lines, graph_frontier, graph_legend_line, graph_rank,
    is_owner_item_query,
};
use super::api::{COMPACT_GRAPH_MICRO_LEGEND, GraphRenderOptions};
use super::header::graph_header;
use super::packet::{
    fallback_algorithm, graph_root, is_owner_item_query_packet, packet_string, packet_view,
};
use super::profiles::graph_profiles_line;

const DEFAULT_SEED_LIMIT: usize = 8;

pub(super) fn render_search_graph_packet(packet: &Value, options: GraphRenderOptions) -> String {
    fn graph_avoid_line(packet: &Value) -> Option<String> {
        let actions = packet.get("avoidNextActions")?.as_array()?;
        let kinds = actions
            .iter()
            .filter_map(|action| action.get("kind").and_then(Value::as_str))
            .filter(|kind| !kind.trim().is_empty())
            .collect::<Vec<_>>();
        (!kinds.is_empty()).then(|| format!("avoid={}", kinds.join(",")))
    }

    let mode = packet_view(packet);
    let root = graph_root(packet, mode);
    let algorithm = packet_string(packet, &["searchSynthesis", "algorithm"])
        .unwrap_or_else(|| fallback_algorithm(mode));
    let aliases = graph_aliases(packet, graph_seed_limit(options.seed_limit));
    let owner_item_query = is_owner_item_query(packet, mode, &aliases);
    let mut lines = vec![graph_header(
        packet,
        mode,
        &root,
        &algorithm,
        is_owner_item_query_packet(packet, mode),
        query_term_count(packet),
    )];
    lines.push(COMPACT_GRAPH_MICRO_LEGEND.to_string());
    lines.push(graph_legend_line(&aliases));
    let alias_definitions = aliases
        .iter()
        .map(aliases::GraphAlias::render)
        .collect::<Vec<_>>()
        .join(";");
    if !alias_definitions.is_empty() {
        lines.push(alias_definitions);
    }
    lines.extend(graph_edge_lines(&aliases, owner_item_query));
    let rank = graph_rank(&aliases, owner_item_query);
    let frontier = graph_frontier(&aliases, owner_item_query);
    lines.push(format!("rank={rank} frontier={frontier}"));
    if let Some(profiles) = graph_profiles_line(packet, &aliases) {
        lines.push(profiles);
    }
    if let Some(avoid) = graph_avoid_line(packet) {
        lines.push(avoid);
    }
    if owner_item_query {
        lines.push("omit=code,comments,blank-lines,nonmatching-items".to_string());
        lines.push("avoid=repeat-owner,raw-read,full-json".to_string());
    }
    lines.push(String::new());
    lines.join(
        "
",
    )
}

fn graph_seed_limit(seed_limit: Option<usize>) -> usize {
    seed_limit
        .filter(|limit| *limit > 0)
        .unwrap_or(DEFAULT_SEED_LIMIT)
}
