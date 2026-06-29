use crate::provider_command::support::{
    asp_command, assert_compact_search_action_contract, home_local_bin, make_executable,
    prepend_path, provider, provider_with_dependency_topology, temp_project_root, write_activation,
    write_marker_provider,
};
use serde_json::Value;

#[test]
fn search_pipe_graph_request_uses_rust_manifest_dependency_versions() {
    let root = temp_project_root("search-pipe-rust-dependency-topology");
    let bin_dir = home_local_bin(&root);
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
    write_dependency_topology_provider(
        &bin_dir,
        "rs-harness",
        &marker,
        "serde",
        "1.0.228",
        "Cargo.toml",
    );
    write_activation(
        &root,
        &[provider_with_dependency_topology("rust", Vec::new())],
    );

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
    assert!(
        marker.exists(),
        "graph request should use provider-owned dependency topology"
    );
    assert_eq!(
        payload["cache"]["dependencySeed"]["topology"].as_str(),
        Some("provider-owned"),
        "{payload}"
    );
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
fn search_pipe_seeds_promotes_matching_dependency_route() {
    let root = temp_project_root("search-pipe-seeds-dependency-action");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"dep-action-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nserde = \"1\"\n",
    )
    .expect("write Cargo.toml");
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
        .args(["rust", "search", "pipe", "serde", "--view", "seeds", "."])
        .output()
        .expect("run asp rust search pipe seeds");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_compact_search_action_contract(&stdout);
    assert!(
        stdout.contains("nextCommand=asp rust search deps serde --workspace . --view seeds"),
        "{stdout}"
    );
    assert!(!stdout.contains("actionFrontier="), "{stdout}");
    assert!(!stdout.contains("recommendedNext="), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_does_not_promote_dependency_route_from_natural_tree_word() {
    let root = temp_project_root("search-pipe-seeds-natural-tree-not-dependency");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"tree-word-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\ntree-sitter = \"0.26\"\n",
    )
    .expect("write Cargo.toml");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn reasoning_tree_frontier() {}\n",
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
            "reasoning tree seed action frontier",
            "--view",
            "seeds",
            ".",
        ])
        .output()
        .expect("run asp rust search pipe seeds");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("search-deps(dependency=tree-sitter"),
        "{stdout}"
    );
    assert!(!stdout.contains("recommendedNext="), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_does_not_promote_dependency_route_from_meta_audit_query() {
    let root = temp_project_root("search-pipe-seeds-meta-audit-not-dependency");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"meta-audit-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\ntree-sitter = \"0.26\"\ntokio = \"1\"\n",
    )
    .expect("write Cargo.toml");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn reasoning_tree_frontier() {}\n",
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
            "audit evidence state reasoning tree expected tests conclusions next plan dependency seed line selector",
            "--view",
            "seeds",
            ".",
        ])
        .output()
        .expect("run asp rust search pipe seeds");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("search-deps(dependency="), "{stdout}");
    assert!(!stdout.contains("recommendedNext="), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_does_not_promote_dependency_route_from_negative_meta_query_with_literal() {
    let root = temp_project_root("search-pipe-seeds-meta-literal-not-dependency");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"meta-literal-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\ntree-sitter = \"0.26\"\ntokio = \"1\"\n",
    )
    .expect("write Cargo.toml");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn reasoning_tree_frontier() {}\n",
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
            "audit meta query dependency reasoning tree should not route to tree-sitter search-deps",
            "--view",
            "seeds",
            ".",
        ])
        .output()
        .expect("run asp rust search pipe seeds");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("search-deps(dependency="), "{stdout}");
    assert!(!stdout.contains("recommendedNext="), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}

fn write_dependency_topology_provider(
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
    make_executable(&path);
}

