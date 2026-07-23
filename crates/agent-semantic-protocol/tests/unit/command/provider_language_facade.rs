use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace root")
}

#[test]
fn structural_item_code_query_routes_to_provider_backend() {
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "typescript",
            "query",
            "--selector",
            "typescript://languages/typescript-lang-project-harness/src/cli/semantic-search/item-query.ts#item/function/renderOwnerItemQuery",
            "--workspace",
            ".",
            "--code",
        ])
        .current_dir(workspace_root())
        .output()
        .expect("run asp structural item query");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr={stderr}");

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(
        stdout.starts_with("function renderOwnerItemQuery("),
        "stdout={stdout}"
    );
    assert!(
        !stdout.contains("export interface SemanticQueryPacket"),
        "structural item query leaked owner file: {stdout}"
    );
}

#[test]
fn exact_structural_selector_does_not_require_a_term() {
    let selector = "typescript://languages/typescript-lang-project-harness/src/cli/semantic-search/item-query.ts#item/function/renderOwnerItemQuery";
    let mut failures = Vec::new();

    for projection in [None, Some("--names-only")] {
        let mut args = vec![
            "typescript",
            "query",
            "--selector",
            selector,
            "--workspace",
            ".",
        ];
        if let Some(projection) = projection {
            args.push(projection);
        }

        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .args(args)
            .current_dir(workspace_root())
            .output()
            .expect("run exact structural selector query");

        let stderr = String::from_utf8_lossy(&output.stderr);
        if !output.status.success() || stderr.contains("query requires at least one --term") {
            failures.push(format!(
                "projection={projection:?} status={} stderr={stderr}",
                output.status
            ));
        }
    }

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
