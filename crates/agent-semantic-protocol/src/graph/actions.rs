use serde_json::Value;

use super::packet::{graph_root, header_field_scalar, packet_query, packet_view};

#[derive(Clone, Copy)]
pub(super) struct GraphActionSpec {
    pub(super) node_type: &'static str,
    pub(super) target_role: &'static str,
    pub(super) alias_prefix: &'static str,
    pub(super) action: &'static str,
}

pub(super) struct GraphAction {
    pub(super) kind: String,
    pub(super) target: String,
    pub(super) locator: Option<String>,
}

macro_rules! graph_action_specs {
    ($($kind:literal => $node_type:literal, $target_role:literal, $alias_prefix:literal, $action:literal);+ $(;)?) => {
        const GRAPH_ACTION_SPECS: &[(&str, GraphActionSpec)] = &[
            $(
                (
                    $kind,
                    GraphActionSpec {
                        node_type: $node_type,
                        target_role: $target_role,
                        alias_prefix: $alias_prefix,
                        action: $action,
                    },
                ),
            )+
        ];
    };
}

graph_action_specs! {
    "owner" => "owner", "path", "O", "owner";
    "prime" => "owner", "path", "O", "owner";
    "package" => "package", "pkg", "P", "owner";
    "test" => "test", "path", "T", "tests";
    "tests" => "test", "path", "T", "tests";
    "symbol" => "symbol", "symbol", "S", "symbol";
    "item-symbol" => "symbol", "symbol", "S", "code";
    "text" => "query", "term", "Q", "fzf";
    "fzf" => "query", "term", "Q", "fzf";
    "query" => "query", "term", "Q", "query";
    "dependency" => "dependency", "pkg", "D", "deps";
    "deps" => "dependency", "pkg", "D", "deps";
    "docs" => "doc", "path", "D", "docs";
    "docs-use" => "doc", "path", "D", "docs";
    "feature" => "feature", "feature", "F", "features";
    "features" => "feature", "feature", "F", "features";
    "cfg" => "cfg", "cfg", "C", "cfg";
    "import" => "import", "path", "I", "import";
    "item" => "item", "symbol", "I", "items";
    "items" => "item", "symbol", "I", "items";
    "ingest" => "owner", "path", "O", "owner";
}

pub(super) fn graph_actions(packet: &Value) -> Vec<GraphAction> {
    let mut actions = Vec::new();
    if let Some(action) = packet_root_action(packet) {
        actions.push(action);
    }
    append_object_actions(&mut actions, packet.get("nextActions"));
    if synthesis_seeds_are_primary(packet) {
        append_object_actions(
            &mut actions,
            packet.get("searchSynthesis").and_then(|s| s.get("seeds")),
        );
        append_object_actions(
            &mut actions,
            packet
                .get("searchSynthesis")
                .and_then(|s| s.get("windowSet")),
        );
    } else {
        append_object_actions(
            &mut actions,
            packet
                .get("searchSynthesis")
                .and_then(|s| s.get("windowSet")),
        );
        append_object_actions(
            &mut actions,
            packet.get("searchSynthesis").and_then(|s| s.get("seeds")),
        );
    }
    append_string_actions(
        &mut actions,
        packet
            .get("searchSynthesis")
            .and_then(|synthesis| synthesis.get("editFrontier")),
        "owner",
    );
    append_string_actions(
        &mut actions,
        packet
            .get("searchSynthesis")
            .and_then(|synthesis| synthesis.get("frontierOwners")),
        "owner",
    );
    append_string_actions(
        &mut actions,
        packet
            .get("searchSynthesis")
            .and_then(|synthesis| synthesis.get("testFrontier")),
        "tests",
    );
    append_owner_paths(&mut actions, packet.get("owners"));
    append_native_fact_owners(&mut actions, packet.get("nativeSyntaxFacts"));
    append_item_symbols(&mut actions, packet.get("items"));
    actions
}

pub(super) fn query_term_count(packet: &Value) -> Option<usize> {
    let symbol_seed_count = graph_actions(packet)
        .into_iter()
        .filter(|action| action.kind == "symbol")
        .count();
    if symbol_seed_count > 0 {
        return Some(symbol_seed_count);
    }
    let query = packet_query(packet)?;
    let count = query
        .split('|')
        .filter(|term| !term.trim().is_empty())
        .count();
    (count > 0).then_some(count)
}

