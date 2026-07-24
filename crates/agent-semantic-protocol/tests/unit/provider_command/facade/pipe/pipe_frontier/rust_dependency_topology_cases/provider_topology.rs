use crate::provider_command::facade::pipe::pipe_frontier::rust_dependency_topology::support::assert_provider_topology_marker;
use crate::provider_command::support;
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
    use crate::provider_command::facade::pipe::assert_graph_turbo_request_contract;

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
        let root = support::temp_project_root(&format!("search-pipe-{language}-project-topology"));
        let bin_dir = root.join(".bin");
        let marker = root.join("provider-called");
        std::fs::create_dir_all(root.join("src")).expect("create src");
        std::fs::write(root.join(project_marker), manifest_text).expect("write project marker");
        std::fs::write(root.join(source_path), source_text).expect("write source");
        support::write_marker_provider(&bin_dir, binary, &marker);
        support::write_activation(&root, &[support::provider(language, Vec::new())]);

        let output = support::asp_command(&root)
            .env("PATH", support::prepend_path(&bin_dir))
            .env("PRJ_CACHE_HOME", root.join(".cache"))
            .args([
                language,
                "search",
                "pipe",
                "TopologyReceipt|topology",
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
        assert_graph_turbo_request_contract(&payload);
        assert_provider_topology_marker(&payload, language, project_marker, dependency_marker);
        let _ = std::fs::remove_dir_all(root);
    }
}
use serde_json::Value;
