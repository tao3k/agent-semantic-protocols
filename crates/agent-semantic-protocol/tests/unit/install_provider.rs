use super::{
    asset_name, checksum_name, parse_sha256_checksum, path_segment, provider_release,
    validate_target,
};

#[test]
fn asset_names_are_rev_independent_and_target_selected() {
    let spec = provider_release("julia").expect("julia release spec");
    assert_eq!(
        asset_name(&spec, "aarch64-apple-darwin"),
        "asp-julia-harness-aarch64-apple-darwin.tar.gz"
    );
    assert_eq!(
        checksum_name(&spec, "aarch64-apple-darwin"),
        "asp-julia-harness-aarch64-apple-darwin.tar.gz.sha256"
    );
}

#[test]
fn parse_checksum_accepts_common_sha256_formats() {
    assert_eq!(
        parse_sha256_checksum(
            "ABCDEFabcdef0123456789abcdef0123456789abcdef0123456789abcdef0123  file.tar.gz\n"
        )
        .as_deref(),
        Some("abcdefabcdef0123456789abcdef0123456789abcdef0123456789abcdef0123")
    );
}

#[test]
fn rev_path_segment_is_filesystem_safe() {
    assert_eq!(
        path_segment("refs/tags/v1.2.3+build"),
        "refs_tags_v1.2.3_build"
    );
}

#[test]
fn unsupported_apple_intel_target_is_rejected() {
    let spec = provider_release("rust").expect("rust release spec");
    let error = validate_target(&spec, "x86_64-apple-darwin").expect_err("unsupported target");
    assert!(error.contains("unsupported target `x86_64-apple-darwin`"));
    assert!(error.contains("aarch64-apple-darwin"));
}

#[test]
fn every_workspace_provider_has_an_explicit_non_shell_build_recipe() {
    for language in ["rust", "typescript", "python", "julia"] {
        let spec = super::provider_release(language).expect("pinned provider release");
        let artifact = spec
            .workspace_artifact
            .as_ref()
            .unwrap_or_else(|| panic!("{language} workspace artifact"));
        assert!(!artifact.root.trim().is_empty(), "{language} artifact root");
        assert!(
            !artifact.entrypoint.trim().is_empty(),
            "{language} artifact entrypoint"
        );
        let build = spec
            .workspace_build
            .as_ref()
            .unwrap_or_else(|| panic!("{language} workspace build recipe"));
        assert!(!build.program.trim().is_empty(), "{language} build program");
        let program_name = std::path::Path::new(&build.program)
            .file_name()
            .and_then(|name| name.to_str())
            .expect("workspace build program name");
        assert!(
            !matches!(program_name, "sh" | "bash" | "zsh" | "fish"),
            "{language} must not use a command shell"
        );
        super::resolve_workspace_relative_path(
            std::path::Path::new("/workspace"),
            &build.working_directory,
            "workingDirectory",
        )
        .expect("project-relative working directory");
        assert!(
            !build.derived_paths.is_empty(),
            "{language} derived path boundary"
        );
        for path in &build.derived_paths {
            super::resolve_workspace_relative_path(
                std::path::Path::new("/workspace"),
                path,
                "derivedPaths",
            )
            .expect("project-relative derived path");
        }
        let artifact_root = super::resolve_workspace_relative_path(
            std::path::Path::new("/workspace"),
            &artifact.root,
            "workspaceArtifact.root",
        )
        .expect("project-relative artifact root");
        assert!(
            build
                .derived_paths
                .iter()
                .any(|derived| artifact_root.starts_with(
                    super::resolve_workspace_relative_path(
                        std::path::Path::new("/workspace"),
                        derived,
                        "derivedPaths",
                    )
                    .expect("project-relative derived path")
                )),
            "{language} artifact root must be inside one derived path"
        );
        if let Some(launch) = &artifact.launch {
            let program_name = std::path::Path::new(&launch.program)
                .file_name()
                .and_then(|name| name.to_str())
                .expect("artifact launch program name");
            assert!(
                !matches!(program_name, "sh" | "bash" | "zsh" | "fish"),
                "{language} launch must not delegate provider behavior to a shell"
            );
        }
    }
    let rust = super::provider_release("rust").expect("rust release");
    let args = &rust.workspace_build.expect("rust build recipe").args;
    assert!(args.iter().any(|arg| arg == "--locked"));
    assert!(args.iter().any(|arg| arg == "--release"));
    assert!(args.iter().any(|arg| arg == "cli"));
    assert!(args.iter().any(|arg| arg == "rs-harness"));

    let typescript = super::provider_release("typescript").expect("typescript release");
    let typescript_artifact = typescript
        .workspace_artifact
        .expect("typescript workspace artifact");
    assert!(typescript_artifact.root.ends_with("dist/provider"));
    assert_eq!(typescript_artifact.entrypoint, "ts-harness.mjs");
    let typescript_launch = typescript_artifact
        .launch
        .expect("typescript launch recipe");
    assert_eq!(typescript_launch.program, "node");
    assert!(typescript_launch.args_relative_to_artifact);

    let python = super::provider_release("python").expect("python release");
    let python_build = python.workspace_build.expect("python build recipe");
    assert!(python_build.args.iter().any(|arg| arg == "--no-editable"));
    let python_launch = python
        .workspace_artifact
        .expect("python workspace artifact")
        .launch
        .expect("python launch recipe");
    assert!(python_launch.program_relative_to_artifact);
    assert!(python_launch.args_relative_to_artifact);
}

