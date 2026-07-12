use crate::provider_command::support::{
    asp_command, make_executable, prepend_path, temp_project_root,
};

#[test]
fn gerbil_deps_search_reads_active_gxi_source_tree_without_provider_activation() {
    let root = temp_project_root("gerbil-deps-search-active-gxi");
    let bin_dir = write_gerbil_install_fixture(&root);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .args([
            "gerbil-scheme",
            "search",
            "deps",
            "gerbil",
            ":std/srfi/13",
            "items",
            "--query",
            "string-prefix",
        ])
        .output()
        .expect("run asp");

    assert!(
        output.status.success(),
        "status={:?} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains(
            "[gerbil-deps] namespace=gerbil authority=active-gxi module=:std/srfi/13 scope=standard-library/srfi"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "|use import=\"(import (only-in :std/srfi/13 string-prefix? string-prefix-ci?))\""
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "|item kind=export name=string-prefix? selector=gerbil:/std/srfi/13#export/string-prefix?"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "|item kind=export name=string-prefix-ci? selector=gerbil:/std/srfi/13#export/string-prefix-ci?"
        ),
        "{stdout}"
    );
    assert!(
        !stdout.contains("name=string-prefix-length "),
        "string-prefix query should prefer the predicate family over contains matches:\n{stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn gerbil_deps_query_reads_included_export_definition() {
    let root = temp_project_root("gerbil-deps-query-active-gxi");
    let bin_dir = write_gerbil_install_fixture(&root);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .args([
            "gerbil-scheme",
            "query",
            "--selector",
            "gerbil:/std/srfi/13#export/string-prefix?",
            "--code",
        ])
        .output()
        .expect("run asp");

    assert!(
        output.status.success(),
        "status={:?} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains(";;; selector: gerbil:/std/srfi/13#export/string-prefix?"),
        "{stdout}"
    );
    assert!(
        stdout.contains(";;; import: (import (only-in :std/srfi/13 string-prefix?))"),
        "{stdout}"
    );
    assert!(stdout.contains("(def (string-prefix? s1 s2"), "{stdout}");
    assert!(
        stdout.contains("(%string-prefix? s1 start1 end1 s2 start2 end2)"),
        "{stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn gerbil_deps_search_blocks_broad_non_module_queries() {
    let root = temp_project_root("gerbil-deps-search-broad-query");

    let output = asp_command(&root)
        .args([
            "gerbil-scheme",
            "search",
            "deps",
            "gerbil",
            "std",
            "items",
            "--query",
            "string",
        ])
        .output()
        .expect("run asp");

    assert!(!output.status.success(), "status={:?}", output.status);
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("[gerbil-deps] namespace=gerbil status=blocked"),
        "{stderr}"
    );
    assert!(
        stderr.contains(
            "asp gerbil-scheme search deps gerbil :std/srfi/13 items --query string-prefix"
        ),
        "{stderr}"
    );

    let _ = std::fs::remove_dir_all(root);
}

fn write_gerbil_install_fixture(root: &std::path::Path) -> std::path::PathBuf {
    let prefix = root.join("gerbil-prefix");
    let bin_dir = prefix.join("bin");
    let source_dir = prefix.join("v0.18.2/src/std/srfi");
    std::fs::create_dir_all(&bin_dir).expect("create fake Gerbil bin");
    std::fs::create_dir_all(&source_dir).expect("create fake Gerbil source dir");

    let gxi = bin_dir.join("gxi");
    std::fs::write(&gxi, "#!/bin/sh\nexit 0\n").expect("write fake gxi");
    make_executable(&gxi);

    std::fs::write(
        source_dir.join("13.ss"),
        r#";;; -*- Gerbil -*-
(import :gerbil/gambit)
(export
  string-prefix-length string-prefix-length-ci
  string-prefix? string-prefix-ci?
  string-suffix? string-suffix-ci?)
(include "srfi-13.scm")
"#,
    )
    .expect("write srfi/13.ss");
    std::fs::write(
        source_dir.join("srfi-13.scm"),
        r#";;; string-prefix? s1 s2 [start1 end1 start2 end2]
(def (string-prefix? s1 s2
                     (start1 0) (end1 (string-length s1))
                     (start2 0) (end2 (string-length s2)))
  (%string-prefix? s1 start1 end1 s2 start2 end2))

(def (string-prefix-ci? s1 s2
                        (start1 0) (end1 (string-length s1))
                        (start2 0) (end2 (string-length s2)))
  (%string-prefix-ci? s1 start1 end1 s2 start2 end2))
"#,
    )
    .expect("write srfi-13.scm");

    bin_dir
}
