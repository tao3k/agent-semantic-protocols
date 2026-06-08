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
        stdout.contains(
            "pipeExpr=prime|pipe(term='HookDecision ClientReceipt')|S1.query-selector conditional=metadata-only"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "pipeProjections=graph-frontier,S1,nextCommand,pipeCommands,conditionalActions"
        ),
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
        !stdout.contains("frontierActions=R1.reasoning("),
        "{stdout}"
    );
    assert!(
        stdout.contains("frontierActions=S1.selector(selector=src/lib.rs:")
            && stdout.contains(",owner=src/lib.rs,symbol=")
            && stdout.contains(")!query-selector"),
        "{stdout}"
    );
    assert!(
        stdout.contains("S1=>asp rust query --selector src/lib.rs:")
            && stdout.contains(" --workspace . --code"),
        "{stdout}"
    );
    let pipe_commands_line = stdout
        .lines()
        .find(|line| line.starts_with("pipeCommands="))
        .expect("pipeCommands line");
    assert!(
        pipe_commands_line.contains("S1=>asp rust query"),
        "{stdout}"
    );
    assert!(!pipe_commands_line.contains("S2=>"), "{stdout}");
    assert!(
        !pipe_commands_line.contains("R1=>asp rust search reasoning"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "conditionalActions=metadata-only selector=hidden run-if-primary-insufficient:"
        ),
        "{stdout}"
    );
    let conditional_actions_line = stdout
        .lines()
        .find(|line| line.starts_with("conditionalActions="))
        .expect("conditionalActions line");
    assert!(
        !conditional_actions_line.contains("asp rust query"),
        "{stdout}"
    );
    assert!(
        !conditional_actions_line.contains("asp rust search reasoning"),
        "{stdout}"
    );
    assert!(
        !conditional_actions_line.contains(".reasoning("),
        "{stdout}"
    );
    assert!(!conditional_actions_line.contains(".selector("), "{stdout}");
    assert!(
        !conditional_actions_line.contains("selector=src/lib.rs:"),
        "{stdout}"
    );
    assert!(
        stdout.contains("recommendedNext=S1.query-selector"),
        "{stdout}"
    );
    assert!(
        stdout.contains("no-duplicate-selector=true no-context-widening=true"),
        "{stdout}"
    );
    assert!(
        stdout.contains("nextCommand=asp rust query --selector src/lib.rs:")
            && stdout.contains(" --workspace . --code"),
        "{stdout}"
    );
    let first_selector_action = stdout
        .find("frontierActions=S1.selector(")
        .expect("S1 frontier action");
    assert!(
        !stdout[first_selector_action..].contains("frontierActions=R1.reasoning("),
        "seeds view should not render reasoning frontier actions: {stdout}"
    );
    let first_selector_command = stdout.find("S1=>asp rust query").expect("S1 command");
    assert!(
        !stdout[first_selector_command..].contains("R1=>asp rust search reasoning"),
        "seeds view should not render runnable reasoning branch commands: {stdout}"
    );
    assert!(
        !stdout.contains("<selector>") && !stdout.contains("<owner-path>"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "avoid=repeat-prime,repeat-pipe,query-rewrite-pipe,reasoning-before-selector,read-all-selectors-by-default,guide-after-selector,repeat-fzf"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "post-projection-owner-search,post-projection-fzf,post-projection-treesitter-guide"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("duplicate-selector,context-widening,raw-read"),
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
fn search_pipe_plan_preserves_search_scope_in_repeat_commands() {
    let root = temp_project_root("search-pipe-package-root-commands");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    let package_root = root.join("languages/rust-harness");
    std::fs::create_dir_all(package_root.join("src")).expect("create package src");
    std::fs::write(
        package_root.join("src/lib.rs"),
        "pub struct HookDecision;\npub struct ClientReceipt;\n",
    )
    .expect("write package source");
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
            "--view",
            "seeds",
            "languages/rust-harness",
        ])
        .output()
        .expect("run asp rust search pipe for package root");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("context=>asp rust search prime --view seeds languages/rust-harness"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "pipe=>asp rust search pipe 'HookDecision ClientReceipt' --view seeds languages/rust-harness"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("nextCommand=asp rust query --selector languages/rust-harness/src/lib.rs:")
            && stdout.contains(" --workspace . --code"),
        "{stdout}"
    );
    assert!(
        stdout.contains("S1=>asp rust query --selector languages/rust-harness/src/lib.rs:")
            && stdout.contains(" --workspace . --code"),
        "{stdout}"
    );
    assert!(!marker.exists(), "search pipe should not spawn provider");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_plan_uses_scope_root_for_provider_local_selectors() {
    let root = temp_project_root("search-pipe-provider-local-selector-root");
    let bin_dir = root.join(".bin");
    let package_root = root.join("languages/rust-harness");
    std::fs::create_dir_all(package_root.join("src")).expect("create package src");
    std::fs::write(
        package_root.join("src/lib.rs"),
        "pub struct Scalar;\npub struct Snapshot {\n    pub scalars: Vec<Scalar>,\n}\n",
    )
    .expect("write package source");
    write_stdout_stderr_provider(
        &bin_dir,
        "rs-harness",
        r#"{"nodes":[{"id":"field:src/lib.rs-scalars-3","kind":"field","role":"struct-field","value":"scalars: Vec<Scalar>","action":"code","path":"src/lib.rs","ownerPath":"src/lib.rs","symbol":"scalars","startLine":3,"endLine":3,"locator":"src/lib.rs:1:4","matchText":"Snapshot::scalars: Vec<Scalar>","fields":{"containerName":"Snapshot","fieldName":"scalars","typeValue":"Vec<Scalar>","collectionKind":"Vec","contextLocator":"src/lib.rs:1:4"}},{"id":"type:src/lib.rs-scalars-vec-3","kind":"type","role":"field-type","value":"Vec<Scalar>","action":"evidence","path":"src/lib.rs","ownerPath":"src/lib.rs","symbol":"Vec","startLine":3,"endLine":3,"locator":"src/lib.rs:3:3","fields":{"fieldName":"scalars","typeValue":"Vec<Scalar>","collectionKind":"Vec"}},{"id":"collection:vec","kind":"collection","role":"family","value":"Vec","action":"evidence","symbol":"Vec","fields":{"collectionKind":"Vec"}}],"edges":[{"source":"query:vec-collection-fields","target":"field:src/lib.rs-scalars-3","relation":"matches"},{"source":"field:src/lib.rs-scalars-3","target":"type:src/lib.rs-scalars-vec-3","relation":"has_type"},{"source":"field:src/lib.rs-scalars-3","target":"collection:vec","relation":"collection_of"}]}"#,
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
            "seeds",
            "languages/rust-harness",
        ])
        .output()
        .expect("run asp rust search pipe with provider facts");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("F=field:struct-field(scalars: Vec<Scalar>)@src/lib.rs:1:4!code"),
        "{stdout}"
    );
    assert!(
        stdout.contains("type:field-type(Vec<Scalar>)@src/lib.rs:3:3!evidence"),
        "{stdout}"
    );
    assert!(
        stdout.contains("C=collection:family(Vec)!evidence"),
        "{stdout}"
    );
    assert!(stdout.contains("has_type"), "{stdout}");
    assert!(stdout.contains("collection_of"), "{stdout}");
    assert!(
        stdout.contains("queryCoverage=matched=vec,collection,fields missing=-"),
        "{stdout}"
    );
    assert!(
        stdout.contains("frontierActions=S1.selector(selector=src/lib.rs:1:4,owner=src/lib.rs,symbol=scalars,source=F)!query-selector"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "nextCommand=asp rust query --selector src/lib.rs:1:4 --workspace languages/rust-harness --code"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "S1=>asp rust query --selector src/lib.rs:1:4 --workspace languages/rust-harness --code"
        ),
        "{stdout}"
    );
    for debug_prefix in [
        "scores=", "paths=", "trace=", "explain=", "cache=", "metrics=",
    ] {
        assert!(
            !stdout.lines().any(|line| line.starts_with(debug_prefix)),
            "{stdout}"
        );
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_injects_read_loop_memory_into_graph_turbo_request_and_suppresses_seen_selector() {
    let root = temp_project_root("search-pipe-read-memory");
    let bin_dir = root.join(".bin");
    let package_root = root.join("languages/rust-harness");
    std::fs::create_dir_all(package_root.join("src")).expect("create package src");
    std::fs::write(
        package_root.join("src/lib.rs"),
        "pub struct Scalar;\npub struct Snapshot {\n    pub scalars: Vec<Scalar>,\n}\npub struct Other {\n    pub queued: Vec<Scalar>,\n}\n",
    )
    .expect("write package source");
    let memory_dir = root.join(".cache/agent-semantic-protocol");
    std::fs::create_dir_all(&memory_dir).expect("create read memory dir");
    std::fs::write(
        memory_dir.join("read-loop-memory.json"),
        r#"{"schemaId":"agent.semantic-protocols.read-loop-memory","schemaVersion":"1","projectRoot":".","seenSelectors":["languages/rust-harness/src/lib.rs:1:15"]}"#,
    )
    .expect("write read memory");
    write_stdout_stderr_provider(
        &bin_dir,
        "rs-harness",
        r#"{"nodes":[{"id":"field:src/lib.rs-scalars-3","kind":"field","role":"struct-field","value":"scalars: Vec<Scalar>","action":"code","path":"src/lib.rs","ownerPath":"src/lib.rs","symbol":"scalars","startLine":3,"endLine":3,"locator":"src/lib.rs:1:4","matchText":"Snapshot::scalars: Vec<Scalar>","fields":{"containerName":"Snapshot","fieldName":"scalars","typeValue":"Vec<Scalar>","collectionKind":"Vec","contextLocator":"src/lib.rs:1:4"}},{"id":"field:src/lib.rs-queued-6","kind":"field","role":"struct-field","value":"queued: Vec<Scalar>","action":"code","path":"src/lib.rs","ownerPath":"src/lib.rs","symbol":"queued","startLine":6,"endLine":6,"locator":"src/lib.rs:5:8","matchText":"Other::queued: Vec<Scalar>","fields":{"containerName":"Other","fieldName":"queued","typeValue":"Vec<Scalar>","collectionKind":"Vec","contextLocator":"src/lib.rs:5:8"}},{"id":"collection:vec","kind":"collection","role":"family","value":"Vec","action":"evidence","symbol":"Vec","fields":{"collectionKind":"Vec"}}],"edges":[{"source":"field:src/lib.rs-scalars-3","target":"collection:vec","relation":"collection_of"},{"source":"field:src/lib.rs-queued-6","target":"collection:vec","relation":"collection_of"}]}"#,
        "",
    );
    write_activation(&root, &[provider("rust", Vec::new())]);

    let request_output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "Vec collection fields",
            "--view",
            "graph-turbo-request",
            "languages/rust-harness",
        ])
        .output()
        .expect("run asp rust search pipe graph request with read memory");

    assert!(
        request_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&request_output.stderr)
    );
    let payload: Value =
        serde_json::from_slice(&request_output.stdout).expect("graph request json");
    assert_eq!(
        payload["readMemory"]["seenSelectors"][0], "languages/rust-harness/src/lib.rs:1:15",
        "{payload}"
    );

    let seeds_output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "Vec collection fields",
            "--view",
            "seeds",
            "languages/rust-harness",
        ])
        .output()
        .expect("run asp rust search pipe seeds with read memory");

    assert!(
        seeds_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&seeds_output.stderr)
    );
    let stdout = String::from_utf8(seeds_output.stdout).expect("stdout");
    assert!(
        stdout.contains("avoid=") && stdout.contains("seen-selector"),
        "{stdout}"
    );
    assert!(
        !stdout.contains(
            "frontierActions=S1.selector(selector=languages/rust-harness/src/lib.rs:1:15"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "frontierActions=S1.selector(selector=languages/rust-harness/src/lib.rs:1:18"
        ),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_graph_turbo_request_keeps_late_query_token_candidates() {
    let root = temp_project_root("search-pipe-graph-candidate-limit");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    let mut source = String::new();
    for index in 0..300 {
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
        r#"{"nodes":[{"id":"field:src/lib.rs-scalars-304","kind":"field","role":"struct-field","value":"scalars: Vec<Scalar>","action":"code","path":"src/lib.rs","ownerPath":"src/lib.rs","symbol":"scalars","startLine":304,"endLine":304,"locator":"src/lib.rs:304:304","matchText":"Snapshot::scalars: Vec<Scalar>","fields":{"containerName":"Snapshot","fieldName":"scalars","typeName":"Vec","typeValue":"Vec<Scalar>","typeArgs":"Scalar","collectionKind":"Vec"}},{"id":"type:src/lib.rs-scalars-vec-304","kind":"type","role":"field-type","value":"Vec<Scalar>","action":"evidence","path":"src/lib.rs","ownerPath":"src/lib.rs","symbol":"Vec","startLine":304,"endLine":304,"locator":"src/lib.rs:304:304","fields":{"fieldName":"scalars","typeName":"Vec","typeValue":"Vec<Scalar>","typeArgs":"Scalar","collectionKind":"Vec"}},{"id":"collection:vec","kind":"collection","role":"family","value":"Vec","action":"evidence","symbol":"Vec","fields":{"collectionKind":"Vec"}}],"edges":[{"source":"field:src/lib.rs-scalars-304","target":"type:src/lib.rs-scalars-vec-304","relation":"has_type"},{"source":"field:src/lib.rs-scalars-304","target":"collection:vec","relation":"collection_of"}]}"#,
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

#[test]
fn search_pipe_graph_turbo_request_accepts_python_provider_semantic_facts() {
    let root = temp_project_root("search-pipe-python-provider-facts");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/model.py"),
        "class Bag:\n    items: list[str]\n",
    )
    .expect("write python source");
    write_stdout_stderr_provider(
        &bin_dir,
        "py-harness",
        r#"{"nodes":[{"id":"field:src/model.py-bag-items-2","kind":"field","role":"class-field","value":"items: list[str]","action":"code","path":"src/model.py","ownerPath":"src/model.py","symbol":"items","startLine":2,"endLine":2,"locator":"src/model.py:2:2","matchText":"Bag.items: list[str]","fields":{"containerName":"Bag","fieldName":"items","typeValue":"list[str]","elementShape":"collection","collectionKind":"list","contextLocator":"src/model.py:1:2"}}],"edges":[]}"#,
        "",
    );
    write_activation(&root, &[provider("python", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "python",
            "search",
            "pipe",
            "list collection fields",
            "--view",
            "graph-turbo-request",
            ".",
        ])
        .output()
        .expect("run asp python search pipe graph request");

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
            node["kind"].as_str() == Some("field")
                && node["symbol"].as_str() == Some("items")
                && node["fields"]["collectionKind"].as_str() == Some("list")
        }),
        "{payload}"
    );
    let _ = std::fs::remove_dir_all(root);
}
