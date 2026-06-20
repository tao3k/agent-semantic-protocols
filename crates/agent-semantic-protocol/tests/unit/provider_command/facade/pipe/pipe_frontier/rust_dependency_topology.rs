use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_marker_provider,
};
use serde_json::Value;

#[test]
fn search_pipe_graph_request_uses_rust_manifest_dependency_versions() {
    let root = temp_project_root("search-pipe-rust-dependency-topology");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"dep-topology-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nserde = \"1\"\n",
    )
    .expect("write Cargo.toml");
    std::fs::write(
        root.join("Cargo.lock"),
        "[[package]]\nname = \"serde\"\nversion = \"1.0.228\"\n",
    )
    .expect("write Cargo.lock");
    std::fs::write(
        root.join("src/lib.rs"),
        "use serde::Serialize;\npub struct Receipt;\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "serde Serialize",
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
    super::assert_graph_turbo_request_contract(&payload);
    let nodes = payload["graph"]["nodes"].as_array().expect("nodes");
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("dependency")
                && node["value"].as_str() == Some("serde")
                && node["confidence"].as_str() == Some("exact")
        }),
        "{payload}"
    );
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("dependency-version")
                && node["value"].as_str() == Some("serde@1.0.228")
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
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_graph_request_uses_typescript_manifest_dependency_versions() {
    let root = temp_project_root("search-pipe-typescript-dependency-topology");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("package.json"),
        r#"{"dependencies":{"react":"18.2.0"},"devDependencies":{"vite":"5.0.0"}}"#,
    )
    .expect("write package.json");
    std::fs::write(
        root.join("src/index.ts"),
        "import React from 'react';\nexport const App = React.Fragment;\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "ts-harness", &marker);
    write_activation(&root, &[provider("typescript", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "typescript",
            "search",
            "pipe",
            "react",
            "--view",
            "graph-turbo-request",
            ".",
        ])
        .output()
        .expect("run asp typescript search pipe graph request");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: Value = serde_json::from_slice(&output.stdout).expect("graph request json");
    super::assert_graph_turbo_request_contract(&payload);
    assert_manifest_dependency_version(&payload, "react", "18.2.0");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_graph_request_uses_python_manifest_dependency_versions() {
    let root = temp_project_root("search-pipe-python-dependency-topology");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("pyproject.toml"),
        "[project]\nname = \"dep-topology-fixture\"\nversion = \"0.1.0\"\ndependencies = [\"requests>=2.31\"]\n",
    )
    .expect("write pyproject.toml");
    std::fs::write(
        root.join("src/main.py"),
        "import requests\nSESSION = requests.Session()\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "py-harness", &marker);
    write_activation(&root, &[provider("python", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "python",
            "search",
            "pipe",
            "requests",
            "--view",
            "graph-turbo-request",
            ".",
        ])
        .output()
        .expect("run asp python search pipe graph request");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: Value = serde_json::from_slice(&output.stdout).expect("graph request json");
    super::assert_graph_turbo_request_contract(&payload);
    assert_manifest_dependency_version(&payload, "requests", ">=2.31");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_graph_request_uses_julia_manifest_dependency_versions() {
    let root = temp_project_root("search-pipe-julia-dependency-topology");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("Project.toml"),
        "[deps]\nDataFrames = \"a93c6f00-e57d-5684-b7b6-d8193f3e46c0\"\n",
    )
    .expect("write Project.toml");
    std::fs::write(
        root.join("Manifest.toml"),
        "[[deps.DataFrames]]\nuuid = \"a93c6f00-e57d-5684-b7b6-d8193f3e46c0\"\nversion = \"1.6.1\"\n",
    )
    .expect("write Manifest.toml");
    std::fs::write(
        root.join("src/main.jl"),
        "using DataFrames\nconst TABLE = DataFrame()\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "asp-julia-harness", &marker);
    write_activation(&root, &[provider("julia", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "julia",
            "search",
            "pipe",
            "DataFrames",
            "--view",
            "graph-turbo-request",
            ".",
        ])
        .output()
        .expect("run asp julia search pipe graph request");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: Value = serde_json::from_slice(&output.stdout).expect("graph request json");
    super::assert_graph_turbo_request_contract(&payload);
    assert_manifest_dependency_version(&payload, "DataFrames", "1.6.1");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_graph_request_uses_gerbil_manifest_dependencies() {
    let root = temp_project_root("search-pipe-gerbil-dependency-topology");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("gerbil.pkg"),
        "(package: dep-topology-fixture\n depend: (\"git.cons.io/mighty-gerbils/gerbil-poo\"))\n",
    )
    .expect("write gerbil.pkg");
    std::fs::write(
        root.join("src/main.ss"),
        ";;; -*- Gerbil -*-\n(import :std/sugar)\n(export run)\n(def (run) 'ok)\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "gerbil-scheme-harness", &marker);
    write_activation(&root, &[provider("gerbil-scheme", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "gerbil-scheme",
            "search",
            "pipe",
            "gerbil-poo",
            "--view",
            "graph-turbo-request",
            ".",
        ])
        .output()
        .expect("run asp gerbil search pipe graph request");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: Value = serde_json::from_slice(&output.stdout).expect("graph request json");
    super::assert_graph_turbo_request_contract(&payload);
    assert_manifest_dependency(&payload, "git.cons.io/mighty-gerbils/gerbil-poo");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_graph_request_includes_language_neutral_project_topology() {
    let root = temp_project_root("search-pipe-project-topology");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::create_dir_all(root.join("languages/gerbil-scheme-language-project-harness/src"))
        .expect("create submodule path");
    std::fs::write(
        root.join(".gitmodules"),
        "[submodule \"languages/gerbil-scheme-language-project-harness\"]\n\
         \tpath = languages/gerbil-scheme-language-project-harness\n\
         \turl = https://example.invalid/gerbil.git\n",
    )
    .expect("write .gitmodules");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"project-topology-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write Cargo.toml");
    std::fs::write(root.join("src/lib.rs"), "pub struct TopologyReceipt;\n").expect("write source");
    std::fs::write(
        root.join("languages/gerbil-scheme-language-project-harness/src/lib.rs"),
        "pub struct SubmoduleTopologyReceipt;\n",
    )
    .expect("write submodule source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "SubmoduleTopologyReceipt",
            "--view",
            "graph-turbo-request",
            ".",
        ])
        .output()
        .expect("run asp rust search pipe topology graph request");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: Value = serde_json::from_slice(&output.stdout).expect("graph request json");
    super::assert_graph_turbo_request_contract(&payload);
    assert!(
        payload["surfaces"]
            .as_array()
            .expect("surfaces")
            .iter()
            .any(|surface| surface.as_str() == Some("topology")),
        "{payload}"
    );
    let nodes = payload["graph"]["nodes"].as_array().expect("nodes");
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("workspace")
                && node["role"].as_str() == Some("root")
                && node["value"].as_str() == Some(".")
        }),
        "{payload}"
    );
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("provider-root")
                && node["role"].as_str() == Some("language-root")
                && node["fields"]["languageId"].as_str() == Some("rust")
        }),
        "{payload}"
    );
    let submodule_id = "submodule:languages/gerbil-scheme-language-project-harness";
    assert!(
        nodes.iter().any(|node| {
            node["id"].as_str() == Some(submodule_id)
                && node["kind"].as_str() == Some("submodule")
                && node["role"].as_str() == Some("workspace-member")
                && node["value"].as_str()
                    == Some("languages/gerbil-scheme-language-project-harness")
        }),
        "{payload}"
    );
    let edges = payload["graph"]["edges"].as_array().expect("edges");
    let owner_id = "owner:languages/gerbil-scheme-language-project-harness/src/lib.rs";
    assert!(
        edges.iter().any(|edge| {
            edge["relation"].as_str() == Some("has_provider_root")
                && edge["source"].as_str() == Some("workspace:.")
        }),
        "{payload}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge["relation"].as_str() == Some("has_submodule")
                && edge["source"].as_str() == Some("workspace:.")
                && edge["target"].as_str() == Some(submodule_id)
        }),
        "{payload}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge["relation"].as_str() == Some("contains")
                && edge["source"].as_str() == Some(submodule_id)
                && edge["target"].as_str() == Some(owner_id)
        }),
        "{payload}"
    );
    let _ = std::fs::remove_dir_all(root);
}

fn assert_manifest_dependency_version(payload: &Value, dependency: &str, version: &str) {
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

fn assert_manifest_dependency(payload: &Value, dependency: &str) {
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
