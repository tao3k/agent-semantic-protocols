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
        "direct-source-read"
            | "structured-source-read"
            | "bulk-source-dump"
            | "raw-broad-search"
            | "source-access-bypass"
    ) {
        return None;
    }
    if reason == "structured-source-read" {
        let fields = decision.get("fields");
        let grammar = fields
            .and_then(|fields| fields.get("filterGrammar"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("bounded-path-v1");
        let path = decision
            .get("subject")
            .and_then(|subject| subject.get("paths"))
            .and_then(serde_json::Value::as_array)
            .and_then(|paths| paths.first())
            .and_then(serde_json::Value::as_str);
        let binary_field = if path.is_some_and(|path| path.ends_with(".toml")) {
            "tomlBinary"
        } else {
            "jsonBinary"
        };
        let binary = fields
            .and_then(|fields| fields.get(binary_field))
            .and_then(serde_json::Value::as_str)
            .unwrap_or(if binary_field == "tomlBinary" {
                "yq"
            } else {
                "jq"
            });
        let target = path.map_or_else(String::new, |path| format!(" against `{path}`"));
        return Some(format!(
            "ASP denied source access (`{reason}`). Next: use the configured `{binary}` structured reader with `{grammar}`{target}; do not retry raw Read."
        ));
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

#[cfg(test)]
#[path = "../../tests/unit/hook_runtime_source_access_message.rs"]
mod tests;
