use serde_json::Value;

use super::actions::query_term_count;
use super::aliases::{
    self, graph_aliases, graph_edge_lines, graph_frontier, graph_legend_line, graph_rank,
    graph_syntax_lines, is_owner_item_query,
};
use super::api::{COMPACT_GRAPH_MICRO_LEGEND, GraphRenderOptions};
use super::header::graph_header;
use super::packet::{
    fallback_algorithm, graph_root, is_owner_item_query_packet, packet_string, packet_view,
};
use super::profiles::graph_profiles_line;

const DEFAULT_SEED_LIMIT: usize = 12;
const PRIME_GRAPH_ALGORITHM: &str = "budgeted-prime-frontier-v1";
const PRIME_GRAPH_LEGEND: &str =
    "legend: ID=kind:role(value)!next; entries profile(selectors=>returns); frontier ID.next";
const PRIME_OWNER_ONLY_LEGEND: &str =
    "legend: ID=kind:role(value)!next; entries profile(selectors=>returns)";

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
    let prime_mode = is_prime_graph_mode(mode);
    let owner_item_query_packet = is_owner_item_query_packet(packet, mode);
    let mut algorithm = packet_string(packet, &["searchSynthesis", "algorithm"])
        .unwrap_or_else(|| fallback_algorithm(mode));
    if prime_mode {
        algorithm = PRIME_GRAPH_ALGORITHM.to_string();
    } else if owner_item_query_packet {
        algorithm = "item-frontier".to_string();
    }
    let seed_limit = graph_seed_limit(options.seed_limit);
    let aliases = graph_aliases(packet, seed_limit);
    let owner_item_query = is_owner_item_query(packet, mode, &aliases);
    let prime_owner_only_frontier =
        prime_mode && aliases.iter().all(|alias| alias.node_type == "owner");
    let mut header = graph_header(
        packet,
        mode,
        &root,
        &algorithm,
        owner_item_query_packet,
        query_term_count(packet),
    );
    if prime_mode {
        header.push_str(&format!(" budget=handles:{seed_limit}"));
    }
    let mut lines = vec![header];
    if prime_mode {
        lines.push(prime_decision_line(packet));
    }
    lines.push(if prime_owner_only_frontier {
        PRIME_OWNER_ONLY_LEGEND.to_string()
    } else if prime_mode {
        PRIME_GRAPH_LEGEND.to_string()
    } else {
        COMPACT_GRAPH_MICRO_LEGEND.to_string()
    });
    lines.push(if prime_owner_only_frontier {
        "aliases: owner:{O=owner}".to_string()
    } else {
        graph_legend_line(&aliases)
    });
    let alias_definitions = aliases
        .iter()
        .map(aliases::GraphAlias::render)
        .collect::<Vec<_>>()
        .join(";");
    if !alias_definitions.is_empty() {
        lines.push(alias_definitions);
    }
    lines.extend(graph_syntax_lines(&aliases));
    if !prime_owner_only_frontier {
        lines.extend(graph_edge_lines(&aliases, owner_item_query));
    }
    if !prime_owner_only_frontier {
        let rank = graph_rank(&aliases, owner_item_query);
        let frontier = graph_frontier(&aliases, owner_item_query);
        lines.push(format!("rank={rank} frontier={frontier}"));
    }
    if owner_item_query && let Some(revisions) = owner_item_query_revisions(packet) {
        lines.push(format!("revise={}", revisions.join(",")));
    }
    if prime_mode {
        if let Some(entries) = prime_graph_entries_line(&aliases) {
            lines.push(entries);
        }
        lines.push("omit=items,blocks,code,full-test-list".to_string());
        lines.push("avoid=raw-read,full-json,broad-lexical".to_string());
    } else if let Some(profiles) = graph_profiles_line(packet, &aliases) {
        lines.push(profiles);
    }
    if !prime_mode && let Some(avoid) = graph_avoid_line(packet) {
        lines.push(avoid);
    }
    if owner_item_query {
        lines.push("omit=code,projection-nodes,large-item-text".to_string());
        lines.push("avoid=inline-code-in-search,raw-read,repeat-owner".to_string());
    }
    lines.push(String::new());
    lines.join(
        "
",
    )
}

fn owner_item_query_revisions(packet: &Value) -> Option<Vec<String>> {
    let revisions = packet
        .get("notes")?
        .as_array()?
        .iter()
        .filter_map(|note| note.get("message").and_then(Value::as_str))
        .flat_map(revision_tokens)
        .collect::<Vec<_>>();
    (!revisions.is_empty()).then_some(revisions)
}

fn revision_tokens(message: &str) -> Vec<String> {
    message
        .split_whitespace()
        .find_map(|token| token.strip_prefix("revise="))
        .map(|revisions| {
            revisions
                .split(',')
                .map(str::trim)
                .filter(|revision| !revision.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn is_prime_graph_mode(mode: &str) -> bool {
    matches!(mode, "prime" | "package")
}

fn prime_graph_entries_line(aliases: &[aliases::GraphAlias]) -> Option<String> {
    fn first_alias_id<'a>(aliases: &'a [aliases::GraphAlias], node_type: &str) -> Option<&'a str> {
        aliases
            .iter()
            .find(|alias| alias.node_type == node_type)
            .map(|alias| alias.id.as_str())
    }

    let owner = first_alias_id(aliases, "owner");
    let query = first_alias_id(aliases, "query");
    let dependency = first_alias_id(aliases, "dependency");
    let mut entries = Vec::new();
    if let (Some(owner), Some(query)) = (owner, query) {
        entries.push(format!(
            "owner-query({owner},{query}=>items+tests+dependency-usage)"
        ));
    }
    if let Some(owner) = owner {
        entries.push(format!(
            "owner-tests({owner}=>covering-tests+test-entrypoints+fixtures)"
        ));
    }
    if let (Some(query), Some(dependency)) = (query, dependency) {
        entries.push(format!(
            "query-deps({query},{dependency}=>owners+imports+usage-tests)"
        ));
    }
    (!entries.is_empty()).then(|| format!("entries={}", entries.join(",")))
}

fn prime_decision_line(packet: &Value) -> String {
    let language_id = packet
        .get("languageId")
        .and_then(Value::as_str)
        .filter(|language_id| !language_id.trim().is_empty())
        .unwrap_or("<language>");
    format!(
        "|decision purpose=decision-primer answer=false code=false capabilities=lexical,pipe,fd-query,rg-query,owner-items,selector-code,treesitter-query ladder=lexical>fd-query|rg-query>owner-items>selector-code>pipe history=asp-artifacts:directReadRisk,repeatedPrime,repeatedPipe,bestPath risk=broad-direct-read,manual-window-scan,repeat-prime next=\"asp {language_id} search lexical <question-term> <related-feature-term> owner tests --workspace . --view seeds\""
    )
}

fn graph_seed_limit(seed_limit: Option<usize>) -> usize {
    seed_limit
        .filter(|limit| *limit > 0)
        .unwrap_or(DEFAULT_SEED_LIMIT)
}