#[test]
fn search_pipe_graph_request_reuses_cached_manifest_dependency_seed() {
    let root = temp_project_root("search-pipe-rust-dependency-seed-cache");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    let cache_home = root.join(".cache");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"dep-seed-cache-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nserde = \"1\"\n",
    )
    .expect("write Cargo.toml");
    std::fs::write(
        root.join("src/lib.rs"),
        "use serde::Serialize;\npub struct Receipt;\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let first =
        rust_dependency_graph_request_payload(&root, &bin_dir, &cache_home, "serde Serialize");
    assert_eq!(
        first["cache"]["dependencySeed"]["status"].as_str(),
        Some("miss"),
        "{first}"
    );
    assert_manifest_dependency(&first, "serde");

    let second =
        rust_dependency_graph_request_payload(&root, &bin_dir, &cache_home, "serde Serialize");
    assert_eq!(
        second["cache"]["dependencySeed"]["status"].as_str(),
        Some("hit"),
        "{second}"
    );
    assert_manifest_dependency(&second, "serde");

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
    write_marker_provider(&bin_dir, "gslph", &marker);
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
fn search_pipe_graph_request_uses_provider_declared_project_topology_markers() {
    let cases = [
        (
            "typescript",
            "ts-harness",
            "package.json",
            "package.json",
            "src/index.ts",
            "export class TopologyReceipt {}\n",
            r#"{"name":"topology-fixture","dependencies":{"react":"18.2.0"}}"#,
        ),
        (
            "python",
            "py-harness",
            "pyproject.toml",
            "pyproject.toml",
            "src/main.py",
            "class TopologyReceipt:\n    pass\n",
            "[project]\nname = \"topology-fixture\"\ndependencies = [\"requests>=2.31\"]\n",
        ),
        (
            "julia",
            "asp-julia-harness",
            "Project.toml",
            "Project.toml",
            "src/main.jl",
            "struct TopologyReceipt end\n",
            "[deps]\nDataFrames = \"a93c6f00-e57d-5684-b7b6-d8193f3e46c0\"\n",
        ),
        (
            "gerbil-scheme",
            "gslph",
            "gerbil.pkg",
            "gerbil.pkg",
            "src/main.ss",
            ";;; TopologyReceipt\n(def TopologyReceipt 'ok)\n",
            "(package: topology-fixture\n depend: (\"git.cons.io/example/pkg\"))\n",
        ),
    ];

    for (
        language,
        binary,
        project_marker,
        dependency_marker,
        source_path,
        source_text,
        manifest_text,
    ) in cases
    {
        let root = temp_project_root(&format!("search-pipe-{language}-project-topology"));
        let bin_dir = root.join(".bin");
        let marker = root.join("provider-called");
        std::fs::create_dir_all(root.join("src")).expect("create src");
        std::fs::write(root.join(project_marker), manifest_text).expect("write project marker");
        std::fs::write(root.join(source_path), source_text).expect("write source");
        write_marker_provider(&bin_dir, binary, &marker);
        write_activation(&root, &[provider(language, Vec::new())]);

        let output = asp_command(&root)
            .env("PATH", prepend_path(&bin_dir))
            .env("PRJ_CACHE_HOME", root.join(".cache"))
            .args([
                language,
                "search",
                "pipe",
                "TopologyReceipt",
                "--view",
                "graph-turbo-request",
                ".",
            ])
            .output()
            .expect("run asp search pipe topology graph request");

        assert!(
            output.status.success(),
            "language={language} stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let payload: Value = serde_json::from_slice(&output.stdout).expect("graph request json");
        super::assert_graph_turbo_request_contract(&payload);
        assert_provider_topology_marker(&payload, language, project_marker, dependency_marker);
        let _ = std::fs::remove_dir_all(root);
    }
}

#[test]
fn search_pipe_graph_request_includes_language_neutral_project_topology() {
    let root = temp_project_root("search-pipe-project-topology");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::create_dir_all(root.join("zz-harnesses/gerbil-scheme-language-project-harness/src"))
        .expect("create submodule path");
    std::fs::write(
        root.join(".gitmodules"),
        "[submodule \"zz-harnesses/gerbil-scheme-language-project-harness\"]\n\
         \tpath = zz-harnesses/gerbil-scheme-language-project-harness\n\
         \turl = https://example.invalid/gerbil.git\n",
    )
    .expect("write .gitmodules");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"project-topology-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write Cargo.toml");
    std::fs::write(
        root.join("zz-harnesses/gerbil-scheme-language-project-harness/Cargo.toml"),
        "[package]\nname = \"submodule-topology-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write submodule Cargo.toml");
    std::fs::write(root.join("src/lib.rs"), "pub struct TopologyReceipt;\n").expect("write source");
    std::fs::write(
        root.join("src/submodule_topology.rs"),
        "pub struct SubmoduleTopologyReceipt;\n",
    )
    .expect("write root topology source");
    std::fs::write(
        root.join("zz-harnesses/gerbil-scheme-language-project-harness/src/lib.rs"),
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
    assert_eq!(
        payload["fields"]["topologyRank"].as_str(),
        Some("submodule-membership"),
        "{payload}"
    );
    assert_eq!(
        payload["summary"]["topologyRankSubmodules"].as_u64(),
        Some(1),
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
    let submodule_id = "submodule:zz-harnesses/gerbil-scheme-language-project-harness";
    assert!(
        nodes.iter().any(|node| {
            node["id"].as_str() == Some(submodule_id)
                && node["kind"].as_str() == Some("submodule")
                && node["role"].as_str() == Some("workspace-member")
                && node["value"].as_str()
                    == Some("zz-harnesses/gerbil-scheme-language-project-harness")
        }),
        "{payload}"
    );
    let root_project_id = "language-project:rust-.";
    let root_config_id = "project-marker:rust-cargo.toml";
    let root_dependency_marker_id = "dependency-marker:rust-cargo.toml";
    let submodule_project_id =
        "language-project:rust-zz-harnesses/gerbil-scheme-language-project-harness";
    let submodule_config_id =
        "project-marker:rust-zz-harnesses/gerbil-scheme-language-project-harness/cargo.toml";
    assert!(
        nodes.iter().any(|node| {
            node["id"].as_str() == Some(root_project_id)
                && node["kind"].as_str() == Some("language-project")
                && node["role"].as_str() == Some("project-root")
                && node["fields"]["languageId"].as_str() == Some("rust")
                && node["fields"]["projectMarker"].as_str() == Some("Cargo.toml")
        }),
        "{payload}"
    );
    assert!(
        nodes.iter().any(|node| {
            node["id"].as_str() == Some(submodule_project_id)
                && node["kind"].as_str() == Some("language-project")
                && node["role"].as_str() == Some("project-root")
                && node["path"].as_str()
                    == Some("zz-harnesses/gerbil-scheme-language-project-harness")
                && node["fields"]["projectMarker"].as_str()
                    == Some("zz-harnesses/gerbil-scheme-language-project-harness/Cargo.toml")
        }),
        "{payload}"
    );
    assert!(
        nodes.iter().any(|node| {
            node["id"].as_str() == Some(root_config_id)
                && node["kind"].as_str() == Some("project-marker")
                && node["role"].as_str() == Some("project-marker")
        }),
        "{payload}"
    );
    assert!(
        nodes.iter().any(|node| {
            node["id"].as_str() == Some(root_dependency_marker_id)
                && node["kind"].as_str() == Some("dependency-marker")
                && node["role"].as_str() == Some("dependency-source")
        }),
        "{payload}"
    );
    assert!(
        nodes.iter().any(|node| {
            node["id"].as_str() == Some(submodule_config_id)
                && node["kind"].as_str() == Some("project-marker")
                && node["role"].as_str() == Some("project-marker")
        }),
        "{payload}"
    );
    let edges = payload["graph"]["edges"].as_array().expect("edges");
    let root_owner_id = "owner:src/submodule_topology.rs";
    let owner_id = "owner:zz-harnesses/gerbil-scheme-language-project-harness/src/lib.rs";
    let root_owner_index = nodes
        .iter()
        .position(|node| node["id"].as_str() == Some(root_owner_id))
        .expect("root owner node");
    let submodule_owner_index = nodes
        .iter()
        .position(|node| node["id"].as_str() == Some(owner_id))
        .expect("submodule owner node");
    assert!(
        submodule_owner_index < root_owner_index,
        "submodule topology owner should outrank matching root owner: {payload}"
    );

    let disabled_output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .env("ASP_GRAPH_TURBO_ABLATION_VARIANT", "no-topology-membership")
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
        .expect("run asp rust search pipe topology graph request without topology ranking");

    assert!(
        disabled_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&disabled_output.stderr)
    );
    let disabled_payload: Value =
        serde_json::from_slice(&disabled_output.stdout).expect("disabled graph request json");
    super::assert_graph_turbo_request_contract(&disabled_payload);
    assert_eq!(
        disabled_payload["queryAdjustmentPolicy"]["topologyMembership"].as_bool(),
        Some(false),
        "{disabled_payload}"
    );
    assert!(
        disabled_payload["fields"]["topologyRank"].is_null(),
        "disabled topology membership must not claim topology rank signal: {disabled_payload}"
    );
    assert_eq!(
        disabled_payload["summary"]["topologyRankSubmodules"].as_u64(),
        Some(1),
        "{disabled_payload}"
    );
    let disabled_nodes = disabled_payload["graph"]["nodes"]
        .as_array()
        .expect("nodes");
    let disabled_root_owner_index = disabled_nodes
        .iter()
        .position(|node| node["id"].as_str() == Some(root_owner_id))
        .expect("disabled root owner node");
    let disabled_submodule_owner_index = disabled_nodes
        .iter()
        .position(|node| node["id"].as_str() == Some(owner_id))
        .expect("disabled submodule owner node");
    assert!(
        disabled_root_owner_index < disabled_submodule_owner_index,
        "disabling topology membership should expose the baseline owner order: {disabled_payload}"
    );
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
            edge["relation"].as_str() == Some("has_language_project")
                && edge["source"].as_str() == Some("provider-root:rust-.")
                && edge["target"].as_str() == Some(root_project_id)
        }),
        "{payload}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge["relation"].as_str() == Some("declared_by")
                && edge["source"].as_str() == Some(root_project_id)
                && edge["target"].as_str() == Some(root_config_id)
        }),
        "{payload}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge["relation"].as_str() == Some("uses_dependency_marker")
                && edge["source"].as_str() == Some(root_project_id)
                && edge["target"].as_str() == Some(root_dependency_marker_id)
        }),
        "{payload}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge["relation"].as_str() == Some("contains_project")
                && edge["source"].as_str() == Some(submodule_id)
                && edge["target"].as_str() == Some(submodule_project_id)
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

fn rust_dependency_graph_request_payload(
    root: &std::path::Path,
    bin_dir: &std::path::Path,
    cache_home: &std::path::Path,
    query: &str,
) -> Value {
    let output = asp_command(root)
        .env("PATH", prepend_path(bin_dir))
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
    super::assert_graph_turbo_request_contract(&payload);
    payload
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

fn assert_provider_topology_marker(
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
