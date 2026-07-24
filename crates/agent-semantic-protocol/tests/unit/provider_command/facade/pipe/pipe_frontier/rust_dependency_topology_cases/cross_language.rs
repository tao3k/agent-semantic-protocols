use crate::provider_command::facade::pipe::assert_graph_turbo_request_contract;
use crate::provider_command::facade::pipe::pipe_frontier::rust_dependency_topology::support::{
    assert_manifest_dependency, assert_manifest_dependency_version,
};
use crate::provider_command::support;

#[test]
fn search_pipe_graph_request_uses_typescript_manifest_dependency_versions() {
    let root = support::temp_project_root("search-pipe-typescript-dependency-topology");
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
    support::write_marker_provider(&bin_dir, "ts-harness", &marker);
    support::write_activation(&root, &[support::provider("typescript", Vec::new())]);
    let output = support::asp_command(&root)
        .env("PATH", support::prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "typescript",
            "search",
            "pipe",
            "react|version",
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
    assert_graph_turbo_request_contract(&payload);
    assert_manifest_dependency_version(&payload, "react", "18.2.0");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_graph_request_uses_python_manifest_dependency_versions() {
    let root = support::temp_project_root("search-pipe-python-dependency-topology");
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
    support::write_marker_provider(&bin_dir, "py-harness", &marker);
    support::write_activation(&root, &[support::provider("python", Vec::new())]);

    let output = support::asp_command(&root)
        .env("PATH", support::prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "python",
            "search",
            "pipe",
            "requests|version",
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
    assert_graph_turbo_request_contract(&payload);
    assert_manifest_dependency_version(&payload, "requests", ">=2.31");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_graph_request_uses_julia_manifest_dependency_versions() {
    let root = support::temp_project_root("search-pipe-julia-dependency-topology");
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
    support::write_marker_provider(&bin_dir, "asp-julia-harness", &marker);
    support::write_activation(&root, &[support::provider("julia", Vec::new())]);

    let output = support::asp_command(&root)
        .env("PATH", support::prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "julia",
            "search",
            "pipe",
            "DataFrames|version",
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
    assert_graph_turbo_request_contract(&payload);
    assert_manifest_dependency_version(&payload, "DataFrames", "1.6.1");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_graph_request_uses_gerbil_manifest_dependencies() {
    let root = support::temp_project_root("search-pipe-gerbil-dependency-topology");
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
    support::write_marker_provider(&bin_dir, "gslph", &marker);
    support::write_activation(&root, &[support::provider("gerbil-scheme", Vec::new())]);

    let output = support::asp_command(&root)
        .env("PATH", support::prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "gerbil-scheme",
            "search",
            "pipe",
            "gerbil-poo|dependency",
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
    assert_graph_turbo_request_contract(&payload);
    assert_manifest_dependency(&payload, "git.cons.io/mighty-gerbils/gerbil-poo");
    let _ = std::fs::remove_dir_all(root);
}
use serde_json::Value;
