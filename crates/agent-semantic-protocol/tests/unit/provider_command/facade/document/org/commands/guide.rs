use crate::provider_command::support::{
    asp_command, provider, state_runtime_bin, temp_project_root, write_activation,
    write_echo_provider, write_marker_provider,
};

#[test]
fn asp_org_guide_routes_to_state_home_orgize_provider() {
    let root = temp_project_root("org-document-guide-state-home");
    write_echo_provider(&state_runtime_bin(&root), "orgize", "state-home-org-guide");
    write_activation(&root, &[provider("org", Vec::new())]);

    let output = asp_command(&root)
        .args(["org", "guide"])
        .output()
        .expect("run asp org guide");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("state-home-org-guide"), "{stdout}");
    assert!(stdout.contains("args=[guide]"), "{stdout}");
    assert!(
        !stdout.contains("asp org capture")
            && !stdout.contains("asp org recall")
            && !stdout.contains("asp org archive"),
        "{stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_org_rejects_removed_document_commands_without_provider_spawn() {
    let root = temp_project_root("org-document-removed-command-rejections");
    let called = root.join("orgize-called");
    write_marker_provider(&state_runtime_bin(&root), "orgize", &called);
    write_activation(&root, &[provider("org", Vec::new())]);

    for command in [
        "capture",
        "recall",
        "archive",
        "lint",
        "sdd",
        "agent-planning",
        "sparse-tree",
        "task-list",
    ] {
        let output = asp_command(&root)
            .args(["org", command])
            .output()
            .unwrap_or_else(|error| panic!("run asp org {command}: {error}"));
        assert!(
            !output.status.success(),
            "{command} unexpectedly succeeded with stdout: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        let stderr = String::from_utf8(output.stderr).expect("stderr");
        assert!(
            stderr.contains("usage: asp <")
                && stderr.contains("<guide|search|query|check|cache|info|bench|projection"),
            "command={command} stderr={stderr}"
        );
        assert!(
            !called.exists(),
            "removed command spawned provider: {command}"
        );
    }

    let _ = std::fs::remove_dir_all(root);
}
