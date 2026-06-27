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
    pub(super) syntax_query: Option<String>,
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
    "item-symbol" => "item", "symbol", "I", "syntax";
    "hot" => "hot", "symbol", "H", "syntax";
    "text" => "query", "term", "Q", "fzf";
    "fzf" => "query", "term", "Q", "fzf";
    "query" => "query", "term", "Q", "query";
    "dependency" => "dependency", "pkg", "D", "dependency";
    "deps" => "dependency", "pkg", "D", "deps";
    "docs" => "doc", "path", "D", "docs";
    "docs-use" => "doc-use", "path", "U", "docs-use";
    "crate-source" => "crate-source", "pkg", "C", "crate-source";
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
                syntax_query: None,
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
                syntax_query: None,
            }),
            "finding-frontier" => Some(GraphAction {
                kind: "finding".to_string(),
                target: header_scalar(packet, "query")
                    .or_else(|| selector_target(packet, "finding="))?,
                locator: None,
                action: None,
                syntax_query: None,
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
    let language_id = packet_language_id(packet);
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
        append_owner_item_next_actions(&mut actions, packet.get("nextActions"), language_id);
    } else {
        append_object_actions(&mut actions, packet.get("nextActions"), language_id);
    }
    if synthesis_seeds_are_primary(packet) {
        append_object_actions(
            &mut actions,
            packet.get("searchSynthesis").and_then(|s| s.get("seeds")),
            language_id,
        );
        append_object_actions(
            &mut actions,
            packet
                .get("searchSynthesis")
                .and_then(|s| s.get("windowSet")),
            language_id,
        );
    } else {
        append_object_actions(
            &mut actions,
            packet
                .get("searchSynthesis")
                .and_then(|s| s.get("windowSet")),
            language_id,
        );
        append_object_actions(
            &mut actions,
            packet.get("searchSynthesis").and_then(|s| s.get("seeds")),
            language_id,
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
    append_item_symbols(&mut actions, packet.get("items"), language_id);
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
                syntax_query: profile.syntax_query.clone(),
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
        syntax_query: None,
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
            syntax_query: None,
        }),
        "dependency" | "deps" => Some(GraphAction {
            kind: "dependency".to_string(),
            target: query.to_string(),
            locator: None,
            action: None,
            syntax_query: None,
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
                syntax_query: None,
            })
        }
        _ => None,
    }
}

fn append_object_actions(actions: &mut Vec<GraphAction>, value: Option<&Value>, language_id: &str) {
    let Some(values) = value.and_then(Value::as_array) else {
        return;
    };
    for value in values {
        if let Some(action) = action_from_value(value, language_id) {
            actions.push(action);
        }
    }
}

fn append_owner_item_next_actions(
    actions: &mut Vec<GraphAction>,
    value: Option<&Value>,
    language_id: &str,
) {
    let Some(values) = value.and_then(Value::as_array) else {
        return;
    };
    for value in values {
        if value.get("kind").and_then(Value::as_str) == Some("symbol") {
            continue;
        }
        if let Some(action) = action_from_value(value, language_id) {
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
            syntax_query: None,
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
                syntax_query: None,
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
                syntax_query: None,
            });
        }
    }
}

fn append_item_symbols(actions: &mut Vec<GraphAction>, value: Option<&Value>, language_id: &str) {
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
            let locator = graph_item_locator(value, language_id, target);
            let syntax_query = graph_item_syntax_query(value, language_id, target);
            actions.push(GraphAction {
                kind: "item-symbol".to_string(),
                target: target.to_string(),
                action: locator
                    .as_deref()
                    .map(item_frontier_action)
                    .map(ToOwned::to_owned),
                locator,
                syntax_query,
            });
        }
    }
}

fn graph_item_locator(value: &Value, language_id: &str, target: &str) -> Option<String> {
    graph_item_structural_selector(value)
        .or_else(|| graph_item_structural_selector_from_hints(value, language_id, target))
}

