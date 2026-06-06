use crate::provider_command::support::{asp_command, make_executable, temp_project_root};

#[test]
fn top_level_usage_lists_document_facades() {
    let root = temp_project_root("document-facade-top-level-help");

    let output = asp_command(&root)
        .arg("--help")
        .output()
        .expect("run top-level help");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("rust|typescript|python|julia|org|md"),
        "stderr={stderr}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn document_facade_help_does_not_spawn_orgize() {
    let root = temp_project_root("document-facade-help");

    for language in ["org", "md"] {
        let output = asp_command(&root)
            .env_remove("ASP_ORGIZE_BIN")
            .env("PATH", "")
            .args([language, "--help"])
            .output()
            .expect("run document help");

        assert!(
            output.status.success(),
            "language={language} stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).expect("stdout");
        assert!(
            stdout.contains(&format!("usage: asp {language} <guide|search|query> ...")),
            "stdout={stdout}"
        );
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn org_facade_uses_native_orgize_dependency() {
    let root = temp_project_root("org-document-facade");
    let orgize = root.join("orgize");
    std::fs::write(&orgize, "#!/bin/sh\nexit 42\n").expect("write orgize");
    make_executable(&orgize);

    let output = asp_command(&root)
        .env("ASP_ORGIZE_BIN", &orgize)
        .env("PATH", "")
        .args(["org", "search", "prime", "--view", "seeds", "."])
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
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn md_facade_uses_native_orgize_dependency() {
    let root = temp_project_root("md-document-facade");
    let orgize = root.join("orgize");
    std::fs::write(&orgize, "#!/bin/sh\nexit 42\n").expect("write orgize");
    make_executable(&orgize);

    let output = asp_command(&root)
        .env("ASP_ORGIZE_BIN", &orgize)
        .env("PATH", "")
        .args(["md", "search", "prime", "--view", "seeds", "."])
        .output()
        .expect("run asp md search");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("[search-prime] lang=md"), "stdout={stdout}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn document_facade_rejects_non_document_commands() {
    let root = temp_project_root("document-facade-rejects-check");

    let output = asp_command(&root)
        .args(["org", "check", "."])
        .output()
        .expect("run asp org check");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("unsupported document command `check`"),
        "stderr={stderr}"
    );
    let _ = std::fs::remove_dir_all(root);
}
