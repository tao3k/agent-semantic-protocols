use crate::provider_command::support::{asp_command, temp_project_root};

#[test]
fn asp_fd_query_ranks_path_candidates_by_normalized_query_axis_coverage() {
    let root = temp_project_root("asp-fd-query-wrapper-axis-coverage");
    std::fs::create_dir_all(root.join("src/compiler/transformers/module"))
        .expect("create transformer module dir");
    std::fs::write(
        root.join("package.json"),
        r#"{"name":"query-wrapper-fixture"}"#,
    )
    .expect("write package json");
    std::fs::write(
        root.join("src/compiler/transformers/module/module.ts"),
        "export const moduleTransform = 1;\n",
    )
    .expect("write broad module source");
    std::fs::write(
        root.join("src/compiler/moduleNameResolver.ts"),
        "export const resolveModuleName = 1;\n",
    )
    .expect("write resolver source");

    let output = asp_command(&root)
        .args([
            "fd",
            "-query",
            "TypeScript|compiler|resolveModuleName|module|tests",
            "src",
        ])
        .output()
        .expect("run asp fd -query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("terms=typescript,compiler,resolvemodulename,module,tests"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "ownerCandidates=compiler/moduleNameResolver.ts,compiler/transformers/module/module.ts"
        ),
        "{stdout}"
    );
    assert!(!stdout.contains("rankedEvidence="), "{stdout}");
    assert!(!stdout.contains("evidenceFrontier="), "{stdout}");
    assert!(!stdout.contains("actionFrontier="), "{stdout}");
    assert!(
        stdout.contains(
            "nextCommand=asp typescript search owner compiler/moduleNameResolver.ts items"
        ),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_fd_query_ranks_query_dense_owner_before_low_coverage_path() {
    let root = temp_project_root("asp-fd-query-wrapper-query-axis-rank");
    std::fs::create_dir_all(root.join("semantic_sandtable")).expect("create sandtable dir");
    std::fs::write(root.join("semantic_sandtable/overview.py"), "VALUE = 1\n")
        .expect("write low coverage source");
    std::fs::write(
        root.join("semantic_sandtable/_discovery_steps_common.py"),
        "VALUE = 2\n",
    )
    .expect("write discovery source");
    std::fs::write(
        root.join("semantic_sandtable/large_library_intent_matrix_support.py"),
        "VALUE = 3\n",
    )
    .expect("write matrix source");
    std::fs::write(
        root.join("semantic_sandtable/test_large_library_report_chain.py"),
        "VALUE = 4\n",
    )
    .expect("write report chain source");

    let output = asp_command(&root)
        .args([
            "fd",
            "-query",
            "topology|membership|ablation|sandtable|runner|report|chain|controlled|full|disabled|request|policy",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp fd -query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    let owner_line = stdout
        .lines()
        .find(|line| line.starts_with("ownerCandidates="))
        .expect("owner candidates line");
    assert!(
        owner_line
            .starts_with("ownerCandidates=semantic_sandtable/test_large_library_report_chain.py"),
        "{stdout}"
    );
    assert!(!stdout.contains("rankedEvidence="), "{stdout}");
    assert!(!stdout.contains("evidenceFrontier="), "{stdout}");
    assert!(!stdout.contains("actionFrontier="), "{stdout}");
    assert!(
        stdout.contains("--workspace 'semantic_sandtable/test_large_library_report_chain.py'"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_fd_query_does_not_fan_out_package_path_when_axis_is_already_covered() {
    let root = temp_project_root("asp-fd-query-wrapper-package-axis-covered");
    let package_root = root.join("packages/python/asp_graph_turbo/src/asp_graph_turbo");
    std::fs::create_dir_all(&package_root).expect("create graph turbo package dir");
    std::fs::write(
        package_root.join("benchmark_cli.py"),
        "def benchmark_packet():\n    return None\n",
    )
    .expect("write benchmark source");
    for index in 0..48 {
        std::fs::write(
            package_root.join(format!("unrelated_{index}.py")),
            "VALUE = 1\n",
        )
        .expect("write unrelated source");
    }

    let output = asp_command(&root)
        .args([
            "fd",
            "-query",
            "asp_graph_turbo|benchmark",
            "packages/python/asp_graph_turbo",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp fd package axis query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(!stdout.contains("packagePathAugmented="), "{stdout}");
    assert!(
        stdout.contains("ownerCandidates=src/asp_graph_turbo/benchmark_cli.py"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_fd_query_owner_items_query_uses_selected_owner_axes() {
    let root = temp_project_root("asp-fd-query-wrapper-owner-items-query");
    std::fs::create_dir_all(root.join("src")).expect("create src dir");
    std::fs::write(
        root.join("src/graph_turbo_candidate_ranking.rs"),
        "pub fn ranked_candidate_paths() {}\n",
    )
    .expect("write ranking source");

    let output = asp_command(&root)
        .args([
            "fd",
            "-query",
            "owner|search|frontier|fd|graph|turbo|candidate|ranking",
            ".",
        ])
        .output()
        .expect("run asp fd -query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains(
            "nextCommand=asp rust search owner src/graph_turbo_candidate_ranking.rs items --query 'graph|turbo|candidate|ranking' --workspace . --view seeds"
        ),
        "{stdout}"
    );
    let owner_items_line = stdout
        .lines()
        .find(|line| {
            line.starts_with(
                "nextCommand=asp rust search owner src/graph_turbo_candidate_ranking.rs items",
            )
        })
        .expect("owner-items command line");
    assert!(
        !owner_items_line.contains("owner|search|frontier|fd"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_fd_query_owner_items_query_prefers_semantic_variants_before_path_terms() {
    let root = temp_project_root("asp-fd-query-wrapper-owner-items-variants");
    std::fs::create_dir_all(root.join("src")).expect("create src dir");
    std::fs::write(
        root.join("src/search_pipe_graph_turbo_owner_rank.rs"),
        "pub fn ranked_candidate_paths() {}\n",
    )
    .expect("write owner rank source");

    let output = asp_command(&root)
        .args([
            "fd",
            "-query",
            "owner|sandtable|report|chain|graph|turbo|ranking|search",
            ".",
        ])
        .output()
        .expect("run asp fd -query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains(
            "nextCommand=asp rust search owner src/search_pipe_graph_turbo_owner_rank.rs items --query 'ranked|sandtable|report|chain|ranking|graph' --workspace . --view seeds"
        ),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}
