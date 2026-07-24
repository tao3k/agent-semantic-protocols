//! `semantic-tree-sitter-query` compact replay rendering.

use serde_json::Value;

const SEMANTIC_TREE_SITTER_QUERY_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-tree-sitter-query";

macro_rules! syntax_query_replay_text {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub struct $name(String);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }
    };
}

syntax_query_replay_text!(SyntaxQueryMatchLocator);
syntax_query_replay_text!(SyntaxQueryCaptureLocator);
syntax_query_replay_text!(SyntaxQueryCaptureName);
syntax_query_replay_text!(SyntaxQueryNodeType);
syntax_query_replay_text!(SyntaxQueryFieldName);
syntax_query_replay_text!(SyntaxQueryCapturedText);

/// One syntax capture row projected from a replay packet or DB row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxQueryReplayCapture {
    pub match_locator: SyntaxQueryMatchLocator,
    pub capture_locator: SyntaxQueryCaptureLocator,
    pub capture_name: SyntaxQueryCaptureName,
    pub capture_node_type: Option<SyntaxQueryNodeType>,
    pub item_node_type: Option<SyntaxQueryNodeType>,
    pub field: Option<SyntaxQueryFieldName>,
    pub text: SyntaxQueryCapturedText,
}

/// Request for rendering DB-backed syntax query replay rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxQueryRowsReplay<'a> {
    pub language_id: &'a str,
    pub input_form: &'a str,
    pub input_kind: &'a str,
    pub grammar_id: &'a str,
    pub grammar_profile_version: &'a str,
    pub compiled_source: &'a str,
    pub captures: &'a [String],
    pub rows: Vec<SyntaxQueryReplayCapture>,
}

/// Render a semantic tree-sitter query packet into compact stdout.
pub fn render_semantic_tree_sitter_query_stdout(packet: &Value) -> Option<String> {
    if string_field(packet, "schemaId")? != SEMANTIC_TREE_SITTER_QUERY_SCHEMA_ID {
        return None;
    }
    let matches = packet.get("matches")?.as_array()?;
    if matches.is_empty() {
        return render_syntax_query_miss_stdout(packet);
    }

    let query = packet.get("query")?;
    let query_fields = query.get("fields");
    let language_id = string_field(packet, "languageId").unwrap_or("unknown");
    let query_input = string_field(query, "compiledSource")
        .or_else(|| string_field(query, "input"))
        .unwrap_or("");
    let query_plan = parse_query_abi_source(query_input);
    let query_node_type = query_fields
        .and_then(|fields| string_array_field(fields, "nodeTypes").first().cloned())
        .or_else(|| {
            query_plan
                .as_ref()
                .and_then(|plan| plan.node_types.first().cloned())
        });
    let query_capture_node_type = query_plan
        .as_ref()
        .and_then(|plan| plan.node_types.last().cloned())
        .or_else(|| query_node_type.clone());
    let query_field = query_fields
        .and_then(|fields| string_array_field(fields, "fields").first().cloned())
        .or_else(|| {
            query_plan
                .as_ref()
                .and_then(|plan| plan.fields.first().cloned())
        });
    let mut rows = Vec::new();
    for item in matches {
        let match_locator = item.get("range").and_then(syntax_range_locator);
        let captures = item.get("captures")?.as_array()?;
        for capture in captures {
            let Some(text) = syntax_capture_text(capture).or_else(|| syntax_capture_text(item))
            else {
                continue;
            };
            let capture_locator = capture
                .get("range")
                .and_then(syntax_range_locator)
                .or_else(|| match_locator.clone())?;
            let item_locator = match_locator
                .clone()
                .or_else(|| {
                    capture
                        .get("fields")
                        .and_then(|fields| string_field(fields, "itemRead"))
                        .map(str::to_string)
                })
                .unwrap_or_else(|| capture_locator.clone());
            rows.push(SyntaxQueryReplayCapture {
                match_locator: item_locator,
                capture_locator,
                capture_name: string_field(capture, "name")
                    .unwrap_or("capture")
                    .to_string(),
                capture_node_type: string_field(capture, "nodeType")
                    .map(str::to_string)
                    .or_else(|| query_capture_node_type.clone()),
                item_node_type: syntax_item_node_type(item, capture)
                    .map(str::to_string)
                    .or_else(|| {
                        let capture_node_type = string_field(capture, "nodeType");
                        if capture_node_type == query_capture_node_type.as_deref() {
                            query_node_type.clone()
                        } else {
                            capture_node_type.map(str::to_string)
                        }
                    })
                    .or_else(|| query_node_type.clone()),
                field: string_field(capture, "field")
                    .map(str::to_string)
                    .or_else(|| query_field.clone()),
                text: text.to_string(),
            });
        }
    }
    if rows.is_empty() {
        render_syntax_query_miss_stdout(packet)
    } else {
        Some(render_syntax_query_frontier_graph(
            language_id,
            query_node_type.as_deref(),
            query_field.as_deref(),
            query_fields
                .and_then(|fields| string_array_field(fields, "captures").first().cloned())
                .as_deref(),
            packet.get("execution"),
            &rows,
        ))
    }
}