fn graph_item_structural_selector(value: &Value) -> Option<String> {
    value
        .get("fields")
        .and_then(|fields| fields.get("structuralSelector"))
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .get("fields")
                .and_then(|fields| fields.get("semanticSelector"))
                .and_then(Value::as_str)
        })
        .or_else(|| value.get("structuralSelector").and_then(Value::as_str))
        .or_else(|| value.get("semanticSelector").and_then(Value::as_str))
        .filter(|selector| !selector.trim().is_empty())
        .map(str::to_string)
}

fn graph_item_structural_selector_from_hints(
    value: &Value,
    language_id: &str,
    target: &str,
) -> Option<String> {
    let owner_path = graph_item_owner_path(value)?;
    let kind = graph_item_kind(value).unwrap_or(default_item_kind(language_id));
    let language = if language_id.is_empty() {
        "code"
    } else {
        language_id
    };
    Some(format!(
        "{}://{}#item/{}/{}",
        selector_token(language),
        owner_path,
        selector_token(kind),
        selector_token(target)
    ))
}

fn graph_item_owner_path(value: &Value) -> Option<String> {
    value
        .get("ownerPath")
        .and_then(Value::as_str)
        .or_else(|| value.get("path").and_then(Value::as_str))
        .or_else(|| {
            value
                .get("fields")
                .and_then(|fields| fields.get("ownerPath"))
                .and_then(Value::as_str)
        })
        .or_else(|| {
            value
                .get("fields")
                .and_then(|fields| fields.get("path"))
                .and_then(Value::as_str)
        })
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            graph_item_source_locator_hint(value).and_then(|locator| source_locator_path(&locator))
        })
}

fn graph_item_source_locator_hint(value: &Value) -> Option<String> {
    value
        .get("fields")
        .and_then(|fields| fields.get("sourceLocatorHint"))
        .and_then(Value::as_str)
        .or_else(|| value.get("sourceLocatorHint").and_then(Value::as_str))
        .or_else(|| {
            value
                .get("fields")
                .and_then(|fields| fields.get("read"))
                .and_then(Value::as_str)
        })
        .or_else(|| value.get("read").and_then(Value::as_str))
        .map(str::trim)
        .filter(|locator| !locator.is_empty())
        .map(ToOwned::to_owned)
}

fn source_locator_path(locator: &str) -> Option<String> {
    locator
        .split_once(':')
        .map(|(path, _)| path.trim())
        .filter(|path| !path.is_empty())
        .map(ToOwned::to_owned)
}

fn selector_token(value: &str) -> String {
    let token = value
        .trim()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.' | '/' | ':')
            {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    if token.is_empty() {
        "item".to_string()
    } else {
        token
    }
}

fn graph_item_syntax_query(value: &Value, language_id: &str, target: &str) -> Option<String> {
    for field in ["syntaxQuery", "syntax_query", "tsq"] {
        if let Some(query) = value
            .get("fields")
            .and_then(|fields| fields.get(field))
            .and_then(Value::as_str)
            .or_else(|| value.get(field).and_then(Value::as_str))
            .filter(|query| !query.trim().is_empty())
        {
            return Some(query.to_string());
        }
    }
    tree_sitter_pattern_for_item(
        language_id,
        graph_item_kind(value).unwrap_or(default_item_kind(language_id)),
        target,
    )
}

fn graph_item_kind(value: &Value) -> Option<&str> {
    value
        .get("fields")
        .and_then(|fields| fields.get("itemKind"))
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .get("fields")
                .and_then(|fields| fields.get("symbolKind"))
                .and_then(Value::as_str)
        })
        .or_else(|| {
            value
                .get("fields")
                .and_then(|fields| fields.get("kind"))
                .and_then(Value::as_str)
        })
        .or_else(|| value.get("itemKind").and_then(Value::as_str))
        .or_else(|| value.get("symbolKind").and_then(Value::as_str))
        .or_else(|| value.get("role").and_then(Value::as_str))
        .or_else(|| value.get("kind").and_then(Value::as_str))
}

