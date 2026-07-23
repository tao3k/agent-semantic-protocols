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
fn workspace_build_env_expands_pinned_workspace_root() {
    let build = super::WorkspaceBuildSpec {
        program: "gxi".to_string(),
        args: Vec::new(),
        working_directory: "languages/gerbil".to_string(),
        source_snapshot_anchors: vec!["languages/gerbil/gerbil.pkg".to_string()],
        derived_paths: vec!["languages/gerbil/target".to_string()],
        env: std::collections::BTreeMap::from([(
            "HOME".to_string(),
            "${ASP_WORKSPACE_ROOT}/languages/gerbil/target/home".to_string(),
        )]),
    };
    let rendered =
        super::rendered_workspace_build_env(&build, std::path::Path::new("/immutable/source-root"));
    assert_eq!(
        rendered.get("HOME").map(String::as_str),
        Some("/immutable/source-root/languages/gerbil/target/home")
    );
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
fn pinned_workspace_snapshot_isolated_from_live_edits() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let temp = std::env::temp_dir().join(format!(
        "asp-pinned-workspace-source-{}-{nonce}",
        std::process::id()
    ));
    let live = temp.join("live");
    let pinned = temp.join("pinned");
    let derived = live.join("target");
    std::fs::create_dir_all(live.join("src")).expect("create live source");
    std::fs::create_dir_all(&derived).expect("create derived path");
    std::fs::write(live.join("src/lib.rs"), b"pub fn value() -> u8 { 1 }\n")
        .expect("write live source");
    std::fs::write(derived.join("ignored"), b"derived\n").expect("write derived output");

    let before = super::super::install_provider_workspace_source::capture_workspace_build_snapshot(
        &live,
        &[derived],
        &[],
        "provider-digest",
    )
    .expect("capture workspace snapshot");
    super::super::install_provider_workspace_source::copy_workspace_snapshot_leaves(
        &live, &pinned, &before,
    )
    .expect("materialize pinned source");

    std::fs::write(live.join("src/lib.rs"), b"pub fn value() -> u8 { 2 }\n")
        .expect("edit live source after pinning");
    let pinned_snapshot =
        super::super::install_provider_workspace_source::capture_workspace_build_snapshot(
            &pinned,
            &[],
            &[],
            "provider-digest",
        )
        .expect("capture pinned snapshot");
    let live_after =
        super::super::install_provider_workspace_source::capture_workspace_build_snapshot(
            &live,
            &[live.join("target")],
            &[],
            "provider-digest",
        )
        .expect("capture edited live source");
    assert_eq!(
        before.evidence.root_digest,
        pinned_snapshot.evidence.root_digest
    );
    assert_ne!(before.evidence.root_digest, live_after.evidence.root_digest);

    std::fs::write(pinned.join("src/lib.rs"), b"pub fn value() -> u8 { 3 }\n")
        .expect("mutate pinned build source");
    let pinned_after =
        super::super::install_provider_workspace_source::capture_workspace_build_snapshot(
            &pinned,
            &[],
            &[],
            "provider-digest",
        )
        .expect("capture mutated pinned source");
    assert_eq!(before.changed_paths(&pinned_after), vec!["src/lib.rs"]);
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

#[test]
fn ignored_harness_anchor_is_forced_into_merkle_source_cas() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let temp = std::env::temp_dir().join(format!(
        "asp-workspace-source-anchor-{}-{nonce}",
        std::process::id()
    ));
    let live = temp.join("live");
    let pinned = temp.join("pinned");
    let derived = live.join("target");
    let manifest = live.join("Cargo.toml");
    let lockfile = live.join("Cargo.lock");
    std::fs::create_dir_all(live.join("src")).expect("create live source");
    std::fs::create_dir_all(&derived).expect("create derived path");
    std::fs::write(live.join(".gitignore"), b"Cargo.lock\ntarget\n")
        .expect("write ignore contract");
    std::fs::write(
        &manifest,
        b"[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
    )
    .expect("write manifest anchor");
    std::fs::write(&lockfile, b"version = 4\n").expect("write ignored lock anchor");
    std::fs::write(live.join("src/lib.rs"), b"pub fn value() -> u8 { 1 }\n")
        .expect("write live source");
    std::fs::write(derived.join("ignored"), b"derived\n").expect("write derived output");

    let anchors = vec![manifest, lockfile.clone()];
    let before = super::super::install_provider_workspace_source::capture_workspace_build_snapshot(
        &live,
        &[derived],
        &anchors,
        "provider-digest",
    )
    .expect("capture anchored workspace snapshot");
    assert!(before.leaves.contains_key("Cargo.toml"));
    assert!(before.leaves.contains_key("Cargo.lock"));
    assert!(!before.leaves.contains_key("target/ignored"));

    super::super::install_provider_workspace_source::copy_workspace_snapshot_leaves(
        &live, &pinned, &before,
    )
    .expect("materialize anchored source CAS");
    assert!(pinned.join("Cargo.lock").is_file());

    std::fs::write(&lockfile, b"version = 4\n# changed\n").expect("change lock anchor");
    let after = super::super::install_provider_workspace_source::capture_workspace_build_snapshot(
        &live,
        &[live.join("target")],
        &anchors,
        "provider-digest",
    )
    .expect("capture changed anchor snapshot");
    assert_ne!(before.evidence.root_digest, after.evidence.root_digest);
    assert_eq!(before.changed_paths(&after), vec!["Cargo.lock"]);
    let _ = std::fs::remove_dir_all(temp);
}

#[test]
fn typescript_workspace_descriptor_materializes_locked_dependencies_before_build() {
    let descriptor = super::super::install_provider_workspace_descriptor::workspace_install_descriptor_for_language("typescript")
        .expect("typescript workspace install descriptor should resolve");
    let materialization = descriptor
        .dependency_materialization
        .expect("typescript must materialize dependencies inside the isolated build sandbox");
    assert_eq!(materialization.program, "npm");
    assert_eq!(materialization.args, ["ci"]);
    assert_eq!(
        materialization.working_directory,
        "languages/typescript-lang-project-harness"
    );
}

#[test]
fn resolves_provider_owned_workspace_descriptors_through_manifests() {
    let cases = [
        ("rust", "rs-harness", "rs-harness"),
        ("typescript", "ts-harness", "ts-harness"),
        ("python", "py-harness", "py-harness"),
        ("julia", "julia-lang-project-harness", "asp-julia-harness"),
        ("gerbil-scheme", "gerbil-scheme-harness", "gslph"),
        ("org", "orgize", "orgize"),
        ("md", "orgize", "orgize"),
    ];

    for (language_id, provider_id, binary) in cases {
        let descriptor = super::super::install_provider_workspace_descriptor::workspace_install_descriptor_for_language(language_id)
            .unwrap_or_else(|error| panic!("{language_id}: {error}"));
        assert_eq!(descriptor.provider_id, provider_id);
        assert_eq!(descriptor.binary, binary);
        assert!(
            !descriptor
                .workspace_build
                .source_snapshot_anchors
                .is_empty(),
            "{language_id} must publish harness-owned source snapshot anchors"
        );
    }
}