/// Render DB-backed semantic tree-sitter query rows into compact stdout.
pub fn render_semantic_tree_sitter_query_rows_stdout(replay: SyntaxQueryRowsReplay<'_>) -> String {
    if replay.rows.is_empty() {
        return render_syntax_query_miss_line(
            replay.input_form,
            replay.input_kind,
            replay.grammar_id,
            replay.grammar_profile_version,
            replay.captures,
        );
    }
    let query_plan = parse_query_abi_source(replay.compiled_source);
    let query_node_type = query_plan
        .as_ref()
        .and_then(|plan| plan.node_types.first().map(String::as_str))
        .or_else(|| {
            replay
                .rows
                .first()
                .and_then(|row| row.item_node_type.as_deref())
        });
    let query_field = query_plan
        .as_ref()
        .and_then(|plan| plan.fields.first().map(String::as_str))
        .or_else(|| replay.rows.first().and_then(|row| row.field.as_deref()));
    render_syntax_query_frontier_graph(
        replay.language_id,
        query_node_type,
        query_field,
        replay.captures.first().map(String::as_str),
        None,
        &replay.rows,
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct QueryAbiPlan {
    node_types: Vec<String>,
    fields: Vec<String>,
}

fn parse_query_abi_source(source: &str) -> Option<QueryAbiPlan> {
    let mut node_types = Vec::new();
    let mut fields = Vec::new();
    for token in source.split(|ch: char| ch.is_whitespace() || matches!(ch, '(' | ')' | '@')) {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        if token == "_" || token.starts_with('#') {
            continue;
        }
        if token.contains('.') {
            continue;
        }
        if let Some(field) = token.strip_suffix(':') {
            if !field.is_empty() {
                fields.push(field.to_string());
            }
        } else {
            node_types.push(token.to_string());
        }
    }
    (!node_types.is_empty() || !fields.is_empty()).then_some(QueryAbiPlan { node_types, fields })
}

fn render_syntax_query_frontier_graph(
    language_id: &str,
    query_node_type: Option<&str>,
    query_field: Option<&str>,
    capture_name: Option<&str>,
    execution: Option<&Value>,
    rows: &[SyntaxQueryReplayCapture],
) -> String {
    let pattern = syntax_query_pattern_label(query_node_type, query_field);
    let capture = capture_name
        .or_else(|| rows.first().map(|row| row.capture_name.as_str()))
        .unwrap_or("capture");
    let execution_mode = execution
        .and_then(|execution| string_field(execution, "engine"))
        .unwrap_or("unknown");
    let elapsed_ms = execution
        .and_then(|execution| execution.get("elapsedMs"))
        .and_then(Value::as_u64)
        .map_or_else(|| "unknown".to_string(), |elapsed| elapsed.to_string());
    let mut output = String::new();
    output.push_str(&format!(
        "[query-treesitter] root=. lang={language_id} pattern={pattern} capture={capture} mode={execution_mode} alg=syntax-capture-frontier elapsedMs={elapsed_ms}\n"
    ));
    output.push_str(
        "legend: aliases ID:kind; node ID=kind:role(value)!next; ts=node/field; frontier ID.next\n",
    );
    output.push_str("aliases=G:query,Q:tsquery,C:capture,I:item,O:owner\n\n");
    output.push_str(&format!("Q=tsquery:pattern({pattern})!query\n"));

    for (index, row) in rows.iter().enumerate() {
        let capture_id = graph_id("C", index);
        let item_id = graph_id("I", index);
        let ts = syntax_ts_label(row.capture_node_type.as_deref(), row.field.as_deref());
        let item_kind = syntax_item_kind(row.item_node_type.as_deref(), &row.capture_name);
        let item_ts = row.item_node_type.as_deref().unwrap_or("node");
        let graph_value = syntax_graph_value(row.capture_node_type.as_deref(), &row.text);
        output.push_str(&format!(
            "{capture_id}=capture:{}({})@{}!code ts={}\n",
            row.capture_name, graph_value, row.capture_locator, ts
        ));
        output.push_str(&format!(
            "{item_id}=item:{item_kind}({})@{}!code ts={item_ts}\n",
            graph_value, row.match_locator
        ));
    }

    output.push('\n');
    output.push_str("G>{Q:selects}\n");
    output.push_str("Q>{");
    output.push_str(
        &rows
            .iter()
            .enumerate()
            .map(|(index, _)| format!("{}:captures", graph_id("C", index)))
            .collect::<Vec<_>>()
            .join(","),
    );
    output.push_str("}\n");
    for (index, _) in rows.iter().enumerate() {
        output.push_str(&format!(
            "{}>{{{}:enclosing-item}}\n",
            graph_id("C", index),
            graph_id("I", index)
        ));
    }

    output.push('\n');
    output.push_str("omit=code,full-node-list,capture-text\n");
    output.push_str("rank=");
    output.push_str(
        &rows
            .iter()
            .enumerate()
            .map(|(index, _)| graph_id("I", index))
            .collect::<Vec<_>>()
            .join(","),
    );
    output.push('\n');
    output.push_str("frontier=");
    output.push_str(
        &rows
            .iter()
            .enumerate()
            .map(|(index, _)| format!("{}.code", graph_id("I", index)))
            .collect::<Vec<_>>()
            .join(","),
    );
    output.push('\n');
    output.push_str("avoid=broad-code-output,raw-read\n");
    output
}

fn graph_id(prefix: &str, index: usize) -> String {
    if index == 0 {
        prefix.to_string()
    } else {
        format!("{prefix}{}", index + 1)
    }
}

fn syntax_query_pattern_label(node_type: Option<&str>, field: Option<&str>) -> String {
    match (node_type, field) {
        (Some(node_type), Some(field)) => format!("{node_type}/{field}"),
        (Some(node_type), None) => node_type.to_string(),
        (None, Some(field)) => format!("field/{field}"),
        (None, None) => "query".to_string(),
    }
}

fn syntax_ts_label(node_type: Option<&str>, field: Option<&str>) -> String {
    match (node_type, field) {
        (Some(node_type), Some(field)) => format!("{node_type}/{field}"),
        (Some(node_type), None) => node_type.to_string(),
        (None, Some(field)) => format!("node/{field}"),
        (None, None) => "node".to_string(),
    }
}

fn syntax_item_kind(node_type: Option<&str>, capture_name: &str) -> &'static str {
    let node_type = node_type.unwrap_or("");
    if node_type.contains("function") || capture_name.contains("function") {
        "fn"
    } else if node_type.contains("struct")
        || node_type.contains("enum")
        || node_type.contains("type")
        || capture_name.contains("type")
    {
        "type"
    } else if node_type.contains("import") || capture_name.contains("import") {
        "import"
    } else if node_type.contains("call") || capture_name.contains("call") {
        "call"
    } else {
        "item"
    }
}

fn syntax_graph_value(node_type: Option<&str>, text: &str) -> String {
    match node_type {
        Some("identifier" | "field_identifier" | "type_identifier" | "scoped_identifier") => {
            text.to_string()
        }
        Some(node_type) => format!("<{node_type}>"),
        None => "<node>".to_string(),
    }
}

fn render_syntax_query_miss_stdout(packet: &Value) -> Option<String> {
    let query = packet.get("query")?;
    let input_form = string_field(query, "inputForm").unwrap_or("s-expression");
    let input = if string_field(query, "catalogId").is_some() {
        "catalog"
    } else {
        "inline"
    };
    let grammar = string_field(packet, "grammarId").unwrap_or("unknown");
    let grammar_profile = string_field(packet, "grammarProfileVersion").unwrap_or("unknown");
    let captures = query
        .get("fields")
        .and_then(|fields| fields.get("captures"))
        .and_then(Value::as_array)
        .map(|captures| {
            captures
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Some(render_syntax_query_miss_line(
        input_form,
        input,
        grammar,
        grammar_profile,
        &captures,
    ))
}

fn render_syntax_query_miss_line(
    input_form: &str,
    input: &str,
    grammar: &str,
    grammar_profile: &str,
    captures: &[String],
) -> String {
    let captures_display = captures.join(",");
    let capture_count = if captures_display.is_empty() {
        0
    } else {
        captures.len()
    };
    format!(
        "|syntax-query inputForm={input_form} input={input} grammar={grammar} grammarProfile={grammar_profile} dialect=tree-sitter-query matchStatus=miss match=0 rows=0 truncated=false captureCount={capture_count} captures={captures_display}\n"
    )
}

fn syntax_capture_text(value: &Value) -> Option<&str> {
    value
        .get("fields")
        .and_then(|fields| string_field(fields, "symbol").or_else(|| string_field(fields, "name")))
        .or_else(|| string_field(value, "text"))
        .or_else(|| string_field(value, "name"))
}

fn syntax_item_node_type<'a>(item: &'a Value, capture: &'a Value) -> Option<&'a str> {
    item.get("fields")
        .and_then(|fields| string_field(fields, "nodeType"))
        .or_else(|| string_field(item, "nodeType"))
        .or_else(|| {
            capture
                .get("fields")
                .and_then(|fields| string_field(fields, "nativeNodeType"))
        })
}

fn syntax_range_locator(range: &Value) -> Option<String> {
    let path = string_field(range, "path")?;
    let line_range = range.get("lineRange")?;
    let (start, end) = syntax_line_range_bounds(line_range)?;
    if start == end {
        Some(format!("{path}:{start}"))
    } else {
        Some(format!("{path}:{start}:{end}"))
    }
}

fn syntax_line_range_bounds(line_range: &Value) -> Option<(String, String)> {
    if let Some(line_range) = line_range.as_str() {
        let (start, end) = line_range.split_once(':')?;
        return Some((start.to_string(), end.to_string()));
    }
    let start = line_range.get("start").and_then(Value::as_u64)?;
    let end = line_range.get("end").and_then(Value::as_u64)?;
    Some((start.to_string(), end.to_string()))
}

fn string_array_field(value: &Value, field: &str) -> Vec<String> {
    value
        .get(field)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn string_field<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}
