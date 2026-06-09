use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_marker_provider,
};

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
    assert!(stdout.starts_with("[search-pipe]"), "{stdout}");
    assert!(
        stdout.contains("lang=rust view=seeds source=auto ranker=graph-turbo:owner-query"),
        "{stdout}"
    );
    assert!(
        stdout.contains("query=HookDecision ClientReceipt"),
        "{stdout}"
    );
    assert!(
        stdout.contains("queryTerms=HookDecision:symbol,ClientReceipt:symbol"),
        "{stdout}"
    );
    assert!(
        stdout.contains("globalCoverage=matched=hookdecision,clientreceipt missing=-"),
        "{stdout}"
    );
    assert!(
        stdout.contains("strongCoverage=matched=HookDecision,ClientReceipt weak=-"),
        "{stdout}"
    );
    assert!(stdout.contains("queryQuality=high reason=ok"), "{stdout}");
    assert!(
        stdout.contains("sourceTrace=provider:partial,finder:used"),
        "{stdout}"
    );
    assert!(
        stdout.contains("handles=inputTerms=HookDecision,ClientReceipt contextTerms=- ownerSeedTerms=HookDecision,ClientReceipt conceptTerms=-"),
        "{stdout}"
    );
    assert!(
        stdout.contains("parserHandles=HookDecision@src/lib.rs:1,ClientReceipt@src/lib.rs:2"),
        "{stdout}"
    );
    assert!(
        stdout
            .contains("nextClasses=fd-query,rg-query,owner-items,treesitter-query,query-selector"),
        "{stdout}"
    );
    assert!(stdout.contains("[graph-frontier]"), "{stdout}");
    assert!(!stdout.contains("Q=query:term"), "{stdout}");
    assert!(
        stdout.contains("I=item:symbol(clientreceipt)@src/lib.rs:2:2!syntax"),
        "{stdout}"
    );
    assert!(
        stdout.contains("I2=item:symbol(hookdecision)@src/lib.rs:1:1!syntax"),
        "{stdout}"
    );
    let frontier_line = stdout
        .lines()
        .find(|line| line.starts_with("evidenceFrontier="))
        .expect("evidence frontier line");
    assert!(!frontier_line.contains("Q.fzf"), "{stdout}");
    for entry in ["I.syntax", "H.hot", "I2.syntax", "H2.hot"] {
        assert!(frontier_line.contains(entry), "{stdout}");
    }
    assert!(
        stdout.contains("seedPlan=seed-query alg=asp-search-pipe-v2"),
        "{stdout}"
    );
    let removed_expression_label = ["pipe", "Expr="].concat();
    let removed_operator = ['|', '>'].iter().collect::<String>();
    assert!(!stdout.contains(&removed_expression_label), "{stdout}");
    assert!(!stdout.contains(&removed_operator), "{stdout}");
    assert!(!stdout.contains("pipePlan="), "{stdout}");
    assert!(!stdout.contains("pipeSurfaces="), "{stdout}");
    assert!(!stdout.contains("pipeProjections="), "{stdout}");
    assert!(!stdout.contains("pipeChoice="), "{stdout}");
    assert!(!stdout.contains("pipeExecution="), "{stdout}");
    assert!(!stdout.contains("R4=>"), "{stdout}");
    assert!(!stdout.contains("frontierActions=R4."), "{stdout}");
    assert!(!stdout.contains("pipeStages="), "{stdout}");
    assert!(!stdout.contains("selectorPolicy="), "{stdout}");
    assert!(!stdout.contains("context=>"), "{stdout}");
    assert!(!stdout.contains("pipe=>asp rust search pipe"), "{stdout}");
    assert!(
        !stdout.contains("frontierActions=R1.reasoning("),
        "{stdout}"
    );
    assert!(!stdout.contains("frontierActions="), "{stdout}");
    assert!(!stdout.contains("pipeCommands="), "{stdout}");
    assert!(!stdout.contains("conditionalActions="), "{stdout}");
    assert!(
        stdout.contains("commandHandles=fdQuery=HookDecision|ClientReceipt;rgQuery=HookDecision|ClientReceipt|clientreceipt|hookdecision;ownerItems=src/lib.rs:HookDecision|ClientReceipt|hookdecision|clientreceipt"),
        "{stdout}"
    );
    assert!(
        stdout.contains("treeSitterHandles=exported-declarations:HookDecision|ClientReceipt"),
        "{stdout}"
    );
    assert!(stdout.contains("actionRank=A1,A2,A3,A4,A5"), "{stdout}");
    assert!(
        stdout.contains("A1=query-code(selector=src/lib.rs:"),
        "{stdout}"
    );
    assert!(
        stdout.contains("actionFrontier=A1.query-code,A2.fd-query,A3.rg-query,A4.owner-items,A5.treesitter-query"),
        "{stdout}"
    );
    assert!(stdout.contains("recommendedNext=A1.query-code"), "{stdout}");
    assert!(
        stdout.contains("nextCommand=asp rust query --selector src/lib.rs:")
            && stdout.contains(" --workspace . --code"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("R1=>asp rust search reasoning"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("<selector>") && !stdout.contains("<owner-path>"),
        "{stdout}"
    );
    assert!(
        stdout.contains("avoid=repeat-search-pipe,broad-fzf,raw-rg,manual-window-scan,direct-source-read,raw-read"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_reports_multi_clause_query_pack_coverage() {
    let root = temp_project_root("search-pipe-multi-clause");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub struct Fiber;\npub struct Queue;\npub struct Scope;\nfn lifecycle() {}\n",
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
            "Fiber Queue|Scope lifecycle",
            "--view",
            "seeds",
            ".",
        ])
        .output()
        .expect("run asp rust search pipe multi-clause");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("queryPack=clauses=2 quality=high raw='Fiber Queue|Scope lifecycle'"),
        "{stdout}"
    );
    assert!(
        stdout.contains("clauseCoverage=C1 matched=fiber,queue missing=-"),
        "{stdout}"
    );
    assert!(
        stdout.contains("clauseCoverage=C2 matched=scope,lifecycle missing=-"),
        "{stdout}"
    );
    assert!(
        stdout.contains("handles=inputTerms=lifecycle,Fiber,Queue,Scope contextTerms=- ownerSeedTerms=Fiber,Queue,Scope conceptTerms=lifecycle"),
        "{stdout}"
    );
    assert!(stdout.contains("evidenceFrontier="), "{stdout}");
    assert!(
        stdout.contains("actionFrontier=A1.query-code,A2.fd-query,A3.rg-query,A4.owner-items,A5.treesitter-query"),
        "{stdout}"
    );
    assert!(
        stdout.contains("treeSitterHandles=exported-declarations:Fiber|Queue|Scope"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}
