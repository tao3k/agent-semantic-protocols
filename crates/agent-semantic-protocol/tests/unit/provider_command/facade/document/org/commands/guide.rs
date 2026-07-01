use crate::provider_command::support::{asp_command, temp_project_root};

#[test]
fn asp_org_guide_exposes_generic_ast_recipes_only() {
    let root = temp_project_root("org-document-command-guide-generic");

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
    assert!(
        stdout.contains("|recipe todo-tasks=asp org query --kind task --field todo=TODO"),
        "{stdout}"
    );
    assert!(
        stdout.contains("|recipe checked-checklist-items=asp org query --kind checklistItem"),
        "{stdout}"
    );
    assert!(
        stdout.contains("|recipe property-value=asp org query --kind property --field key=<KEY>"),
        "{stdout}"
    );
    assert!(
        stdout.contains("|recipe capture-task=asp org capture --contract agent.task.v1 --title"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "|recipe sdd-kind-properties=asp org query --kind property --field key=SDD_KIND"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("|recipe org-id-properties=asp org query --kind property --field key=ID"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "|recipe tagged-tasks=asp org query --kind task --term <TEXT> --field tag=<TAG>"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("|recipe done-tasks=asp org query --kind task --field todo=DONE"),
        "{stdout}"
    );

    for domain_recipe in [
        "sdd-property",
        "wendao-task",
        "wendao-orgid",
        "agent-plan",
        "plan-record",
    ] {
        assert!(
            !stdout.contains(domain_recipe),
            "retired recipe `{domain_recipe}` leaked into asp org guide:\n{stdout}"
        );
    }

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_org_rejects_domain_specific_embedded_commands() {
    let root = temp_project_root("org-document-command-domain-specific-rejections");

    for command in ["sdd", "agent-planning", "sparse-tree", "task-list"] {
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
            stderr.contains(&format!("unsupported document command `{command}`")),
            "command={command} stderr={stderr}"
        );
        let supported = stderr
            .split("supported commands are ")
            .nth(1)
            .unwrap_or_default();
        assert!(!supported.contains("sdd"), "{stderr}");
        assert!(!supported.contains("agent-planning"), "{stderr}");
        assert!(!supported.contains("task-list"), "{stderr}");
        assert!(!supported.contains("sparse-tree"), "{stderr}");
    }

    let _ = std::fs::remove_dir_all(root);
}
