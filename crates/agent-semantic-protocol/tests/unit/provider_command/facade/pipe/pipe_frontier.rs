use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation,
    write_marker_provider, write_stdout_stderr_provider,
};

use super::assert_graph_turbo_request_contract;
use serde_json::Value;

#[test]
fn search_pipe_is_asp_owned_and_renders_generated_candidates_without_provider_spawn() {
    let root = temp_project_root("search-pipe-facade");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub struct HookDecision;\npub struct ClientReceipt;\nfn unrelated() {}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "HookDecision ClientReceipt",
            "--pipe",
            "items,tests",
            "--view",
            "seeds",
            ".",
        ])
        .output()
        .expect("run asp rust search pipe");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.starts_with("[graph-frontier]"), "{stdout}");
    assert!(
        stdout.contains("Q=query:term(HookDecision ClientReceipt)!fzf"),
        "{stdout}"
    );
    assert!(
        stdout.contains("I=item:symbol(clientreceipt)@src/lib.rs:2:2!syntax"),
        "{stdout}"
    );
    assert!(
        stdout.contains("I2=item:symbol(hookdecision)@src/lib.rs:1:1!syntax"),
        "{stdout}"
    );
    assert!(
        stdout.contains("frontier=Q.fzf,I.syntax,H.code,I2.syntax,H2.code"),
        "{stdout}"
    );
    assert!(
        stdout.contains("pipePlan=query-pipeline alg=asp-search-pipe-v1"),
        "{stdout}"
    );
    assert!(
        stdout.contains("pipeExpr=prime |> search(term='HookDecision ClientReceipt') |> rank(profile=owner-query) |> filter(path=source-preferred) |> project(frontierActions,pipeCommands,selectors) |> choose(branch=bounded,max=3,repeat=false,rewrite=false)"),
        "{stdout}"
    );
    assert!(
        stdout.contains("pipeProjections=graph-frontier,frontierActions,pipeCommands"),
        "{stdout}"
    );
    assert!(
        stdout
            .contains("pipeChoice=bounded-fanout maxBranches=3 repeat=false owner=asp-graph-turbo"),
        "{stdout}"
    );
    assert!(
        stdout.contains("pipeExecution=each-branch-at-most-once"),
        "{stdout}"
    );
    assert!(!stdout.contains("R4=>"), "{stdout}");
    assert!(!stdout.contains("frontierActions=R4."), "{stdout}");
    assert!(
        stdout.contains("pipeStages=search-prime,search-pipe,query-selector,search-reasoning"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "selectorPolicy=run-first reason=exact-selector-present before=search-reasoning"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("context=>asp rust search prime --view seeds ."),
        "{stdout}"
    );
    assert!(
        stdout.contains("pipe=>asp rust search pipe 'HookDecision ClientReceipt' --view seeds ."),
        "{stdout}"
    );
    assert!(
        stdout.contains("frontierActions=R1.reasoning(owner=src/lib.rs,querySource=search-pipe)!search-reasoning"),
        "{stdout}"
    );
    assert!(
        stdout.contains("frontierActions=S1.selector(selector=src/lib.rs:")
            && stdout.contains(",owner=src/lib.rs,symbol=")
            && stdout.contains(")!query-selector"),
        "{stdout}"
    );
    assert!(
        stdout.contains("R1=>asp rust search reasoning owner-query --owner src/lib.rs --query 'HookDecision ClientReceipt' --view seeds ."),
        "{stdout}"
    );
    assert!(
        stdout.contains("S1=>asp rust query --selector src/lib.rs:")
            && stdout.contains(" --code ."),
        "{stdout}"
    );
    assert!(
        stdout.contains("recommendedNext=S1.query-selector"),
        "{stdout}"
    );
    assert!(
        stdout.contains("nextCommand=asp rust query --selector src/lib.rs:")
            && stdout.contains(" --code ."),
        "{stdout}"
    );
    let first_selector_action = stdout
        .find("frontierActions=S1.selector(")
        .expect("S1 frontier action");
    let first_reasoning_action = stdout
        .find("frontierActions=R1.reasoning(")
        .expect("R1 frontier action");
    assert!(
        first_selector_action < first_reasoning_action,
        "selector action should be rendered before reasoning action: {stdout}"
    );
    let first_selector_command = stdout.find("S1=>asp rust query").expect("S1 command");
    let first_reasoning_command = stdout
        .find("R1=>asp rust search reasoning")
        .expect("R1 command");
    assert!(
        first_selector_command < first_reasoning_command,
        "selector command should be rendered before reasoning command: {stdout}"
    );
    assert!(
        !stdout.contains("<selector>") && !stdout.contains("<owner-path>"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "avoid=repeat-prime,repeat-pipe,query-rewrite-pipe,reasoning-before-selector,repeat-fzf"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "post-projection-owner-search,post-projection-fzf,post-projection-treesitter-guide"
        ),
        "{stdout}"
    );
    assert!(!marker.exists(), "search pipe should not spawn provider");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_commands_view_points_to_search_suggest() {
    let root = temp_project_root("search-pipe-commands-facade");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "HookDecision",
            "--pipe",
            "items,tests",
            "--view",
            "commands",
            ".",
        ])
        .output()
        .expect("run asp rust search pipe commands");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("search pipe --view commands moved to search suggest --view commands"),
        "{stderr}"
    );
    assert!(
        !marker.exists(),
        "commands migration error should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_graph_turbo_request_keeps_late_query_token_candidates() {
    let root = temp_project_root("search-pipe-graph-candidate-limit");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    let mut source = String::new();
    for index in 0..80 {
        source.push_str(&format!(
            "fn early_vec_{index}() {{ let _ = Vec::<u8>::new(); }}\n"
        ));
    }
    source.push_str("fn later_collection_fields() { let _ = \"collection fields\"; }\n");
    source
        .push_str("pub struct Scalar;\npub struct Snapshot {\n    pub scalars: Vec<Scalar>,\n}\n");
    std::fs::write(root.join("src/lib.rs"), source).expect("write source");
    write_stdout_stderr_provider(
        &bin_dir,
        "rs-harness",
        r#"{"nodes":[{"id":"field:src/lib.rs-scalars-84","kind":"field","role":"struct-field","value":"scalars: Vec<Scalar>","action":"code","path":"src/lib.rs","ownerPath":"src/lib.rs","symbol":"scalars","startLine":84,"endLine":84,"locator":"src/lib.rs:84:84","matchText":"Snapshot::scalars: Vec<Scalar>","fields":{"containerName":"Snapshot","fieldName":"scalars","typeName":"Vec","typeValue":"Vec<Scalar>","typeArgs":"Scalar","collectionKind":"Vec"}},{"id":"type:src/lib.rs-scalars-vec-84","kind":"type","role":"field-type","value":"Vec<Scalar>","action":"evidence","path":"src/lib.rs","ownerPath":"src/lib.rs","symbol":"Vec","startLine":84,"endLine":84,"locator":"src/lib.rs:84:84","fields":{"fieldName":"scalars","typeName":"Vec","typeValue":"Vec<Scalar>","typeArgs":"Scalar","collectionKind":"Vec"}},{"id":"collection:vec","kind":"collection","role":"family","value":"Vec","action":"evidence","symbol":"Vec","fields":{"collectionKind":"Vec"}}],"edges":[{"source":"field:src/lib.rs-scalars-84","target":"type:src/lib.rs-scalars-vec-84","relation":"has_type"},{"source":"field:src/lib.rs-scalars-84","target":"collection:vec","relation":"collection_of"}]}"#,
        "",
    );
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "Vec collection fields",
            "--view",
            "graph-turbo-request",
            ".",
        ])
        .output()
        .expect("run asp rust search pipe graph request");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: Value = serde_json::from_slice(&output.stdout).expect("graph request json");
    assert_graph_turbo_request_contract(&payload);
    let nodes = payload["graph"]["nodes"].as_array().expect("nodes");
    assert!(
        nodes.iter().any(|node| {
            node["symbol"].as_str() == Some("collection")
                && node["matchText"]
                    .as_str()
                    .is_some_and(|text| text.contains("collection fields"))
        }),
        "{payload}"
    );
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("field")
                && node["symbol"].as_str() == Some("scalars")
                && node["fields"]["typeValue"].as_str() == Some("Vec<Scalar>")
        }),
        "{payload}"
    );
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("type")
                && node["fields"]["fieldName"].as_str() == Some("scalars")
                && node["fields"]["collectionKind"].as_str() == Some("Vec")
        }),
        "{payload}"
    );
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("collection") && node["symbol"].as_str() == Some("Vec")
        }),
        "{payload}"
    );
    let edges = payload["graph"]["edges"].as_array().expect("edges");
    assert!(
        edges
            .iter()
            .any(|edge| edge["relation"].as_str() == Some("has_type")),
        "{payload}"
    );
    assert!(
        edges
            .iter()
            .any(|edge| edge["relation"].as_str() == Some("collection_of")),
        "{payload}"
    );
    let _ = std::fs::remove_dir_all(root);
}
