use serde_json::Value;

const PATH_SCALAR_KEYS: &[&str] = &[
    "path",
    "file",
    "file_name",
    "fileName",
    "file_path",
    "filePath",
    "absolute_path",
    "absolutePath",
    "relative_path",
    "relativePath",
    "glob",
    "uri",
];

const PATH_CONTAINER_KEYS: &[&str] = &[
    "paths",
    "files",
    "resource",
    "resources",
    "uris",
    "document",
    "documents",
    "text_document",
    "textDocument",
];

const PATH_VALUE_KEYS: &[&str] = &[
    "path",
    "file",
    "file_name",
    "fileName",
    "file_path",
    "filePath",
    "absolute_path",
    "absolutePath",
    "relative_path",
    "relativePath",
    "uri",
    "paths",
    "files",
    "resource",
    "resources",
    "uris",
    "document",
    "documents",
    "text_document",
    "textDocument",
];

pub(super) fn extract_paths_direct(tool_input: &Value) -> Vec<String> {
    let mut paths = path_values_for_keys(tool_input, PATH_SCALAR_KEYS);
    paths.extend(path_values_for_keys(tool_input, PATH_CONTAINER_KEYS));
    paths
}

pub(super) fn extract_apply_patch_paths_direct(tool_input: &Value) -> Vec<String> {
    let Some(patch) = extract_apply_patch_text_direct(tool_input) else {
        return extract_paths_direct(tool_input);
    };
    agent_semantic_command_match::apply_patch_header_paths(patch)
}

pub(super) fn extract_apply_patch_text_direct(value: &Value) -> Option<&str> {
    if let Some(patch) = value.as_str() {
        return Some(patch);
    }
    if let Some(patch) = value.get("patch").and_then(Value::as_str) {
        return Some(patch);
    }
    for key in [
        "tool_input",
        "toolInput",
        "input",
        "arguments",
        "args",
        "parameters",
        "params",
    ] {
        if let Some(patch) = value.get(key).and_then(extract_apply_patch_text_direct) {
            return Some(patch);
        }
    }
    None
}

fn path_values_for_keys(tool_input: &Value, keys: &[&str]) -> Vec<String> {
    keys.iter()
        .filter_map(|key| tool_input.get(*key))
        .flat_map(path_values)
        .collect()
}

pub(super) fn path_values(value: &Value) -> Vec<String> {
    if let Some(path) = value.as_str() {
        return vec![normalize_path_value(path)];
    }
    if value.is_object() {
        return path_values_for_keys(value, PATH_VALUE_KEYS);
    }
    value
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(path_values)
        .collect()
}

fn normalize_path_value(path: &str) -> String {
    let Some(uri_path) = path.strip_prefix("file://") else {
        return path.to_string();
    };
    if let Some(localhost_path) = uri_path.strip_prefix("localhost/") {
        return format!("/{localhost_path}");
    }
    uri_path.to_string()
}

pub(super) fn push_unique_path(paths: &mut Vec<String>, path: String) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}