#[test]
fn workspace_build_paths_reject_parent_traversal() {
    let error = super::resolve_workspace_relative_path(
        std::path::Path::new("/workspace"),
        "../stale-provider",
        "workspaceArtifact.root",
    )
    .expect_err("parent traversal must be rejected");
    assert!(error.contains("project-relative path without parent traversal"));
}

#[test]
fn workspace_artifact_tree_copy_preserves_merkle_root() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let temp = std::env::temp_dir().join(format!(
        "asp-workspace-artifact-tree-{}-{nonce}",
        std::process::id()
    ));
    let source = temp.join("source");
    let copied = temp.join("copied");
    std::fs::create_dir_all(source.join("nested")).expect("create artifact source");
    std::fs::write(source.join("entrypoint"), b"#!/bin/sh\nexit 0\n").expect("write entrypoint");
    std::fs::write(
        source.join("nested/module.js"),
        b"export const value = 1;\n",
    )
    .expect("write nested module");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            source.join("entrypoint"),
            std::fs::Permissions::from_mode(0o755),
        )
        .expect("chmod entrypoint");
        std::os::unix::fs::symlink("nested/module.js", source.join("module-link"))
            .expect("create artifact symlink");
    }

    let source_snapshot =
        super::super::install_provider_workspace_artifact::capture_workspace_artifact_snapshot(
            &source,
        )
        .expect("snapshot source artifact");
    super::super::install_provider_workspace_artifact::copy_workspace_artifact_tree(
        &source, &copied,
    )
    .expect("copy artifact tree");
    let copied_snapshot =
        super::super::install_provider_workspace_artifact::capture_workspace_artifact_snapshot(
            &copied,
        )
        .expect("snapshot copied artifact");
    assert_eq!(source_snapshot.root_digest, copied_snapshot.root_digest);
    assert_eq!(source_snapshot.leaf_count, copied_snapshot.leaf_count);

    std::fs::write(
        copied.join("nested/module.js"),
        b"export const value = 2;\n",
    )
    .expect("mutate copied artifact");
    let mutated_snapshot =
        super::super::install_provider_workspace_artifact::capture_workspace_artifact_snapshot(
            &copied,
        )
        .expect("snapshot mutated artifact");
    assert_ne!(source_snapshot.root_digest, mutated_snapshot.root_digest);
    let _ = std::fs::remove_dir_all(temp);
}

#[test]
fn corrupted_workspace_artifact_cas_is_rematerialized_from_merkle_source() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let temp = std::env::temp_dir().join(format!(
        "asp-workspace-artifact-cas-repair-{}-{nonce}",
        std::process::id()
    ));
    let source = temp.join("source");
    let cas = temp.join("cas");
    std::fs::create_dir_all(&source).expect("create artifact source");
    std::fs::write(source.join("provider"), b"provider-v1\n").expect("write provider");
    let expected =
        super::super::install_provider_workspace_artifact::capture_workspace_artifact_snapshot(
            &source,
        )
        .expect("snapshot source artifact");
    super::super::install_provider_workspace_cas::materialize_workspace_artifact_cas(
        &source,
        &cas,
        &expected.root_digest,
        expected.leaf_count,
    )
    .expect("materialize artifact CAS");
    std::fs::write(cas.join("provider"), b"corrupt\n").expect("corrupt artifact CAS");
    super::super::install_provider_workspace_cas::materialize_workspace_artifact_cas(
        &source,
        &cas,
        &expected.root_digest,
        expected.leaf_count,
    )
    .expect("repair artifact CAS");
    let repaired =
        super::super::install_provider_workspace_artifact::capture_workspace_artifact_snapshot(
            &cas,
        )
        .expect("snapshot repaired artifact CAS");
    assert_eq!(expected.root_digest, repaired.root_digest);
    assert_eq!(expected.leaf_count, repaired.leaf_count);
    let _ = std::fs::remove_dir_all(temp);
}