pub(super) fn graph_action_spec(kind: &str) -> Option<GraphActionSpec> {
    GRAPH_ACTION_SPECS
        .iter()
        .find_map(|(candidate, spec)| (*candidate == kind).then_some(*spec))
}

fn synthesis_seeds_are_primary(packet: &Value) -> bool {
    packet_view(packet) == "prime" && graph_root(packet, "prime") != "."
}

fn packet_root_action(packet: &Value) -> Option<GraphAction> {
    let mode = packet_view(packet);
    let query = packet_query(packet)?;
    match mode {
        "owner" | "tests" => Some(GraphAction {
            kind: mode.to_string(),
            target: query.to_string(),
            locator: None,
        }),
        "dependency" | "deps" => Some(GraphAction {
            kind: "dependency".to_string(),
            target: query.to_string(),
            locator: None,
        }),
        "fzf" => {
            let action = packet
                .get("header")
                .and_then(|header| header.get("fields"))
                .and_then(|fields| fields.get("skipped"))
                .and_then(header_field_scalar)
                .filter(|skipped| skipped == "code-shaped-query")
                .map(|_| "query")
                .unwrap_or("fzf");
            Some(GraphAction {
                kind: action.to_string(),
                target: query.to_string(),
                locator: None,
            })
        }
        _ => None,
    }
}

fn append_object_actions(actions: &mut Vec<GraphAction>, value: Option<&Value>) {
    let Some(values) = value.and_then(Value::as_array) else {
        return;
    };
    for value in values {
        if let Some(action) = action_from_value(value) {
            actions.push(action);
        }
    }
}

fn append_string_actions(actions: &mut Vec<GraphAction>, value: Option<&Value>, kind: &str) {
    let Some(values) = value.and_then(Value::as_array) else {
        return;
    };
    for value in values {
        let Some(target) = value.as_str().filter(|target| !target.trim().is_empty()) else {
            continue;
        };
        actions.push(GraphAction {
            kind: kind.to_string(),
            target: target.to_string(),
            locator: None,
        });
    }
}

fn append_owner_paths(actions: &mut Vec<GraphAction>, value: Option<&Value>) {
    let Some(values) = value.and_then(Value::as_array) else {
        return;
    };
    for value in values {
        if let Some(target) = value.get("path").and_then(Value::as_str) {
            actions.push(GraphAction {
                kind: "owner".to_string(),
                target: target.to_string(),
                locator: None,
            });
        }
    }
}

fn append_native_fact_owners(actions: &mut Vec<GraphAction>, value: Option<&Value>) {
    let Some(values) = value.and_then(Value::as_array) else {
        return;
    };
    for value in values {
        let target = value
            .get("ownerPath")
            .or_else(|| value.get("owner"))
            .or_else(|| value.get("path"))
            .and_then(Value::as_str);
        if let Some(target) = target {
            actions.push(GraphAction {
                kind: "owner".to_string(),
                target: target.to_string(),
                locator: None,
            });
        }
    }
}

fn append_item_symbols(actions: &mut Vec<GraphAction>, value: Option<&Value>) {
    let Some(values) = value.and_then(Value::as_array) else {
        return;
    };
    for value in values {
        let target = value
            .get("name")
            .or_else(|| value.get("symbol"))
            .or_else(|| value.get("target"))
            .and_then(Value::as_str);
        if let Some(target) = target {
            actions.push(GraphAction {
                kind: "item-symbol".to_string(),
                target: target.to_string(),
                locator: graph_item_locator(value),
            });
        }
    }
}

fn graph_item_locator(value: &Value) -> Option<String> {
    value
        .get("fields")
        .and_then(|fields| fields.get("read"))
        .and_then(Value::as_str)
        .or_else(|| value.get("read").and_then(Value::as_str))
        .map(str::to_string)
}

fn action_from_value(value: &Value) -> Option<GraphAction> {
    Some(GraphAction {
        kind: value.get("kind")?.as_str()?.to_string(),
        target: value
            .get("target")
            .or_else(|| value.get("ownerPath"))?
            .as_str()?
            .to_string(),
        locator: value
            .get("read")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}
