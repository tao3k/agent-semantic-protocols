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
            "--workspace",
            ".",
            "--view",
            "seeds",
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
    assert!(!stdout.contains("subagentHint="), "{stdout}");
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
fn gerbil_search_pipe_recalls_source_and_config_files_without_provider_spawn() {
    let root = temp_project_root("gerbil-search-pipe-config-recall");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("gerbil.pkg"),
        "(package: sample/local-alias)\n(export main)\n",
    )
    .expect("write gerbil package config");
    std::fs::write(
        root.join("build.ss"),
        "(import :std/build-script)\n(defbuild-script main)\n",
    )
    .expect("write gerbil build config");
    std::fs::write(
        root.join("src/main.ss"),
        "(package: sample/local-alias)\n(def (use-let-star)\n  (let* ((star-value \"ok\")\n         (star-alias star-value))\n    (needs-string star-alias)))\n",
    )
    .expect("write gerbil source");
    write_marker_provider(&bin_dir, "gerbil-scheme-harness", &marker);
    write_activation(&root, &[provider("gerbil-scheme", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "gerbil-scheme",
            "search",
            "pipe",
            "gerbil.pkg build.ss local alias",
            "--view",
            "seeds",
            ".",
        ])
        .output()
        .expect("run asp gerbil-scheme search pipe");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("lang=gerbil-scheme"), "{stdout}");
    assert!(
        stdout.contains("pageIndexHandles=gerbil.pkg,build.ss")
            || stdout.contains("pageIndexHandles=build.ss,gerbil.pkg"),
        "{stdout}"
    );
    assert!(stdout.contains("src/main.ss"), "{stdout}");
    assert!(stdout.contains("fdQuery=gerbil.pkg|build.ss"), "{stdout}");
    assert!(
        stdout.contains("recommendedNext=A1.owner-items")
            || stdout.contains("recommendedNext=A2.owner-items"),
        "{stdout}"
    );
    assert!(!stdout.contains("A1=query-code"), "{stdout}");
    assert!(
        !marker.exists(),
        "search pipe config/source recall should not spawn provider"
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
    assert!(!stdout.contains("subagentHint="), "{stdout}");
    assert!(
        stdout.contains("treeSitterHandles=exported-declarations:Fiber|Queue|Scope"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_splits_api_compounds_before_seed_quality_analysis() {
    let root = temp_project_root("search-pipe-api-compound-query");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src/buf")).expect("create rust src");
    std::fs::write(
        root.join("src/buf/buf_mut.rs"),
        "pub trait BufMut {\n    unsafe fn advance_mut(&mut self, cnt: usize);\n}\n",
    )
    .expect("write buf mut source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "BufMut 的 advance_mut/unsafe 写入边界如何被组织？先找 trait 和实现 owner。",
            "--view",
            "seeds",
            ".",
        ])
        .output()
        .expect("run asp rust search pipe api compound");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains(
            "queryTerms=BufMut:symbol,advance_mut:symbol,unsafe:concept,trait:concept,owner:context"
        ),
        "{stdout}"
    );
    assert!(!stdout.contains("advance_mut/unsafe:symbol"), "{stdout}");
    assert!(!stdout.contains("owner。:concept"), "{stdout}");
    assert!(
        stdout.contains("strongCoverage=matched=BufMut weak=-"),
        "{stdout}"
    );
    assert!(
        stdout.contains("packageCohesion=high packages=src/buf"),
        "{stdout}"
    );
    assert!(stdout.contains("queryQuality=medium reason=ok"), "{stdout}");
    assert!(stdout.contains("fdQuery=BufMut|advance_mut"), "{stdout}");
    assert!(stdout.contains("recommendedNext=A1.query-code"), "{stdout}");
    assert!(!marker.exists(), "search pipe should not spawn provider");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_preserves_rust_path_compounds_as_precise_symbol_terms() {
    let root = temp_project_root("search-pipe-rust-path-compound-query");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src/runtime")).expect("create rust src");
    std::fs::write(
        root.join("src/runtime/handle.rs"),
        "pub struct Handle;\nimpl Handle {\n    pub fn enter(&self) -> EnterGuard { EnterGuard }\n}\npub struct EnterGuard;\n",
    )
    .expect("write handle source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "Tokio runtime Handle::enter 的上下文进入和 guard 生命周期应该从哪些 owner frontier 开始定位？",
            "--view",
            "seeds",
            ".",
        ])
        .output()
        .expect("run asp rust search pipe rust path compound");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("Handle::enter:symbol"), "{stdout}");
    assert!(
        stdout.contains("strongCoverage=matched=Handle::enter weak=-"),
        "{stdout}"
    );
    assert!(stdout.contains("fdQuery=Handle::enter|Tokio"), "{stdout}");
    assert!(stdout.contains("A1=fd-query("), "{stdout}");
    assert!(
        stdout.contains("nextCommand=asp fd -query 'Handle::enter|Tokio' ."),
        "{stdout}"
    );
    assert!(!marker.exists(), "search pipe should not spawn provider");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_keeps_gerbil_package_terms_on_gerbil_candidates() {
    let root = temp_project_root("search-pipe-gerbil-package-candidates");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src/extensions")).expect("create gerbil src");
    std::fs::create_dir_all(root.join("analyzers/WendaoGraph.jl/src/reasoning/page_index"))
        .expect("create distractor src");
    std::fs::write(
        root.join("gerbil.pkg"),
        "(package: sample/app\n depend: (\"git.cons.io/mighty-gerbils/gerbil-poo\"))\n",
    )
    .expect("write gerbil package");
    std::fs::write(
        root.join("src/extensions/poo.ss"),
        ";;; gxpkg required dependency extension facts for Poo\n(def poo-extension-active? #t)\n",
    )
    .expect("write gerbil source");
    std::fs::write(
        root.join("analyzers/WendaoGraph.jl/src/reasoning/page_index/actions.jl"),
        "GitHub Actions matrix cache build restore key\n",
    )
    .expect("write distractor source");
    write_marker_provider(&bin_dir, "gerbil-scheme-harness", &marker);
    write_activation(&root, &[provider("gerbil-scheme", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "gerbil-scheme",
            "search",
            "pipe",
            "GitHub Actions matrix gxpkg deps install gerbil.pkg Poo cache",
            "--view",
            "seeds",
            ".",
        ])
        .output()
        .expect("run asp gerbil-scheme search pipe");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.starts_with("[search-pipe] lang=gerbil-scheme"),
        "{stdout}"
    );
    assert!(stdout.contains("gerbil.pkg"), "{stdout}");
    assert!(stdout.contains("src/extensions/poo.ss"), "{stdout}");
    assert!(
        stdout.contains("globalCoverage=matched=gerbil.pkg,poo,gxpkg"),
        "{stdout}"
    );
    assert!(
        stdout.contains("recommendedNext=A1.owner-items"),
        "{stdout}"
    );
    assert!(
        stdout.contains("nextCommand=asp gerbil-scheme search owner gerbil.pkg items"),
        "{stdout}"
    );
    assert!(
        stdout.contains("finderHandles=") && stdout.contains("poo"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("analyzers/WendaoGraph.jl"),
        "Gerbil package query drifted to unrelated owner:\n{stdout}"
    );
    assert!(
        !stdout.contains("provider-called"),
        "search pipe should stay ASP-owned:\n{stdout}"
    );
    assert!(!marker.exists(), "search pipe should not spawn provider");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_auto_clauses_suppress_cross_package_selector_drift() {
    let root = temp_project_root("search-pipe-package-drift");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    for path in [
        "marlin-gerbil-scheme/src",
        "marlin-org-workflow/src",
        "marlin-gerbil-ir/src",
        "tools/dev-dependency/src",
    ] {
        std::fs::create_dir_all(root.join(path)).expect("create package");
    }
    std::fs::write(
        root.join("marlin-gerbil-scheme/src/real_gxi.rs"),
        "pub fn real_gxi_smoke_path() {}\n",
    )
    .expect("write real_gxi");
    std::fs::write(
        root.join("marlin-org-workflow/src/lib.rs"),
        "pub fn marlin_org_workflow_dependency() {}\n",
    )
    .expect("write org workflow");
    std::fs::write(
        root.join("marlin-gerbil-ir/src/lib.rs"),
        "pub struct IrFact { pub long_field_signatures: Vec<(String, String, String, String, String, String, String, String, String, String, String, String, String, String, String)> }\n",
    )
    .expect("write gerbil ir");
    std::fs::write(
        root.join("tools/dev-dependency/src/lib.rs"),
        "pub fn through_smoke_dev_dependency() {}\n",
    )
    .expect("write dev dependency");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "marlin-gerbil-scheme marlin-org-workflow marlin-gerbil-ir real_gxi.rs through smoke dev dependency long-field-signatures",
            "--view",
            "seeds",
            ".",
        ])
        .output()
        .expect("run asp rust search pipe package drift");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("queryPack=clauses=3 quality=low"),
        "{stdout}"
    );
    assert!(stdout.contains("real_gxi.rs:symbol"), "{stdout}");
    assert!(stdout.contains("marlin-gerbil-scheme:symbol"), "{stdout}");
    assert!(stdout.contains("long-field-signatures:concept"), "{stdout}");
    assert!(!stdout.contains("through:context"), "{stdout}");
    assert!(!stdout.contains("smoke:context"), "{stdout}");
    assert!(stdout.contains("rgQuery=real_gxi.rs"), "{stdout}");
    assert!(stdout.contains("risk=package-drift"), "{stdout}");
    assert!(!stdout.contains("A1=query-code"), "{stdout}");
    assert!(
        !stdout.contains("recommendedNext=A1.query-code"),
        "{stdout}"
    );
    assert!(
        stdout.contains("recommendedNext=A1.owner-items"),
        "{stdout}"
    );
    assert!(
        stdout.contains("reason=query-selector-low-confidence,owner-seed-base-required"),
        "{stdout}"
    );
    assert!(stdout.contains("subagentHint="), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}
