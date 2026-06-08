use crate::provider_command::support::{asp_command, temp_project_root};

use super::assert_graph_turbo_request_contract;

#[test]
fn asp_fd_and_rg_query_help_are_public_query_set_surfaces() {
    let root = temp_project_root("asp-query-wrapper-help");

    let fd_output = asp_command(&root)
        .args(["fd", "--help"])
        .output()
        .expect("run asp fd help");
    assert!(
        fd_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&fd_output.stderr)
    );
    let fd_stdout = String::from_utf8(fd_output.stdout).expect("fd stdout");
    assert!(
        fd_stdout.contains("usage: asp fd -query <owner-or-path-term-a|term-b|term-c> [scope...]"),
        "{fd_stdout}"
    );
    assert!(
        fd_stdout.contains("LLM-generated grouped query-set"),
        "{fd_stdout}"
    );

    let rg_output = asp_command(&root)
        .args(["rg", "--help"])
        .output()
        .expect("run asp rg help");
    assert!(
        rg_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&rg_output.stderr)
    );
    let rg_stdout = String::from_utf8(rg_output.stdout).expect("rg stdout");
    assert!(
        rg_stdout
            .contains("usage: asp rg -query <content-or-error-term-a|term-b|term-c> [scope...]"),
        "{rg_stdout}"
    );
    assert!(
        rg_stdout.contains("LLM-generated grouped query-set"),
        "{rg_stdout}"
    );
    assert!(!fd_stdout.contains("natural-intent"), "{fd_stdout}");
    assert!(!rg_stdout.contains("natural-intent"), "{rg_stdout}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_rg_query_renders_grouped_query_frontier() {
    let root = temp_project_root("asp-rg-query-wrapper");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("package.json"),
        r#"{"name":"query-wrapper-fixture"}"#,
    )
    .expect("write package json");
    std::fs::write(
        root.join("src/effect.ts"),
        "export const Fiber = {};\nexport const Queue = {};\nexport const Runtime = {};\n",
    )
    .expect("write source");

    let output = asp_command(&root)
        .args([
            "rg",
            "-query",
            "Fiber|Queue|Runtime",
            ".",
            "--",
            "--glob",
            "*.ts",
        ])
        .output()
        .expect("run asp rg -query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.starts_with("[search-rg]"), "{stdout}");
    assert!(stdout.contains("querySet=3"), "{stdout}");
    assert!(stdout.contains("query=Fiber|Queue|Runtime"), "{stdout}");
    assert!(stdout.contains("terms=fiber,queue,runtime"), "{stdout}");
    assert!(
        stdout.contains("nativeArgs=pass-through count=2"),
        "{stdout}"
    );
    assert!(stdout.contains("[graph-frontier]"), "{stdout}");
    assert!(
        stdout.contains("Q=query:term(Fiber|Queue|Runtime)!fzf"),
        "{stdout}"
    );
    assert!(stdout.contains("frontier="), "{stdout}");
    assert!(
        stdout.contains("nextClasses=query-selector,owner-items,fd-query"),
        "{stdout}"
    );
    assert!(
        stdout.contains("avoid=repeat-rg,manual-window-scan,raw-read"),
        "{stdout}"
    );
    assert!(!stdout.contains("|>"), "{stdout}");
    assert!(!stdout.contains("lit("), "{stdout}");
    assert!(!stdout.contains("any("), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_fd_query_graph_request_carries_surface_and_query_terms() {
    let root = temp_project_root("asp-fd-query-wrapper");
    std::fs::create_dir_all(root.join("src/internal")).expect("create src");
    std::fs::write(
        root.join("package.json"),
        r#"{"name":"query-wrapper-fixture"}"#,
    )
    .expect("write package json");
    std::fs::write(
        root.join("src/internal/FiberRuntime.ts"),
        "export const x = 1;\n",
    )
    .expect("write source");

    let output = asp_command(&root)
        .args([
            "fd",
            "-query",
            "Fiber|Runtime|internal",
            "--view",
            "graph-turbo-request",
            ".",
        ])
        .output()
        .expect("run asp fd -query graph request");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("graph request json");
    assert_graph_turbo_request_contract(&payload);
    assert_eq!(payload["surface"], "search-fd");
    assert_eq!(
        payload["queryTerms"],
        serde_json::json!(["Fiber", "Runtime", "internal"])
    );
    assert_eq!(payload["source"], "finder");
    assert_eq!(payload["candidateSources"], serde_json::json!(["finder"]));
    let nodes = payload["graph"]["nodes"].as_array().expect("nodes");
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("item")
                && node["path"].as_str() == Some("src/internal/FiberRuntime.ts")
                && node["source"].as_str() == Some("fd-query")
        }),
        "{payload}"
    );
    let _ = std::fs::remove_dir_all(root);
}
