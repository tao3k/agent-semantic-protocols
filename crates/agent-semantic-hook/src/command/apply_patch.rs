//! Lightweight apply_patch command detection for hook routing.
//!
//! This parser only decides whether a source-file text patch should be blocked
//! before it reaches Codex apply_patch. It does not validate or apply patches.

const PATCH_MARKERS: &[&str] = &[
    "*** Add File: ",
    "*** Delete File: ",
    "*** Update File: ",
    "*** Move to: ",
];

pub(crate) fn apply_patch_source_paths(tool_name: &str, command: &str) -> Vec<String> {
    if !looks_like_apply_patch_invocation(tool_name, command) {
        return Vec::new();
    }
    patch_paths(command)
}

fn looks_like_apply_patch_invocation(tool_name: &str, command: &str) -> bool {
    let tool_name = tool_name.to_ascii_lowercase();
    let command = command.trim();
    let command_lower = command.to_ascii_lowercase();
    let apply_patch_tool = tool_name == "apply_patch"
        || tool_name.ends_with(".apply_patch")
        || tool_name == "applypatch";
    let apply_patch_command =
        command_lower.contains("apply_patch") || command_lower.contains("applypatch");
    let patch_markers = command.contains("*** Begin Patch") && command.contains("*** End Patch");
    patch_markers && (apply_patch_tool || apply_patch_command)
}

fn patch_paths(command: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for line in command.lines() {
        let line = line.trim_start();
        for marker in PATCH_MARKERS {
            let Some(path) = line.strip_prefix(marker) else {
                continue;
            };
            push_unique_path(&mut paths, clean_patch_path(path));
        }
    }
    paths
}

fn clean_patch_path(path: &str) -> String {
    path.trim()
        .trim_matches(|character| matches!(character, '\'' | '"' | '`'))
        .to_string()
}

fn push_unique_path(paths: &mut Vec<String>, path: String) {
    if path.is_empty() || paths.iter().any(|existing| existing == &path) {
        return;
    }
    paths.push(path);
}
