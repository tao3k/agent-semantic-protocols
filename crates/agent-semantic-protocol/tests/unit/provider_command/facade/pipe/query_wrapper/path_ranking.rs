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
    assert!(
        stdout.contains("rankedEvidence=H1:compiler/moduleNameResolver.ts"),
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
            "A1=owner-items(owner=src/graph_turbo_candidate_ranking.rs,query=graph|turbo|candidate|ranking)"
        ),
        "{stdout}"
    );
    let owner_items_line = stdout
        .lines()
        .find(|line| line.starts_with("A1=owner-items("))
        .expect("owner-items action line");
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
            "A1=owner-items(owner=src/search_pipe_graph_turbo_owner_rank.rs,query=ranked|sandtable|report|chain|ranking|graph)"
        ),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}
