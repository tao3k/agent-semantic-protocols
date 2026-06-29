use crate::provider_command::support::{asp_command, temp_project_root};

#[test]
fn asp_org_lint_fix_writes_formatted_org_files() {
    let root = temp_project_root("org-document-command-lint-fix");
    let path = root.join("table.org");
    std::fs::write(&path, "* Table\n|a|bb|\n|long|c|\n").expect("write table fixture");

    let output = asp_command(&root)
        .args(["org", "lint", "--fix", path.to_str().unwrap()])
        .output()
        .expect("run asp org lint --fix");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert_eq!(stdout, "[ok] orgize lint\n");
    let fixed = std::fs::read_to_string(&path).expect("read fixed org");
    assert!(fixed.contains("| a    | bb |"), "{fixed}");
    assert!(fixed.contains("| long | c  |"), "{fixed}");

    let _ = std::fs::remove_dir_all(root);
}