fn graph_item_target(value: &Value) -> Option<&str> {
    value
        .get("target")
        .and_then(Value::as_str)
        .or_else(|| value.get("itemName").and_then(Value::as_str))
        .or_else(|| value.get("symbol").and_then(Value::as_str))
        .or_else(|| {
            value
                .get("fields")
                .and_then(|fields| fields.get("itemName"))
                .and_then(Value::as_str)
        })
        .or_else(|| {
            value
                .get("fields")
                .and_then(|fields| fields.get("symbol"))
                .and_then(Value::as_str)
        })
        .or_else(|| value.get("ownerPath").and_then(Value::as_str))
}

fn graph_item_ownerish_target(value: &Value) -> Option<&str> {
    value
        .get("target")
        .and_then(Value::as_str)
        .or_else(|| value.get("ownerPath").and_then(Value::as_str))
        .or_else(|| value.get("path").and_then(Value::as_str))
}

fn default_item_kind(language_id: &str) -> &'static str {
    match language_id {
        "python" => "function",
        _ => "fn",
    }
}

fn packet_language_id(packet: &Value) -> &str {
    packet
        .get("languageId")
        .and_then(Value::as_str)
        .unwrap_or_default()
}

fn tree_sitter_pattern_for_item(language_id: &str, kind: &str, target: &str) -> Option<String> {
    let escaped_target = target.replace('\\', "\\\\").replace('"', "\\\"");
    match language_id {
        "rust" => rust_tree_sitter_pattern(kind, &escaped_target),
        "python" => python_tree_sitter_pattern(kind, &escaped_target),
        _ => None,
    }
}

fn rust_tree_sitter_pattern(kind: &str, escaped_target: &str) -> Option<String> {
    let (node, capture) = match kind {
        "struct" => ("struct_item", "type.name"),
        "enum" => ("enum_item", "type.name"),
        "trait" | "trait_alias" => ("trait_item", "type.name"),
        "type" => ("type_item", "type.name"),
        "const" => ("const_item", "constant.name"),
        "static" => ("static_item", "constant.name"),
        "mod" => ("mod_item", "module.name"),
        "macro" => ("macro_definition", "macro.name"),
        "impl" => {
            return Some(format!(
                "((impl_item type: (_) @impl.target) (#match? @impl.target \"{escaped_target}\"))"
            ));
        }
        "fn" | "function" | "method" => ("function_item", "function.name"),
        _ => ("function_item", "function.name"),
    };
    Some(format!(
        "(({node} name: (_) @{capture}) (#eq? @{capture} \"{escaped_target}\"))"
    ))
}

fn python_tree_sitter_pattern(kind: &str, escaped_target: &str) -> Option<String> {
    let (node, capture) = match kind {
        "class" | "class_definition" => ("class_definition", "class.name"),
        "function" | "function_definition" | "method" | "fn" => {
            ("function_definition", "function.name")
        }
        _ => ("function_definition", "function.name"),
    };
    Some(format!(
        "(({node} name: (identifier) @{capture}) (#eq? @{capture} \"{escaped_target}\"))"
    ))
}

fn action_from_value(value: &Value, language_id: &str) -> Option<GraphAction> {
    let kind = value.get("kind")?.as_str()?.to_string();
    let action = if kind == "hot" {
        Some("syntax".to_string())
    } else {
        None
    };
    let target = graph_item_target(value)
        .or_else(|| graph_item_ownerish_target(value))?
        .to_string();
    let locator = graph_item_locator(value, language_id, &target);
    Some(GraphAction {
        kind,
        syntax_query: graph_item_syntax_query(value, language_id, &target),
        target,
        locator,
        action,
    })
}

fn item_frontier_action(_locator: &str) -> &'static str {
    "syntax"
}
