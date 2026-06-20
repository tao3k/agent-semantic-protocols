use crate::provider_command::support::{
    asp_command, make_executable, prepend_path, temp_project_root,
};

#[test]
fn org_facade_uses_native_orgize_dependency() {
    let root = temp_project_root("org-document-facade");
    std::fs::write(root.join("plan.org"), "* Document Prime\n").expect("write org fixture");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    let orgize = bin_dir.join("orgize");
    std::fs::write(&orgize, "#!/bin/sh\nexit 42\n").expect("write orgize");
    make_executable(&orgize);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .args([
            "org",
            "search",
            "prime",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp org search");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("[search-prime] lang=org"),
        "stdout={stdout}"
    );
    assert!(stdout.contains("owner:path("), "stdout={stdout}");
    assert!(stdout.contains("plan.org"), "stdout={stdout}");
    assert!(stdout.contains("frontier=O.owner"), "stdout={stdout}");
    assert!(!stdout.contains("G>{}"), "stdout={stdout}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn org_facade_guide_explains_element_query_axes() {
    let root = temp_project_root("org-document-guide-query-axes");

    let output = asp_command(&root)
        .args(["org", "guide", "--workspace", "."])
        .output()
        .expect("run asp org guide");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("|query-axis field matches=key-or-key=value value-match=contains"),
        "{stdout}"
    );
    assert!(
        stdout.contains("|field-map heading fields=level,title,todo,todoType,priority,tag"),
        "{stdout}"
    );
    assert!(
        stdout.contains("|field-map block fields=kind=source|export,lang,backend"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "|recipe paragraph-content=asp org query --kind paragraph --term <term> --workspace . --content"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "|surface contract-trace purpose=contract-org-evaluation-trace output=json content=false"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "|cmd contract-trace=asp org contract trace --org-contract-registry <contract.org> <target.org>"
        ),
        "{stdout}"
    );

    let query_output = asp_command(&root)
        .args(["org", "query", "guide", "--workspace", "."])
        .output()
        .expect("run asp org query guide");
    assert!(
        query_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&query_output.stderr)
    );
    let query_stdout = String::from_utf8(query_output.stdout).expect("stdout");
    assert!(
        query_stdout
            .contains("|combine all=--selector+--term+--kind+--field semantics=intersection"),
        "{query_stdout}"
    );
    assert!(
        query_stdout.contains("|content-rule requires=--selector|--term|--kind|--field"),
        "{query_stdout}"
    );
    assert!(
        query_stdout.contains(
            "|mode selector command=\"query --selector <structural-selector> --workspace . --view metadata\""
        ),
        "{query_stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}
