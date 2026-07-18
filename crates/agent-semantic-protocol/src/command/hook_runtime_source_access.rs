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

pub(super) fn collect_activation_path_values(value: &serde_json::Value, paths: &mut Vec<String>) {
    match value {
        serde_json::Value::String(text) if looks_like_path(text) => paths.push(text.to_string()),
        serde_json::Value::Array(values) => {
            for value in values {
                collect_activation_path_values(value, paths);
            }
        }
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                if matches!(
                    key.as_str(),
                    "file_path" | "filePath" | "path" | "paths" | "file" | "files" | "selector"
                ) {
                    collect_activation_path_values(value, paths);
                }
            }
        }
        _ => {}
    }
}

pub(super) fn collect_activation_command_paths(command: &str, paths: &mut Vec<String>) {
    for token in command.split_whitespace().skip(1) {
        let token = token.trim_matches(|character: char| {
            matches!(
                character,
                '\'' | '"' | ',' | ';' | ':' | '(' | ')' | '[' | ']'
            )
        });
        if looks_like_path(token) {
            paths.push(token.to_string());
        }
    }
}

fn looks_like_path(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && !value.starts_with('-')
        && (value == "."
            || value == ".."
            || value.contains('/')
            || value.contains('\\')
            || std::path::Path::new(value).extension().is_some())
}
