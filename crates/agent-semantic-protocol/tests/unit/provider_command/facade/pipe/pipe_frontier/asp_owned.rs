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
    let frontier_line = stdout
        .lines()
        .find(|line| line.starts_with("frontier="))
        .expect("frontier line");
    for entry in ["Q.fzf", "I.syntax", "H.code", "I2.syntax", "H2.code"] {
        assert!(frontier_line.contains(entry), "{stdout}");
    }
    assert!(
        stdout.contains("pipePlan=query-pipeline alg=asp-search-pipe-v1"),
        "{stdout}"
    );
    assert!(
        stdout.contains("pipeSurfaces=owner,items,tests"),
        "{stdout}"
    );
    let removed_expression_label = ["pipe", "Expr="].concat();
    let removed_operator = ['|', '>'].iter().collect::<String>();
    assert!(!stdout.contains(&removed_expression_label), "{stdout}");
    assert!(!stdout.contains(&removed_operator), "{stdout}");
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
    assert!(!stdout.contains("context=>"), "{stdout}");
    assert!(!stdout.contains("pipe=>asp rust search pipe"), "{stdout}");
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
    assert!(!pipe_commands_line.contains("search prime"), "{stdout}");
    assert!(!pipe_commands_line.contains("search pipe"), "{stdout}");
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
