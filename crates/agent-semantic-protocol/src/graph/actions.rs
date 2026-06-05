use serde_json::Value;

use super::packet::{
    graph_root, header_field_scalar, is_owner_item_query_packet, packet_query, packet_view,
};

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
    pub(super) action: Option<String>,
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
    "item-symbol" => "item", "symbol", "I", "code";
    "hot" => "hot", "symbol", "H", "code";
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
    fn header_scalar(packet: &Value, field: &str) -> Option<String> {
        packet
            .get("header")
            .and_then(|header| header.get("fields"))
            .and_then(|fields| fields.get(field))
            .and_then(header_field_scalar)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    }

    fn push_header_action(actions: &mut Vec<GraphAction>, kind: &str, target: Option<String>) {
        if let Some(target) = target {
            actions.push(GraphAction {
                kind: kind.to_string(),
                target,
                locator: None,
                action: None,
            });
        }
    }

    fn selector_target(packet: &Value, prefix: &str) -> Option<String> {
        header_scalar(packet, "selector").and_then(|selector| {
            selector
                .strip_prefix(prefix)
                .map(str::to_string)
                .filter(|value| !value.trim().is_empty())
        })
    }

    fn reasoning_profile_action(packet: &Value) -> Option<GraphAction> {
        match packet_query(packet)? {
            "feature-cfg" => Some(GraphAction {
                kind: "feature".to_string(),
                target: header_scalar(packet, "query")
                    .or_else(|| selector_target(packet, "feature="))?,
                locator: None,
                action: None,
            }),
            "finding-frontier" => Some(GraphAction {
                kind: "finding".to_string(),
                target: header_scalar(packet, "query")
                    .or_else(|| selector_target(packet, "finding="))?,
                locator: None,
                action: None,
            }),
            _ => None,
        }
    }

    fn is_reasoning_profile_duplicate(profile: &GraphAction, action: &GraphAction) -> bool {
        if action.target != profile.target {
            return false;
        }
        match profile.kind.as_str() {
            "feature" => matches!(action.kind.as_str(), "query" | "features"),
            "finding" => action.kind == "query",
            _ => false,
        }
    }

    let mut actions = Vec::new();
    let reasoning_profile = (packet_view(packet) == "reasoning")
        .then(|| reasoning_profile_action(packet))
        .flatten();
    if packet_view(packet) == "reasoning" {
        if reasoning_profile.is_none() {
            push_header_action(&mut actions, "query", header_scalar(packet, "query"));
            push_header_action(
                &mut actions,
                "dependency",
                header_scalar(packet, "dependency"),
            );
        }
        push_header_action(
            &mut actions,
            "owner",
            header_scalar(packet, "ownerSelector"),
        );
    }
    if let Some(action) = packet_root_action(packet) {
        actions.push(action);
    }
    if is_owner_item_query_packet(packet, packet_view(packet))
        && let Some(action) = owner_item_query_action(packet)
    {
        actions.push(action);
    }
    if is_owner_item_query_packet(packet, packet_view(packet)) {
        append_owner_item_next_actions(&mut actions, packet.get("nextActions"));
    } else {
        append_object_actions(&mut actions, packet.get("nextActions"));
    }
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
    if let Some(profile) = reasoning_profile.as_ref() {
        actions.retain(|action| {
            !(is_reasoning_profile_duplicate(profile, action)
                || (action.kind == profile.kind && action.target == profile.target))
        });
        actions.insert(
            0,
            GraphAction {
                kind: profile.kind.clone(),
                target: profile.target.clone(),
                locator: profile.locator.clone(),
                action: profile.action.clone(),
            },
        );
    }
    actions
}

pub(super) fn query_term_count(packet: &Value) -> Option<usize> {
    if is_owner_item_query_packet(packet, packet_view(packet)) {
        return owner_item_query_terms(packet).map(|terms| terms.len());
    }
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

fn owner_item_query_action(packet: &Value) -> Option<GraphAction> {
    Some(GraphAction {
        kind: "query".to_string(),
        target: owner_item_query_terms(packet)?.join("|"),
        locator: None,
        action: Some("query".to_string()),
    })
}

fn owner_item_query_terms(packet: &Value) -> Option<Vec<String>> {
    let query_set_terms = packet
        .get("querySet")
        .and_then(Value::as_array)
        .map(|terms| {
            terms
                .iter()
                .filter_map(query_set_term_value)
                .collect::<Vec<_>>()
        })
        .filter(|terms| !terms.is_empty());
    query_set_terms.or_else(|| {
        header_scalar_value(packet, "itemQuery").and_then(|query| {
            let terms = query
                .split('|')
                .map(str::trim)
                .filter(|term| !term.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>();
            (!terms.is_empty()).then_some(terms)
        })
    })
}

fn query_set_term_value(value: &Value) -> Option<String> {
    value
        .as_str()
        .or_else(|| value.get("value").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn header_scalar_value(packet: &Value, field: &str) -> Option<String> {
    packet
        .get("header")
        .and_then(|header| header.get("fields"))
        .and_then(|fields| fields.get(field))
        .and_then(header_field_scalar)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(super) fn graph_action_spec(kind: &str) -> Option<GraphActionSpec> {
    match kind {
        "feature" => Some(GraphActionSpec {
            node_type: "feature",
            target_role: "feature",
            alias_prefix: "F",
            action: "cfg",
        }),
        "finding" => Some(GraphActionSpec {
            node_type: "finding",
            target_role: "finding",
            alias_prefix: "F",
            action: "finding",
        }),
        _ => GRAPH_ACTION_SPECS
            .iter()
            .find_map(|(candidate, spec)| (*candidate == kind).then_some(*spec)),
    }
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
            action: None,
        }),
        "dependency" | "deps" => Some(GraphAction {
            kind: "dependency".to_string(),
            target: query.to_string(),
            locator: None,
            action: None,
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
                action: None,
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

fn append_owner_item_next_actions(actions: &mut Vec<GraphAction>, value: Option<&Value>) {
    let Some(values) = value.and_then(Value::as_array) else {
        return;
    };
    for value in values {
        if value.get("kind").and_then(Value::as_str) == Some("symbol") {
            continue;
        }
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
            action: None,
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
                action: None,
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
                action: None,
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
            let locator = graph_item_locator(value);
            actions.push(GraphAction {
                kind: "item-symbol".to_string(),
                target: target.to_string(),
                action: locator
                    .as_deref()
                    .map(item_frontier_action)
                    .map(ToOwned::to_owned),
                locator,
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
    let kind = value.get("kind")?.as_str()?.to_string();
    let locator = value
        .get("read")
        .and_then(Value::as_str)
        .map(str::to_string);
    let action = if kind == "hot" {
        Some("code".to_string())
    } else {
        None
    };
    Some(GraphAction {
        kind,
        target: value
            .get("target")
            .or_else(|| value.get("ownerPath"))?
            .as_str()?
            .to_string(),
        locator,
        action,
    })
}

fn item_frontier_action(locator: &str) -> &'static str {
    let mut parts = locator.rsplit(':');
    let Some(end) = parts.next().and_then(|part| part.parse::<usize>().ok()) else {
        return "code";
    };
    let Some(start) = parts.next().and_then(|part| part.parse::<usize>().ok()) else {
        return "code";
    };
    if end.saturating_sub(start).saturating_add(1) > 40 {
        "outline"
    } else {
        "code"
    }
}
