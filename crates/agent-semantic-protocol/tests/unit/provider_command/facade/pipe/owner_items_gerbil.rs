use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation,
    write_stdout_stderr_provider,
};

#[test]
fn gerbil_projection_import_publishes_turso_owner_items_without_warm_provider_process() {
    let root = temp_project_root("search-owner-gerbil-projection-import");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(root.join("src/checker")).expect("create source");
    std::fs::write(
        root.join("src/checker/types.ss"),
        "(def (type-compatible? actual expected)\n  (equal? actual expected))\n",
    )
    .expect("write source");
    write_stdout_stderr_provider(
        &bin_dir,
        "gslph",
        r#"{
          "schemaId":"agent.semantic-protocols.semantic-language-projection",
          "schemaVersion":"1",
          "protocolId":"agent.semantic-protocols.language-projection",
          "protocolVersion":"1",
          "languageId":"gerbil-scheme",
          "harness":{"harnessId":"gerbil-scheme-language-project-harness","parserAbi":"gerbil-parser-v1","selectorDialect":"gerbil-scheme"},
          "sources":[{"sourceId":"source:src/checker/types.ss","path":"src/checker/types.ss","sourceKind":"source"}],
          "owners":[{"ownerId":"owner:src/checker/types.ss","sourceId":"source:src/checker/types.ss","kind":"module","name":"types"}],
          "items":[{"itemId":"item:type-compatible","ownerId":"owner:src/checker/types.ss","kind":"function","name":"type-compatible?","selector":"gerbil-scheme://src/checker/types.ss#item/function/type-compatible"}],
          "relations":[
            {"from":{"kind":"source","id":"source:src/checker/types.ss"},"kind":"contains","to":{"kind":"owner","id":"owner:src/checker/types.ss"}},
            {"from":{"kind":"owner","id":"owner:src/checker/types.ss"},"kind":"contains","to":{"kind":"item","id":"item:type-compatible"}}
          ]
        }"#,
        "projection-harness-called",
    );
    write_activation(&root, &[provider("gerbil-scheme", Vec::new())]);

    let cold = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .args([
            "gerbil-scheme",
            "search",
            "owner",
            "src/checker/types.ss",
            "items",
            "--query",
            "type-compatible",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("search cold Gerbil owner");
    assert!(
        cold.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&cold.stderr)
    );
    let cold_stdout = String::from_utf8(cold.stdout).expect("cold stdout");
    assert!(
        cold_stdout.contains("state=projection-cold-required"),
        "{cold_stdout}"
    );
    assert!(
        cold_stdout.contains("providerProcessCount=0"),
        "{cold_stdout}"
    );

    let imported = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .args([
            "gerbil-scheme",
            "projection",
            "import",
            "--owner",
            "src/checker/types.ss",
            "--workspace",
            ".",
        ])
        .output()
        .expect("import Gerbil projection");
    assert!(
        imported.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&imported.stderr)
    );
    let imported_stdout = String::from_utf8(imported.stdout).expect("import stdout");
    assert!(
        imported_stdout.contains("[projection-import] language=gerbil-scheme"),
        "{imported_stdout}"
    );
    assert!(
        imported_stdout.contains("parserProcessCount=1"),
        "{imported_stdout}"
    );

    let warm = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .args([
            "gerbil-scheme",
            "search",
            "owner",
            "src/checker/types.ss",
            "items",
            "--query",
            "type-compatible",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("search imported Gerbil owner");
    assert!(
        warm.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&warm.stderr)
    );
    let warm_stdout = String::from_utf8(warm.stdout).expect("warm stdout");
    assert!(
        warm_stdout.contains("alg=graph-turbo-owner-items"),
        "{warm_stdout}"
    );
    assert!(warm_stdout.contains("type-compatible?"), "{warm_stdout}");
    assert!(
        warm_stdout.contains(
            "structuralSelector=gerbil-scheme://src/checker/types.ss#item/function/type-compatible"
        ),
        "{warm_stdout}"
    );
    assert!(
        !warm_stdout.contains("agent.semantic-protocols.semantic-language-projection"),
        "warm search invoked the harness: {warm_stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn gerbil_projection_import_rejects_nonrelative_owner_before_provider_lookup() {
    let root = temp_project_root("search-owner-gerbil-projection-invalid-owner");
    let rejected = asp_command(&root)
        .args([
            "gerbil-scheme",
            "projection",
            "import",
            "--owner",
            "/tmp/invalid.ss",
            "--workspace",
            ".",
        ])
        .output()
        .expect("reject invalid Gerbil projection owner");
    assert!(!rejected.status.success());
    let stderr = String::from_utf8(rejected.stderr).expect("invalid owner stderr");
    assert!(
        stderr.contains("projection import --owner must be a non-empty relative source path"),
        "{stderr}"
    );
    let _ = std::fs::remove_dir_all(root);
}
