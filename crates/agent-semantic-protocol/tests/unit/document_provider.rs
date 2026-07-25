use crate::provider_command::support::{
    asp_command, provider, temp_project_root, write_activation,
};

#[test]
fn markdown_facade_dispatches_through_orgize_without_project_args() {
    let root = temp_project_root("md-orgize-mode");
    write_echo_orgize(&root);
    write_activation(&root, &[provider("md", Vec::new())]);

    let output = asp_command(&root)
        .args([
            "md",
            "query",
            "--term",
            "Project",
            "--workspace",
            ".",
            "--view",
            "metadata",
        ])
        .output()
        .expect("run asp md query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("query stdout");
    assert!(
        stdout.starts_with("asp md args=[query][--term][Project]"),
        "{stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn markdown_provider_can_be_disabled_from_project_config() {
    let root = temp_project_root("md-disabled-config");
    write_echo_orgize(&root);
    write_activation(&root, &[provider("md", Vec::new())]);
    std::fs::write(root.join("asp.toml"), "[providers.md]\nenabled = false\n")
        .expect("write asp config");

    let output = asp_command(&root)
        .args([
            "md",
            "search",
            "prime",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp md search");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("language `md` is disabled by asp.toml"),
        "{stderr}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn markdown_registry_and_manifest_share_orgize_provider() {
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root");
    let registry: serde_json::Value = serde_json::from_slice(
        &std::fs::read(workspace_root.join("schemas/semantic-language-registry.providers.v1.json"))
            .expect("read semantic language registry"),
    )
    .expect("parse semantic language registry");
    let markdown_registry = registry["languages"]
        .as_array()
        .expect("registry languages")
        .iter()
        .find(|provider| provider["languageId"].as_str() == Some("md"))
        .expect("markdown registry provider");
    assert_eq!(markdown_registry["providerId"].as_str(), Some("orgize"));
    assert_eq!(markdown_registry["binary"].as_str(), Some("orgize"));

    let markdown_manifest = agent_semantic_hook::builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id().as_str() == "md")
        .expect("markdown provider manifest");
    assert_eq!(markdown_manifest.provider_id().as_str(), "orgize");
    assert_eq!(markdown_manifest.binary(), "orgize");
    assert_eq!(
        markdown_manifest.source().default_extensions,
        vec![".md".to_string(), ".markdown".to_string()]
    );
}

fn write_echo_orgize(root: &std::path::Path) {
    let binary = root
        .join("home")
        .join(".agent-semantic-protocols")
        .join("runtime")
        .join("bin")
        .join("orgize");
    std::fs::create_dir_all(binary.parent().expect("orgize bin parent"))
        .expect("create orgize bin directory");
    std::fs::write(
        &binary,
        "#!/bin/sh\nprintf 'orgize args='\nfor arg in \"$@\"; do printf '[%s]' \"$arg\"; done\nprintf '\\n'\n",
    )
    .expect("write fake orgize provider");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&binary)
            .expect("fake orgize metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&binary, permissions).expect("chmod fake orgize");
    }
}
