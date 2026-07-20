use crate::provider_command::support;
use crate::provider_command::facade::pipe::assert_graph_turbo_request_contract;
use serde_json::Value;

pub(super) fn write_dependency_topology_provider(
    bin_dir: &std::path::Path,
    binary: &str,
    marker: &std::path::Path,
    dependency: &str,
    version: &str,
    manifest_path: &str,
) {
    std::fs::create_dir_all(bin_dir).expect("create fake provider bin dir");
    let path = bin_dir.join(binary);
    std::fs::write(
        &path,
        format!(
            "#!/bin/sh\nprintf called > '{}'\ncat <<'JSON'\n{{\"packetKind\":\"dependency-topology\",\"fingerprint\":\"sha256:2222222222222222222222222222222222222222222222222222222222222222\",\"graph\":{{\"nodes\":[{{\"id\":\"dependency:{}\",\"kind\":\"dependency\",\"value\":\"{}\",\"path\":\"{}\",\"fields\":{{\"dependencyName\":\"{}\",\"manifestPath\":\"{}\"}}}},{{\"id\":\"dependency-version:{}\",\"kind\":\"dependency-version\",\"value\":\"{}\",\"fields\":{{\"version\":\"{}\"}}}}],\"edges\":[{{\"source\":\"dependency:{}\",\"target\":\"dependency-version:{}\",\"relation\":\"version_locked\"}}]}}}}\nJSON\n",
            marker.display(),
            dependency,
            dependency,
            manifest_path,
            dependency,
            manifest_path,
            dependency,
            version,
            version,
            dependency,
            dependency
        ),
    )
    .expect("write fake provider");
    support::make_executable(&path);
}

pub(super) fn rust_dependency_graph_request_payload(
    root: &std::path::Path,
    bin_dir: &std::path::Path,
    cache_home: &std::path::Path,
    query: &str,
) -> Value {
    let output = support::asp_command(root)
        .env("PATH", support::prepend_path(bin_dir))
        .env("PRJ_CACHE_HOME", cache_home)
        .args([
            "rust",
            "search",
            "pipe",
            query,
            "--view",
            "graph-turbo-request",
            ".",
        ])
        .output()
        .expect("run asp rust search pipe graph request");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: Value = serde_json::from_slice(&output.stdout).expect("graph request json");
    assert_graph_turbo_request_contract(&payload);
    payload
}

pub(super) fn assert_manifest_dependency_version(payload: &Value, dependency: &str, version: &str) {
    let nodes = payload["graph"]["nodes"].as_array().expect("nodes");
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("dependency")
                && node["value"].as_str() == Some(dependency)
                && node["confidence"].as_str() == Some("exact")
        }),
        "{payload}"
    );
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("dependency-version")
                && node["value"].as_str() == Some(&format!("{dependency}@{version}"))
        }),
        "{payload}"
    );
    let edges = payload["graph"]["edges"].as_array().expect("edges");
    assert!(
        edges
            .iter()
            .any(|edge| edge["relation"].as_str() == Some("version_locked")),
        "{payload}"
    );
}

pub(super) fn assert_manifest_dependency(payload: &Value, dependency: &str) {
    let nodes = payload["graph"]["nodes"].as_array().expect("nodes");
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("dependency")
                && node["value"].as_str() == Some(dependency)
                && node["confidence"].as_str() == Some("exact")
        }),
        "{payload}"
    );
}

pub(super) fn assert_provider_topology_marker(
    payload: &Value,
    language: &str,
    project_marker: &str,
    dependency_marker: &str,
) {
    let nodes = payload["graph"]["nodes"].as_array().expect("nodes");
    let project_id = format!("language-project:{language}-.");
    assert!(
        nodes.iter().any(|node| {
            node["id"].as_str() == Some(project_id.as_str())
                && node["kind"].as_str() == Some("language-project")
                && node["role"].as_str() == Some("project-root")
                && node["fields"]["languageId"].as_str() == Some(language)
                && node["fields"]["projectMarker"].as_str() == Some(project_marker)
        }),
        "language={language} payload={payload}"
    );
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("project-marker")
                && node["role"].as_str() == Some("project-marker")
                && node["path"].as_str() == Some(project_marker)
                && node["fields"]["marker"].as_str() == Some(project_marker)
        }),
        "language={language} payload={payload}"
    );
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("dependency-marker")
                && node["role"].as_str() == Some("dependency-source")
                && node["path"].as_str() == Some(dependency_marker)
                && node["fields"]["marker"].as_str() == Some(dependency_marker)
        }),
        "language={language} payload={payload}"
    );
    let edges = payload["graph"]["edges"].as_array().expect("edges");
    assert!(
        edges.iter().any(|edge| {
            edge["relation"].as_str() == Some("has_language_project")
                && edge["target"].as_str() == Some(project_id.as_str())
        }),
        "language={language} payload={payload}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge["relation"].as_str() == Some("declared_by")
                && edge["source"].as_str() == Some(project_id.as_str())
        }),
        "language={language} payload={payload}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge["relation"].as_str() == Some("uses_dependency_marker")
                && edge["source"].as_str() == Some(project_id.as_str())
        }),
        "language={language} payload={payload}"
    );
}
