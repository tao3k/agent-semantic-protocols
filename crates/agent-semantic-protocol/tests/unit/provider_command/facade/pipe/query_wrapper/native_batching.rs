use crate::provider_command::facade::pipe::assert_graph_turbo_request_contract;
use crate::provider_command::support::{asp_command, make_executable, temp_project_root};

#[test]
fn asp_rg_query_batches_terms_into_one_runtime_call_per_root() {
    let root = temp_project_root("asp-rg-query-batched-runtime-wrapper");
    let _ = std::fs::remove_dir_all(&root);
    let runtime_bin = root.join(".cache/agent-semantic-protocol/runtime/bin");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::create_dir_all(&runtime_bin).expect("create runtime bin");
    std::fs::write(
        root.join("package.json"),
        r#"{"name":"query-wrapper-fixture"}"#,
    )
    .expect("write package json");
    std::fs::write(root.join("src/runtime.ts"), "export const LocalOnly = 1;\n")
        .expect("write source");
    let trace = root.join("rg-trace.txt");
    let rg = runtime_bin.join("rg");
    std::fs::write(
        &rg,
        "#!/bin/sh\nprintf 'rg\\n' >> \"$RG_TRACE_FILE\"\nprintf 'src/runtime.ts:1:export const AlphaRuntime = 1;\\n'\nprintf 'src/runtime.ts:2:export const BetaRuntime = 1;\\n'\n",
    )
    .expect("write rg wrapper");
    make_executable(&rg);

    let output = asp_command(&root)
        .env("ASP_RUNTIME_BIN_DIR", &runtime_bin)
        .env("RG_TRACE_FILE", &trace)
        .args([
            "rg",
            "-query",
            "AlphaRuntime|BetaRuntime",
            "--view",
            "graph-turbo-request",
            ".",
        ])
        .output()
        .expect("run asp rg -query graph request through batched runtime wrapper");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("graph request json");
    assert_graph_turbo_request_contract(&payload);
    let trace_fields = &payload["sourceTrace"][0]["fields"];
    assert_eq!(trace_fields["backend"], serde_json::json!("rg"));
    assert_eq!(
        trace_fields["candidateBasis"],
        serde_json::json!("source-lines")
    );
    assert_eq!(trace_fields["sourceSearchPasses"], serde_json::json!(1));
    assert_eq!(trace_fields["selectedCandidates"], serde_json::json!(2));
    let rg_trace = std::fs::read_to_string(&trace).expect("read rg trace");
    assert_eq!(rg_trace.lines().count(), 1, "{rg_trace}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_fd_query_ranks_specific_owner_before_low_coverage_path() {
    let root = temp_project_root("asp-fd-query-coverage-rank");
    let runtime_bin = root.join(".cache/agent-semantic-protocol/runtime/bin");
    std::fs::create_dir_all(root.join("src/semantic_sandtable")).expect("create package src");
    std::fs::create_dir_all(&runtime_bin).expect("create runtime bin");
    std::fs::write(
        root.join("pyproject.toml"),
        "[project]\nname='fd-rank-fixture'\nversion='0.1.0'\n",
    )
    .expect("write pyproject");
    std::fs::write(
        root.join("src/semantic_sandtable/overview.py"),
        "VALUE = 1\n",
    )
    .expect("write low coverage source");
    std::fs::write(
        root.join("src/semantic_sandtable/report_chain.py"),
        "VALUE = 2\n",
    )
    .expect("write report chain source");
    let fd = runtime_bin.join("fd");
    std::fs::write(
        &fd,
        "#!/bin/sh\nprintf 'src/semantic_sandtable/overview.py\\n'\nprintf 'src/semantic_sandtable/report_chain.py\\n'\n",
    )
    .expect("write fd wrapper");
    make_executable(&fd);

    let output = asp_command(&root)
        .env("ASP_RUNTIME_BIN_DIR", &runtime_bin)
        .args([
            "fd",
            "-query",
            "topology|membership|ablation|sandtable|runner|report|chain|controlled|full|disabled|request|policy",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp fd -query through path-ranking wrapper");

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
        owner_line.starts_with(
            "ownerCandidates=src/semantic_sandtable/report_chain.py,src/semantic_sandtable/overview.py"
        ),
        "{stdout}"
    );
    assert!(stdout.contains("backend=fd"), "{stdout}");
    assert!(stdout.contains("candidateBasis=paths"), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}
