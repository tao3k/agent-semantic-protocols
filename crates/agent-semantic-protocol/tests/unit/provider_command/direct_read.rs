use super::support;

fn direct_read_fixture(name: &str) -> std::path::PathBuf {
    let root = support::temp_project_root(name);
    std::fs::create_dir_all(root.join("src")).expect("create src dir");
    let source = (1..=50)
        .map(|line| format!("pub fn line_{line}() {{}}\n"))
        .collect::<String>();
    std::fs::write(root.join("src/lib.rs"), source).expect("write source");
    support::write_activation(&root, &[support::provider("rust", Vec::new())]);
    root
}

#[test]
fn direct_source_read_accepts_whole_file_selector() {
    let root = direct_read_fixture("direct-read-whole-file");
    let output = support::asp_command(&root)
        .args([
            "rust",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "src/lib.rs",
            "--workspace",
            ".",
            "--code",
        ])
        .output()
        .expect("run asp");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("pub fn line_1() {}"), "stdout={stdout}");
    assert!(stdout.contains("pub fn line_50() {}"), "stdout={stdout}");
    std::fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn direct_source_read_accepts_wide_selector_range() {
    let root = direct_read_fixture("direct-read-wide-range");
    let output = support::asp_command(&root)
        .args([
            "rust",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "src/lib.rs:1-41",
            "--workspace",
            ".",
            "--code",
        ])
        .output()
        .expect("run asp");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("pub fn line_1() {}"), "stdout={stdout}");
    assert!(stdout.contains("pub fn line_41() {}"), "stdout={stdout}");
    std::fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn direct_source_read_accepts_bounded_selector_range() {
    let root = direct_read_fixture("direct-read-bounded-range");
    let output = support::asp_command(&root)
        .args([
            "rust",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "src/lib.rs:2-3",
            "--workspace",
            ".",
            "--code",
        ])
        .output()
        .expect("run asp");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert_eq!(stdout, "pub fn line_2() {}\npub fn line_3() {}\n");
    std::fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn ordinary_selector_query_does_not_use_direct_source_read_limit() {
    let root = direct_read_fixture("ordinary-selector-query");
    let output = support::asp_command(&root)
        .args([
            "rust",
            "query",
            "--selector",
            "src/lib.rs:1-41",
            "--workspace",
            ".",
            "--code",
        ])
        .output()
        .expect("run asp");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("pub fn line_1() {}"), "stdout={stdout}");
    assert!(stdout.contains("pub fn line_41() {}"), "stdout={stdout}");
    std::fs::remove_dir_all(root).expect("remove temp root");
}
