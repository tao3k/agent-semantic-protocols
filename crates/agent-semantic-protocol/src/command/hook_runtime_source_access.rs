pub(super) fn compact_root_source_access_message(
    decision: &serde_json::Value,
    resident_child_name: &str,
) -> Option<String> {
    let reason = decision
        .get("reasonKind")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("source-access");
    if !matches!(
        reason,
        "direct-source-read" | "bulk-source-dump" | "raw-broad-search" | "source-access-bypass"
    ) {
        return None;
    }
    let route_command = decision
        .get("routes")
        .and_then(serde_json::Value::as_array)
        .and_then(|routes| routes.first())
        .and_then(|route| route.get("argv"))
        .and_then(serde_json::Value::as_array)
        .map(|argv| {
            argv.iter()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|command| !command.is_empty());
    Some(match route_command {
        Some(command) => format!(
            "ASP denied source access (`{reason}`). Next: send this parser-owned route to resident `{resident_child_name}`: `{command}`."
        ),
        None => format!(
            "ASP denied source access (`{reason}`). Next: resume resident `{resident_child_name}` for parser-owned ASP search."
        ),
    })
}
