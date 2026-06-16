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
