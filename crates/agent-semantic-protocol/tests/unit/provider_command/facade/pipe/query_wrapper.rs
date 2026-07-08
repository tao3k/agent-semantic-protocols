mod graph_request;
mod low_cohesion;
mod native_batching;
mod org_scope;
mod path_ranking;

use crate::provider_command::support::{
    asp_command, assert_compact_search_action_contract, make_executable, prepend_path, provider,
    temp_project_root, write_activation, write_marker_provider,
};

use super::assert_graph_turbo_request_contract;

fn refresh_source_index(root: &std::path::Path) {
    let output = asp_command(root)
        .args(["cache", "source-index", "rebuild"])
        .output()
        .expect("run asp cache source-index rebuild");
    assert!(
        output.status.success(),
        "source-index rebuild failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

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
        fd_stdout.contains(
            "usage: asp fd -query <owner-or-path-term-a|term-b|term-c> [-query <second-clause>] [--workspace <root>] [scope...]"
        ),
        "{fd_stdout}"
    );
    assert!(
        fd_stdout.contains("repeatable LLM-generated query clauses"),
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
        rg_stdout.contains(
            "usage: asp rg -query <content-or-error-term-a|term-b|term-c> [-query <second-clause>] [--workspace <root>] [scope...]"
        ),
        "{rg_stdout}"
    );
    assert!(
        rg_stdout.contains("repeatable LLM-generated query clauses"),
        "{rg_stdout}"
    );
    assert!(!fd_stdout.contains("natural-intent"), "{fd_stdout}");
    assert!(!rg_stdout.contains("natural-intent"), "{rg_stdout}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_rg_query_uses_source_index_before_query_overlay() {
    let root = temp_project_root("asp-rg-query-wrapper-source-index");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"asp-rg-query-wrapper-source-index\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write rust package anchor");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn source_index_fixture() {}\npub fn unrelated() {}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);
    refresh_source_index(&root);
    let _ = std::fs::remove_file(&marker);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["rg", "-query", "source_index_fixture", "--workspace", "."])
        .output()
        .expect("run asp rg -query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert_compact_search_action_contract(&stdout);
    assert!(stdout.starts_with("[search-rg]"), "{stdout}");
    assert!(stdout.contains("source=source-index"), "{stdout}");
    assert!(stdout.contains("sourceTrace=sourceIndex:used"), "{stdout}");
    assert!(stdout.contains("query-overlay:skipped"), "{stdout}");
    assert!(stdout.contains("packages=src/lib.rs"), "{stdout}");
    assert!(
        stdout.contains("nextCommand=asp fd -query source_index_fixture --workspace ."),
        "{stdout}"
    );
    assert!(
        !stdout.contains("sourceTrace=query-overlay:used"),
        "{stdout}"
    );
    assert!(
        !marker.exists(),
        "source-index fast path should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_fd_query_uses_source_index_before_query_overlay() {
    let root = temp_project_root("asp-fd-query-wrapper-source-index");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"asp-fd-query-wrapper-source-index\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write rust package anchor");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn source_index_fixture() {}\npub fn unrelated() {}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);
    refresh_source_index(&root);
    let _ = std::fs::remove_file(&marker);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["fd", "-query", "source_index_fixture", "--workspace", "."])
        .output()
        .expect("run asp fd -query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert_compact_search_action_contract(&stdout);
    assert!(stdout.starts_with("[search-fd]"), "{stdout}");
    assert!(stdout.contains("source=source-index"), "{stdout}");
    assert!(stdout.contains("sourceTrace=sourceIndex:used"), "{stdout}");
    assert!(stdout.contains("query-overlay:skipped"), "{stdout}");
    assert!(stdout.contains("src/lib.rs"), "{stdout}");
    assert!(
        !stdout.contains("sourceTrace=query-overlay:used"),
        "{stdout}"
    );
    assert!(
        !marker.exists(),
        "source-index fast path should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_rg_query_marks_single_broad_or_as_recall_probe() {
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
            "--workspace",
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
    assert_compact_search_action_contract(&stdout);
    assert!(stdout.starts_with("[search-rg]"), "{stdout}");
    assert!(stdout.contains("querySet=3"), "{stdout}");
    assert!(stdout.contains("query=Fiber|Queue|Runtime"), "{stdout}");
    assert!(
        stdout.contains("queryPack=clauses=1 quality=low"),
        "{stdout}"
    );
    assert!(
        stdout.contains("queryClauses=C1='Fiber|Queue|Runtime'"),
        "{stdout}"
    );
    assert!(stdout.contains("terms=fiber,queue,runtime"), "{stdout}");
    assert!(stdout.contains("scopeQuality=low"), "{stdout}");
    assert!(
        stdout.contains("clauseCoverage=C1 matched=fiber,queue,runtime missing=-"),
        "{stdout}"
    );
    assert!(
        stdout.contains("nativeArgs=pass-through count=2"),
        "{stdout}"
    );
    assert!(!stdout.contains("rankedEvidence="), "{stdout}");
    assert!(!stdout.contains("evidenceFrontier="), "{stdout}");
    assert!(!stdout.contains("actionFrontier="), "{stdout}");
    assert!(!stdout.contains("recommendedNext="), "{stdout}");
    assert!(
        stdout.contains("nextCommand=asp fd -query 'Fiber|Queue|Runtime' --workspace ."),
        "{stdout}"
    );
    assert!(
        stdout.contains("nextClasses=fd-query,scoped-rg-query,owner-items"),
        "{stdout}"
    );
    assert!(
        stdout.contains("avoid=repeat-flat-rg,workspace-wide-rg,manual-window-scan,raw-read"),
        "{stdout}"
    );
    assert!(!stdout.contains("[graph-frontier]"), "{stdout}");
    assert!(!stdout.contains("nextClasses=query-selector"), "{stdout}");
    assert!(!stdout.contains("|>"), "{stdout}");
    assert!(!stdout.contains("lit("), "{stdout}");
    assert!(!stdout.contains("any("), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_rg_query_keeps_repeated_query_clauses_separate() {
    let root = temp_project_root("asp-rg-query-wrapper-clauses");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("package.json"),
        r#"{"name":"query-wrapper-fixture"}"#,
    )
    .expect("write package json");
    std::fs::write(
        root.join("src/effect.ts"),
        "export const Fiber = {};\nexport const Queue = {};\nconst staleCache = 'refresh turso cache';\n",
    )
    .expect("write source");
    std::fs::write(
        root.join("notes.ts"),
        "const stale = 'outside scoped owner';\n",
    )
    .expect("write outside source");

    let output = asp_command(&root)
        .args([
            "rg",
            "-query",
            "Fiber|Queue",
            "-query",
            "stale|refresh|turso|cache",
            "--workspace",
            "src",
        ])
        .output()
        .expect("run asp rg repeated -query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert_compact_search_action_contract(&stdout);
    assert!(stdout.starts_with("[search-rg]"), "{stdout}");
    assert!(stdout.contains("querySet=6"), "{stdout}");
    assert!(
        stdout.contains("query=Fiber|Queue + stale|refresh|turso|cache"),
        "{stdout}"
    );
    assert!(
        stdout.contains("queryPack=clauses=2 quality=high reason=ok"),
        "{stdout}"
    );
    assert!(
        stdout.contains("queryClauses=C1='Fiber|Queue';C2='stale|refresh|turso|cache'"),
        "{stdout}"
    );
    assert!(stdout.contains("scopeQuality=high"), "{stdout}");
    assert!(
        stdout.contains("clauseCoverage=C1 matched=fiber,queue missing=-"),
        "{stdout}"
    );
    assert!(
        stdout.contains("clauseCoverage=C2 matched=stale,refresh,turso,cache missing=-"),
        "{stdout}"
    );
    assert!(
        stdout.contains("packageCohesion=high packages=effect.ts"),
        "{stdout}"
    );
    assert!(!stdout.contains("actionFrontier="), "{stdout}");
    assert!(!stdout.contains("recommendedNext="), "{stdout}");
    assert!(!stdout.contains("rankedEvidence="), "{stdout}");
    assert!(!stdout.contains("evidenceFrontier="), "{stdout}");
    assert!(!stdout.contains("[graph-frontier]"), "{stdout}");
    assert_eq!(
        stdout
            .lines()
            .filter(|line| line.starts_with("avoid="))
            .count(),
        1,
        "{stdout}"
    );
    assert!(
        stdout.contains("nextClasses=owner-items,query-selector,fd-query"),
        "{stdout}"
    );
    assert!(!stdout.contains("notes.ts"), "{stdout}");
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
            "--workspace",
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
    assert_eq!(payload["source"], "query-overlay");
    assert_eq!(
        payload["candidateSources"],
        serde_json::json!(["query-overlay"])
    );
    let nodes = payload["graph"]["nodes"].as_array().expect("nodes");
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("item")
                && node["path"].as_str() == Some("src/internal/FiberRuntime.ts")
                && node["source"].as_str() == Some("query-overlay")
        }),
        "{payload}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_rg_query_graph_request_uses_query_overlay_source() {
    let root = temp_project_root("asp-rg-query-wrapper-graph-request");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("package.json"),
        r#"{"name":"query-wrapper-fixture"}"#,
    )
    .expect("write package json");
    std::fs::write(
        root.join("src/runtime.ts"),
        "export const FiberRuntime = 'runtime';\n",
    )
    .expect("write source");

    let output = asp_command(&root)
        .args([
            "rg",
            "-query",
            "FiberRuntime",
            "--view",
            "graph-turbo-request",
            ".",
        ])
        .output()
        .expect("run asp rg -query graph request");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("graph request json");
    assert_graph_turbo_request_contract(&payload);
    assert_eq!(payload["surface"], "search-rg");
    assert_eq!(payload["source"], "query-overlay");
    assert_eq!(
        payload["candidateSources"],
        serde_json::json!(["query-overlay"])
    );
    let actions = payload["actionFrontier"]
        .as_array()
        .expect("actionFrontier");
    assert_eq!(actions[0]["kind"], serde_json::json!("fd-query"));
    assert_eq!(
        actions[0]["capabilityId"],
        serde_json::json!("fd"),
        "{payload}"
    );
    assert!(
        actions[0].get("command").is_none()
            && actions[0].get("argv").is_none()
            && actions[0]["fields"].get("command").is_none()
            && actions[0]["fields"].get("argv").is_none(),
        "{payload}"
    );
    let nodes = payload["graph"]["nodes"].as_array().expect("nodes");
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("item")
                && node["path"].as_str() == Some("src/runtime.ts")
                && node["source"].as_str() == Some("query-overlay")
                && node["symbol"].as_str() == Some("fiberruntime")
                && node["matchText"]
                    .as_str()
                    .is_some_and(|text| text.contains("FiberRuntime"))
        }),
        "{payload}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_rg_query_prefers_runtime_bin_wrapper() {
    let root = temp_project_root("asp-rg-query-runtime-wrapper");
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
    let rg = runtime_bin.join("rg");
    std::fs::write(
        &rg,
        "#!/bin/sh\nprintf 'src/runtime.ts:1:export const WrappedRuntime = 1;\\n'\n",
    )
    .expect("write rg wrapper");
    make_executable(&rg);

    let output = asp_command(&root)
        .env("ASP_RUNTIME_BIN_DIR", &runtime_bin)
        .args([
            "rg",
            "-query",
            "OnlyWrapperCanEmit",
            "--view",
            "graph-turbo-request",
            ".",
        ])
        .output()
        .expect("run asp rg -query graph request through runtime wrapper");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("graph request json");
    assert_graph_turbo_request_contract(&payload);
    assert_eq!(
        payload["sourceTrace"][0]["fields"]["backend"],
        serde_json::json!("rg")
    );
    assert_eq!(
        payload["sourceTrace"][0]["fields"]["candidateBasis"],
        serde_json::json!("source-lines")
    );
    assert_eq!(
        payload["sourceTrace"][0]["fields"]["sourceSearchPasses"],
        serde_json::json!(1)
    );
    let nodes = payload["graph"]["nodes"].as_array().expect("nodes");
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("item")
                && node["source"].as_str() == Some("query-overlay")
                && node["matchText"]
                    .as_str()
                    .is_some_and(|text| text.contains("WrappedRuntime"))
        }),
        "{payload}"
    );
    let seeds_output = asp_command(&root)
        .env("ASP_RUNTIME_BIN_DIR", &runtime_bin)
        .args(["rg", "-query", "WrappedRuntime", "--view", "seeds", "."])
        .output()
        .expect("run asp rg -query seeds through runtime wrapper");
    assert!(
        seeds_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&seeds_output.stderr)
    );
    let stdout = String::from_utf8(seeds_output.stdout).expect("stdout");
    assert!(
        stdout.contains("sourceTrace=query-overlay:used["),
        "{stdout}"
    );
    assert!(stdout.contains("backend=rg"), "{stdout}");
    assert!(stdout.contains("candidateBasis=source-lines"), "{stdout}");
    assert!(stdout.contains("inputCandidates=1"), "{stdout}");
    assert!(stdout.contains("selectedCandidates=1"), "{stdout}");
    assert!(stdout.contains("sourceSearchPasses=1"), "{stdout}");
    assert!(stdout.contains("elapsedMs="), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_fd_query_uses_runtime_exa_when_fd_is_unavailable() {
    let root = temp_project_root("asp-fd-query-runtime-exa-wrapper");
    let runtime_bin = root.join(".cache/agent-semantic-protocol/runtime/bin");
    std::fs::create_dir_all(root.join("src/internal")).expect("create src");
    std::fs::create_dir_all(&runtime_bin).expect("create runtime bin");
    std::fs::write(
        root.join("package.json"),
        r#"{"name":"query-wrapper-fixture"}"#,
    )
    .expect("write package json");
    std::fs::write(
        root.join("src/internal/FromExaRuntime.ts"),
        "export const LocalOnly = 1;\n",
    )
    .expect("write source");
    std::fs::write(
        root.join("src/internal/OtherRuntime.ts"),
        "export const y = 1;\n",
    )
    .expect("write unrelated source");
    let trace = root.join("exa-trace.txt");
    let exa = runtime_bin.join("exa");
    std::fs::write(
        &exa,
        "#!/bin/sh\nprintf 'exa\\n' >> \"$EXA_TRACE_FILE\"\nprintf 'src/internal/FromExaRuntime.ts\\nsrc/internal/OtherRuntime.ts\\n'\n",
    )
    .expect("write exa wrapper");
    make_executable(&exa);

    let output = asp_command(&root)
        .env("ASP_RUNTIME_BIN_DIR", &runtime_bin)
        .env("EXA_TRACE_FILE", &trace)
        .env("PATH", &runtime_bin)
        .args([
            "fd",
            "-query",
            "FromExaRuntime|MissingTerm",
            "--view",
            "graph-turbo-request",
            ".",
        ])
        .output()
        .expect("run asp fd -query graph request through runtime exa wrapper");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("graph request json");
    assert_graph_turbo_request_contract(&payload);
    let trace_fields = &payload["sourceTrace"][0]["fields"];
    assert_eq!(trace_fields["backend"], serde_json::json!("fd+exa"));
    assert_eq!(trace_fields["fallbackFrom"], serde_json::json!("fd"));
    assert_eq!(trace_fields["candidateBasis"], serde_json::json!("paths"));
    assert_eq!(trace_fields["sourceSearchPasses"], serde_json::json!(1));
    assert_eq!(trace_fields["fileListPasses"], serde_json::json!(1));
    assert_eq!(trace_fields["inputCandidates"], serde_json::json!(2));
    assert_eq!(trace_fields["selectedCandidates"], serde_json::json!(1));
    let nodes = payload["graph"]["nodes"].as_array().expect("nodes");
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("item")
                && node["path"].as_str() == Some("src/internal/FromExaRuntime.ts")
                && node["source"].as_str() == Some("query-overlay")
        }),
        "{payload}"
    );
    assert!(
        nodes
            .iter()
            .all(|node| node["path"].as_str() != Some("src/internal/OtherRuntime.ts")),
        "{payload}"
    );
    let exa_trace = std::fs::read_to_string(&trace).expect("read exa trace");
    assert_eq!(exa_trace.lines().count(), 1, "{exa_trace}");
    let _ = std::fs::remove_dir_all(root);
}
