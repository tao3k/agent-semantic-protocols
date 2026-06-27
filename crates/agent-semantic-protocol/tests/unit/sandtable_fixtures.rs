use serde_json::Value;
use std::path::{Path, PathBuf};

#[test]
fn tracked_sandtable_fixtures_do_not_embed_absolute_local_paths() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("repo root");
    let sandtables_dir = repo_root.join("sandtables");
    assert!(sandtables_dir.is_dir(), "missing {:?}", sandtables_dir);

    let mut fixture_paths = Vec::new();
    collect_json_files(&sandtables_dir, &mut fixture_paths);
    assert!(
        !fixture_paths.is_empty(),
        "expected tracked sandtable JSON fixtures"
    );

    let mut violations = Vec::new();
    for path in fixture_paths {
        let text = std::fs::read_to_string(&path).expect("read sandtable fixture");
        let json: Value = serde_json::from_str(&text).expect("parse sandtable fixture JSON");
        collect_path_violations(repo_root, &path, "$", &json, &mut violations);
    }

    assert!(
        violations.is_empty(),
        "sandtable fixtures must be GitHub-portable; absolute local paths are not allowed:\n{}",
        violations.join("\n")
    );
}

fn collect_json_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).expect("read sandtable dir");
    for entry in entries {
        let path = entry.expect("sandtable dir entry").path();
        if path.is_dir() {
            collect_json_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            out.push(path);
        }
    }
    out.sort();
}

fn collect_path_violations(
    repo_root: &Path,
    fixture_path: &Path,
    pointer: &str,
    value: &Value,
    violations: &mut Vec<String>,
) {
    match value {
        Value::String(text) => {
            if contains_absolute_local_path(text) {
                let relative = fixture_path.strip_prefix(repo_root).unwrap_or(fixture_path);
                violations.push(format!("{} {pointer}: {text:?}", relative.display()));
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                collect_path_violations(
                    repo_root,
                    fixture_path,
                    &format!("{pointer}/{index}"),
                    item,
                    violations,
                );
            }
        }
        Value::Object(map) => {
            for (key, item) in map {
                collect_path_violations(
                    repo_root,
                    fixture_path,
                    &format!("{pointer}/{}", json_pointer_escape(key)),
                    item,
                    violations,
                );
            }
        }
        _ => {}
    }
}

fn contains_absolute_local_path(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with("file://")
        || trimmed.starts_with('/')
        || trimmed.starts_with("\\\\")
        || contains_windows_absolute_path(trimmed)
        || ["/Users/", "/home/", "/private/", "/tmp/", "/var/folders/"]
            .iter()
            .any(|marker| value.contains(marker))
}

fn contains_windows_absolute_path(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.windows(3).enumerate().any(|(index, window)| {
        window[0].is_ascii_alphabetic()
            && window[1] == b':'
            && matches!(window[2], b'/' | b'\\')
            && (index == 0
                || matches!(
                    bytes[index - 1],
                    b' ' | b'\t' | b'\r' | b'\n' | b'"' | b'\'' | b'=' | b'(' | b'[' | b'{'
                ))
    })
}

fn json_pointer_escape(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}
