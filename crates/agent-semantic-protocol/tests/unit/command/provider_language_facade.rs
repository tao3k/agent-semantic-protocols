use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace root")
}

#[test]
fn structural_item_source_query_fails_closed_without_live_provider() {
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "typescript",
            "query",
            "--selector",
            "typescript://languages/typescript-lang-project-harness/src/cli/semantic-search/item-query.ts#item/function/renderOwnerItemQuery",
            "--workspace",
            ".",
            "--projection",
            "source",
        ])
        .current_dir(workspace_root())
        .output()
        .expect("run asp structural item query");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "stderr={stderr}");
    assert!(
        stderr.contains("reasonKind=active-workspace-generation-required"),
        "stderr={stderr}"
    );
    assert!(stderr.contains("provider-missing"), "stderr={stderr}");
}

#[test]
fn exact_structural_selector_does_not_require_a_term() {
    let selector = "typescript://languages/typescript-lang-project-harness/src/cli/semantic-search/item-query.ts#item/function/renderOwnerItemQuery";
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "typescript",
            "query",
            "--selector",
            selector,
            "--workspace",
            ".",
            "--projection",
            "source",
        ])
        .current_dir(workspace_root())
        .output()
        .expect("run exact structural selector query");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "stderr={stderr}");
    assert!(
        !stderr.contains("query requires at least one --term"),
        "stderr={stderr}"
    );
    assert!(
        stderr.contains("reasonKind=active-workspace-generation-required"),
        "stderr={stderr}"
    );
}

#[test]
fn removed_exact_query_flags_are_rejected_by_cli_admission() {
    for removed_flag in ["--code", "--names-only"] {
        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .args([
                "typescript",
                "query",
                "--selector",
                "typescript://languages/typescript-lang-project-harness/src/cli/semantic-search/item-query.ts#item/function/renderOwnerItemQuery",
                "--workspace",
                ".",
                removed_flag,
            ])
            .current_dir(workspace_root())
            .output()
            .expect("run removed exact-query flag");

        assert!(!output.status.success(), "{removed_flag} was admitted");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("unexpected argument"),
            "removed flag did not use ordinary CLI rejection: flag={removed_flag} stderr={stderr}"
        );
        assert!(
            !stderr.contains("legacy") && !stderr.contains("unsupported"),
            "removed flag leaked compatibility guidance: flag={removed_flag} stderr={stderr}"
        );
    }
}

#[test]
fn document_exact_selector_crosses_the_language_neutral_owner_boundary() {
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "org",
            "query",
            "--selector",
            "org://docs/missing.org#item/heading/missing",
            "--workspace",
            ".",
            "--projection",
            "content",
        ])
        .current_dir(workspace_root())
        .output()
        .expect("run org exact structural selector query");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("provider-owned structural query is missing an exact owner path"),
        "org selector was rejected by a language-hardcoded owner boundary: {stderr}"
    );
    assert!(
        !stderr.contains("query requires at least one --term"),
        "org exact selector was misrouted into lexical query: {stderr}"
    );
}
