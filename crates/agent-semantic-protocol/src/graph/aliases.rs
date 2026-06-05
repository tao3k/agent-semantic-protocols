use std::collections::BTreeSet;

use serde_json::Value;

use super::actions::{GraphAction, graph_action_spec, graph_actions};
use super::api::SEARCH_ROOT_ID;
use super::packet::{is_owner_item_query_packet, packet_view};

pub(super) struct GraphAlias {
    pub(super) id: String,
    pub(super) node_type: &'static str,
    target_role: &'static str,
    target: String,
    locator: Option<String>,
    pub(super) action: String,
}

impl GraphAlias {
    pub(super) fn render(&self) -> String {
        let locator = self
            .locator
            .as_deref()
            .map(|locator| format!("@{locator}"))
            .unwrap_or_default();
        format!(
            "{}={}:{}({}){}!{}",
            self.id,
            self.node_type,
            graph_alias_target_role(self.target_role, &self.target),
            self.target,
            locator,
            self.action
        )
    }
}

pub(super) fn graph_aliases(packet: &Value, limit: usize) -> Vec<GraphAlias> {
    let mut aliases = Vec::new();
    let mut seen = BTreeSet::new();
    let mut counters = Vec::<(&'static str, usize)>::new();
    let owner_item_query = is_owner_item_query_packet(packet, packet_view(packet));
    let mut actions = graph_actions(packet);
    if owner_item_query {
        actions.sort_by_key(graph_action_owner_item_order)
    }
    for action in actions {
        let action_kind = if owner_item_query && action.kind == "symbol" {
            "item-symbol"
        } else {
            action.kind.as_str()
        };
        let Some(spec) = graph_action_spec(action_kind) else {
            continue;
        };
        let target = action.target.trim();
        if target.is_empty() {
            continue;
        }
        let action_name = action
            .action
            .clone()
            .unwrap_or_else(|| spec.action.to_string());
        let dedupe_key = format!("{}:{}:{}", spec.node_type, target, action_name);
        if !seen.insert(dedupe_key) {
            continue;
        }
        let id = next_alias_id(&mut counters, spec.alias_prefix);
        aliases.push(GraphAlias {
            id,
            node_type: spec.node_type,
            target_role: spec.target_role,
            target: target.to_string(),
            locator: action.locator.clone(),
            action: action_name,
        });
        if aliases.len() >= limit {
            break;
        }
    }
    aliases
}

pub(super) fn graph_edge_lines(aliases: &[GraphAlias], owner_item_query: bool) -> Vec<String> {
    if !owner_item_query {
        let edges = aliases
            .iter()
            .map(|alias| format!("{}:{}", alias.id, graph_relation(alias.node_type)))
            .collect::<Vec<_>>()
            .join(",");
        return vec![format!("{SEARCH_ROOT_ID}>{{{edges}}}")];
    }

    let owner = aliases.iter().find(|alias| alias.node_type == "owner");
    let queries = aliases
        .iter()
        .filter(|alias| alias.node_type == "query")
        .collect::<Vec<_>>();
    let items = aliases
        .iter()
        .filter(|alias| alias.node_type == "item")
        .collect::<Vec<_>>();
    let hot = aliases
        .iter()
        .filter(|alias| alias.node_type == "hot")
        .collect::<Vec<_>>();
    let mut root_edges = Vec::new();
    if let Some(owner) = owner {
        root_edges.push(format!("{}:selects", owner.id));
    }
    if queries.is_empty() {
        root_edges.extend(items.iter().map(|alias| format!("{}:matches", alias.id)));
    } else {
        root_edges.extend(queries.iter().map(|alias| format!("{}:matches", alias.id)));
    }
    let mut lines = vec![format!("{SEARCH_ROOT_ID}>{{{}}}", root_edges.join(","))];
    if let Some(owner) = owner {
        let contains_edges = items
            .iter()
            .chain(hot.iter())
            .map(|alias| format!("{}:contains", alias.id))
            .collect::<Vec<_>>()
            .join(",");
        if !contains_edges.is_empty() {
            lines.push(format!("{}>{{{contains_edges}}}", owner.id));
        }
    }
    for query in queries {
        let query_edges = items
            .iter()
            .map(|alias| format!("{}:matches", alias.id))
            .chain(hot.iter().map(|alias| format!("{}:revise", alias.id)))
            .collect::<Vec<_>>()
            .join(",");
        if !query_edges.is_empty() {
            lines.push(format!("{}>{{{query_edges}}}", query.id));
        }
    }
    lines
}

pub(super) fn graph_rank(aliases: &[GraphAlias], owner_item_query: bool) -> String {
    graph_rank_aliases(aliases, owner_item_query)
        .into_iter()
        .map(|alias| alias.id.as_str())
        .collect::<Vec<_>>()
        .join(",")
}

pub(super) fn graph_frontier(aliases: &[GraphAlias], owner_item_query: bool) -> String {
    graph_rank_aliases(aliases, owner_item_query)
        .into_iter()
        .filter(|alias| !(owner_item_query && alias.node_type == "owner"))
        .map(|alias| format!("{}.{}", alias.id, alias.action))
        .collect::<Vec<_>>()
        .join(",")
}

pub(super) fn graph_legend_line(aliases: &[GraphAlias]) -> String {
    let mut entries = vec![format!("{SEARCH_ROOT_ID}=search")];
    let mut seen = BTreeSet::new();
    for alias in aliases {
        let compact_id = compact_alias_id(&alias.id);
        if seen.contains(&(compact_id.to_string(), alias.node_type)) {
            continue;
        }
        let id = if seen
            .iter()
            .any(|(existing, node_type)| existing == compact_id && *node_type != alias.node_type)
        {
            alias.id.as_str()
        } else {
            compact_id
        };
        if seen.insert((id.to_string(), alias.node_type)) {
            entries.push(format!("{id}={}", alias.node_type));
        }
    }
    format!("aliases: graph:{{{}}}", entries.join(","))
}

pub(super) fn is_owner_item_query(packet: &Value, mode: &str, aliases: &[GraphAlias]) -> bool {
    is_owner_item_query_packet(packet, mode)
        && aliases.iter().any(|alias| alias.node_type == "owner")
        && aliases.iter().any(|alias| alias.node_type == "item")
}

fn graph_alias_target_role(target_role: &'static str, target: &str) -> &'static str {
    if target == "self" {
        return "self";
    }
    target_role
}

fn graph_action_owner_item_order(action: &GraphAction) -> u8 {
    match action.kind.as_str() {
        "owner" => 0,
        "query" => 1,
        "item-symbol" => 2,
        "symbol" => 3,
        "hot" => 3,
        _ => 5,
    }
}

fn graph_rank_aliases(aliases: &[GraphAlias], owner_item_query: bool) -> Vec<&GraphAlias> {
    if !owner_item_query {
        return aliases.iter().collect();
    }
    aliases
        .iter()
        .filter(|alias| alias.node_type == "hot")
        .chain(aliases.iter().filter(|alias| alias.node_type == "item"))
        .chain(aliases.iter().filter(|alias| alias.node_type == "owner"))
        .collect()
}

fn compact_alias_id(id: &str) -> &str {
    id.trim_end_matches(|character: char| character.is_ascii_digit())
}

fn graph_relation(node_type: &str) -> &'static str {
    match node_type {
        "query" => "matches",
        "test" => "covers",
        "dependency" => "uses",
        "import" => "imports",
        "symbol" | "item" => "contains",
        "doc" => "explains",
        "finding" => "flags",
        "feature" | "cfg" => "gates",
        _ => "selects",
    }
}

fn next_alias_id(counters: &mut Vec<(&'static str, usize)>, prefix: &'static str) -> String {
    if let Some((_, count)) = counters
        .iter_mut()
        .find(|(candidate, _)| *candidate == prefix)
    {
        *count += 1;
        return format!("{prefix}{count}");
    }
    counters.push((prefix, 1));
    prefix.to_string()
}
