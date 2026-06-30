//! Query owner-items packet replay matching and compact rendering.

use serde_json::Value;

/// Request facts needed to decide whether a query packet can replay.
pub struct QueryPacketReplayRequest<'a> {
    pub is_query_method: bool,
    pub forwarded_args: &'a [String],
}

/// Return whether a query owner-items packet matches one replay request.
pub fn query_packet_matches_request(packet: &Value, request: QueryPacketReplayRequest<'_>) -> bool {
    if !request.is_query_method || request.forwarded_args.iter().any(|arg| arg == "--code") {
        return false;
    }
    if string_field(packet, "schemaId") != Some("agent.semantic-protocols.semantic-query-packet") {
        return false;
    }
    if string_field(packet, "method") != Some("query/owner-items") {
        return false;
    }
    if string_field(packet, "ownerPath") != request_owner_path(request.forwarded_args) {
        return false;
    }
    string_field(packet, "query") == request_query_value(request.forwarded_args)
}

/// Render one query owner-items packet into compact replay stdout.
pub fn render_query_packet_stdout(packet: &Value) -> Option<String> {
    if string_field(packet, "schemaId")? != "agent.semantic-protocols.semantic-query-packet" {
        return None;
    }
    if string_field(packet, "method")? != "query/owner-items" {
        return None;
    }

    let query = string_field(packet, "query")?;
    let owner_path = string_field(packet, "ownerPath").unwrap_or(".");
    let package = string_field(packet, "packageName").unwrap_or(".");
    let output_mode = string_field(packet, "outputMode").unwrap_or("code");
    if output_mode == "code" {
        return None;
    }
    let match_mode = string_field(packet, "matchMode").unwrap_or("unknown");
    let matches = packet.get("matches")?.as_array()?;
    let status = query_status(packet, matches);
    let next = query_next_action(output_mode, status);

    let mut output = String::new();
    output.push_str(&format!(
        "[search-owner] q={owner_path} pkg={package} own=1 item={} itemQuery={query}\n",
        matches.len()
    ));
    output.push_str(&format!(
        "|owner {owner_path} role=source source=parser-visible-module\n"
    ));
    output.push_str(&format!(
        "|query itemQuery={query} status={status} match={match_mode} item={} reason=cache-query-packet output={output_mode} next={next}\n",
        matches.len()
    ));

    for item in matches {
        let name = string_field(item, "name")?;
        let kind = string_field(item, "kind")?;
        let read = match_read_locator(item)?;
        output.push_str(&format!(
            "|item {name} kind={kind} next=symbol:{name} read={read}\n"
        ));
        if output_mode == "code"
            && let Some(code) = string_field(item, "code")
        {
            let location = item.get("location")?;
            let path = string_field(location, "path")?;
            let line_range = string_field(location, "lineRange")?;
            let nodes = compact_projection_nodes(item);
            let text = serde_json::to_string(code).ok()?;
            let truncated = bool_field(item, "truncated").unwrap_or(false);
            output.push_str(&format!(
                "|code path={path} lineRange={line_range} reason=query-packet truncated={truncated} nodes={nodes} text={text}\n"
            ));
        }
    }
    Some(output)
}

fn request_owner_path(forwarded_args: &[String]) -> Option<&str> {
    forwarded_args
        .iter()
        .find(|arg| !arg.starts_with('-') && arg.as_str() != ".")
        .map(String::as_str)
}

fn request_query_value(forwarded_args: &[String]) -> Option<&str> {
    forwarded_args
        .windows(2)
        .find(|window| window[0] == "--query" || window[0] == "--term")
        .map(|window| window[1].as_str())
}

fn query_status<'a>(packet: &'a Value, matches: &[Value]) -> &'a str {
    packet
        .get("queryCoverage")
        .and_then(Value::as_array)
        .and_then(|coverage| coverage.first())
        .and_then(|entry| string_field(entry, "status"))
        .unwrap_or(if matches.is_empty() { "miss" } else { "hit" })
}

fn query_next_action(output_mode: &str, status: &str) -> &'static str {
    if status == "miss" {
        "revise-query"
    } else if output_mode == "code" {
        "code"
    } else {
        "select-item"
    }
}

fn match_read_locator(item: &Value) -> Option<String> {
    if let Some(read) = string_field(item, "read") {
        return Some(read.to_string());
    }
    let location = item.get("location")?;
    let path = string_field(location, "path")?;
    let line_range = string_field(location, "lineRange")?;
    Some(format!("{path}:{line_range}"))
}

fn compact_projection_nodes(item: &Value) -> String {
    item.get("projection")
        .and_then(|projection| projection.get("nodes"))
        .and_then(Value::as_array)
        .map(|nodes| {
            nodes
                .iter()
                .filter_map(|node| {
                    let id = string_field(node, "id")?;
                    let kind = string_field(node, "kind").unwrap_or("node");
                    let role = string_field(node, "role").unwrap_or("semantic");
                    Some(format!("{id}:{kind}:{role}"))
                })
                .collect::<Vec<_>>()
                .join(",")
        })
        .filter(|nodes| !nodes.is_empty())
        .unwrap_or_else(|| "-".to_string())
}

fn string_field<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

fn bool_field(value: &Value, field: &str) -> Option<bool> {
    value.get(field).and_then(Value::as_bool)
}
