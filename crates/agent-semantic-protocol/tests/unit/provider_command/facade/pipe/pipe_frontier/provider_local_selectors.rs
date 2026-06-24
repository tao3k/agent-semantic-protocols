use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation,
    write_provider_bin_config, write_semantic_facts_provider,
};

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
    write_semantic_facts_provider(
        &bin_dir,
        "rs-harness",
        r#"{"nodes":[{"id":"field:src/lib.rs-scalars-3","kind":"field","role":"struct-field","value":"scalars: Vec<Scalar>","action":"code","path":"src/lib.rs","ownerPath":"src/lib.rs","symbol":"scalars","startLine":3,"endLine":3,"locator":"src/lib.rs:1:4","matchText":"Snapshot::scalars: Vec<Scalar>","fields":{"containerName":"Snapshot","fieldName":"scalars","typeValue":"Vec<Scalar>","collectionKind":"Vec","contextLocator":"src/lib.rs:1:4"}},{"id":"type:src/lib.rs-scalars-vec-3","kind":"type","role":"field-type","value":"Vec<Scalar>","action":"evidence","path":"src/lib.rs","ownerPath":"src/lib.rs","symbol":"Vec","startLine":3,"endLine":3,"locator":"src/lib.rs:3:3","fields":{"fieldName":"scalars","typeValue":"Vec<Scalar>","collectionKind":"Vec"}},{"id":"collection:vec","kind":"collection","role":"family","value":"Vec","action":"evidence","symbol":"Vec","fields":{"collectionKind":"Vec"}}],"edges":[{"source":"query:vec-collection-fields","target":"field:src/lib.rs-scalars-3","relation":"matches"},{"source":"field:src/lib.rs-scalars-3","target":"type:src/lib.rs-scalars-vec-3","relation":"has_type"},{"source":"field:src/lib.rs-scalars-3","target":"collection:vec","relation":"collection_of"}]}"#,
        "",
    );
    write_provider_bin_config(&root, "rust", &bin_dir.join("rs-harness"));
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
        stdout.contains("field:struct-field(scalars)@src/lib.rs:1:4!evidence"),
        "{stdout}"
    );
    assert!(
        stdout.contains("type:field-type(Vec-Scalar-)@src/lib.rs:3:3!evidence"),
        "{stdout}"
    );
    assert!(
        stdout.contains("collection:family(Vec)!evidence"),
        "{stdout}"
    );
    assert!(stdout.contains("has_type"), "{stdout}");
    assert!(stdout.contains("collection_of"), "{stdout}");
    assert!(
        stdout.contains("globalCoverage=matched=vec missing=collection,fields"),
        "{stdout}"
    );
    assert!(
        stdout.contains("treeSitterHandles=interface-fields:scalars;exported-declarations:Vec"),
        "{stdout}"
    );
    assert!(
        stdout
            .contains("actionFrontier=A1.query-code,A2.fd-query,A3.rg-query,A4.owner-items,A5.treesitter-query"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "nextCommand=asp rust query --selector src/lib.rs:1:15 --workspace languages/rust-harness --code"
        ),
        "{stdout}"
    );
    assert!(!stdout.contains("subagentHint="), "{stdout}");
    assert!(!stdout.contains("frontierActions="), "{stdout}");
    assert!(!stdout.contains("S1=>asp rust query"), "{stdout}");
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
