// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! State Home layout tests.

use agent_semantic_artifacts::ProjectBinding;
use agent_semantic_artifacts::StateHomeLayout;

#[test]
fn physical_namespace_has_no_schema_version_suffix() {
    let binding = ProjectBinding::resolve(None, "repo", "/tmp/workspace").unwrap();
    let layout = StateHomeLayout::new("/tmp/state-home");
    let workspace = layout.workspace(&binding.workspace).unwrap();
    let runtime_root = layout.runtime_state().root().to_path_buf();

    for path in [
        layout.catalog(),
        layout.workspaces(),
        &runtime_root,
        layout.receipts(),
        layout.trash(),
        &workspace.root,
    ] {
        let rendered = path.to_string_lossy();
        assert!(!rendered.contains("state-v"));
        assert!(!rendered.contains("/v1"));
        assert!(!rendered.contains("/v2"));
    }
}

#[test]
fn runtime_generated_state_is_derived_by_one_typed_layout() {
    let layout = StateHomeLayout::new("/tmp/state-home");
    let runtime = layout.runtime_state();

    assert_eq!(
        runtime.root(),
        std::path::Path::new("/tmp/state-home/runtime")
    );
    assert_eq!(
        runtime.artifacts().root(),
        std::path::Path::new("/tmp/state-home/runtime/artifacts")
    );
    assert_eq!(
        runtime.bin(),
        std::path::PathBuf::from("/tmp/state-home/runtime/bin")
    );
    assert_eq!(
        runtime.serving().root(),
        std::path::Path::new("/tmp/state-home/runtime/serving")
    );
    assert_eq!(
        runtime.serving().endpoint_receipt(),
        std::path::PathBuf::from("/tmp/state-home/runtime/serving/endpoint.v1.json")
    );
    assert_eq!(
        runtime.serving().readiness(),
        std::path::PathBuf::from("/tmp/state-home/runtime/serving/readiness")
    );
    assert_eq!(
        runtime.serving().workspaces(),
        std::path::PathBuf::from("/tmp/state-home/runtime/serving/workspaces")
    );
    assert_eq!(
        runtime.serving().hook_host_native_handoff_mailbox(),
        std::path::PathBuf::from(
            "/tmp/state-home/runtime/serving/mailboxes/hook-host-native-handoff"
        )
    );
}

#[test]
fn global_control_cache_and_resources_are_derived_by_one_typed_layout() {
    let layout = StateHomeLayout::new("/tmp/state-home");
    let control = layout.control();
    let cache = layout.cache();
    let resources = layout.resources();

    assert_eq!(
        control.catalog(),
        std::path::PathBuf::from("/tmp/state-home/catalog/state.turso")
    );
    assert_eq!(
        control.hook_client_config(),
        std::path::PathBuf::from("/tmp/state-home/control/config/hook-client.toml")
    );
    assert_eq!(
        control.session_registry_root(),
        std::path::PathBuf::from("/tmp/state-home/control/sessions")
    );
    assert_eq!(
        control.agent_registry(),
        std::path::PathBuf::from("/tmp/state-home/control/config/agents/config.toml")
    );
    assert_eq!(
        cache.reader_behavior(),
        std::path::PathBuf::from("/tmp/state-home/cache/reader-behavior/blake3-256")
    );
    assert_eq!(
        resources.org(),
        std::path::PathBuf::from("/tmp/state-home/resources/org")
    );
    assert_eq!(
        resources.live_corpus(),
        std::path::PathBuf::from("/tmp/state-home/resources/live-corpus")
    );

    for path in [
        control.hook_client_config(),
        control.session_registry_root(),
        control.agent_registry(),
        cache.reader_behavior(),
        resources.org(),
        resources.live_corpus(),
    ] {
        let rendered = path.to_string_lossy();
        assert!(!rendered.contains("projects/by-id"));
        assert!(!rendered.contains("hooks/projects"));
        assert!(!rendered.contains("hooks/generations"));
    }
}

#[test]
fn runtime_status_memory_is_content_bound_and_deterministic() {
    let serving = StateHomeLayout::new("/tmp/state-home")
        .runtime_state()
        .serving();
    let first = serving.status_memory(7, "binding-a", "blake3-256:artifact-a");
    let repeated = serving.status_memory(7, "binding-a", "blake3-256:artifact-a");
    let refreshed = serving.status_memory(7, "binding-a", "blake3-256:artifact-b");

    assert_eq!(first, repeated);
    assert_ne!(first, refreshed);
    assert_eq!(first.parent(), Some(serving.root()));
}

#[test]
fn workspace_materialization_uses_only_the_full_content_digest() {
    let state_home = tempfile::tempdir().unwrap();
    let workspace_root = tempfile::tempdir().unwrap();
    let binding =
        ProjectBinding::resolve(None, "gix-common-dir:/repo/.git", workspace_root.path()).unwrap();
    let layout = StateHomeLayout::new(state_home.path());

    let paths = layout.materialize_workspace(&binding).unwrap();
    let digest = binding
        .workspace
        .digest
        .as_str()
        .strip_prefix("blake3-256:")
        .unwrap();

    assert_eq!(
        paths.root,
        state_home.path().join("workspaces").join(digest)
    );
    assert_eq!(paths.facts, paths.root.join("facts.turso"));
    assert!(paths.artifacts.is_dir());
    assert!(paths.observations.is_dir());
    assert!(paths.binding_path().is_file());
    assert!(!state_home.path().join("projects").exists());
}

#[test]
fn workspace_materialization_rejects_tampered_binding_metadata() {
    let state_home = tempfile::tempdir().unwrap();
    let workspace_root = tempfile::tempdir().unwrap();
    let binding =
        ProjectBinding::resolve(None, "gix-common-dir:/repo/.git", workspace_root.path()).unwrap();
    let layout = StateHomeLayout::new(state_home.path());
    let paths = layout.materialize_workspace(&binding).unwrap();
    let mut tampered = serde_json::to_value(&binding).unwrap();
    tampered["bindingDigest"] = serde_json::json!(binding.repo.digest);
    std::fs::write(
        paths.binding_path(),
        serde_json::to_vec_pretty(&tampered).unwrap(),
    )
    .unwrap();

    let error = layout.materialize_workspace(&binding).unwrap_err();
    assert!(error.contains("project binding digest or identity mismatch"));
}

#[test]
fn workspace_removal_requires_the_full_bound_digest() {
    let state_home = tempfile::tempdir().unwrap();
    let workspace_root = tempfile::tempdir().unwrap();
    let binding =
        ProjectBinding::resolve(None, "gix-common-dir:/repo/.git", workspace_root.path()).unwrap();
    let layout = StateHomeLayout::new(state_home.path());
    let paths = layout.materialize_workspace(&binding).unwrap();
    std::fs::write(paths.artifacts.join("payload"), b"payload").unwrap();

    let discovered = layout.materialized_workspaces().unwrap();
    assert_eq!(discovered.len(), 1);
    assert_eq!(discovered[0].binding, binding);
    assert!(discovered[0].byte_count >= 7);

    assert!(
        layout
            .stage_workspace_removal("workspace-short-id")
            .is_err()
    );
    assert!(paths.root.exists());
    layout
        .stage_workspace_removal(binding.workspace.digest.as_str())
        .unwrap()
        .commit()
        .unwrap();
    assert!(!paths.root.exists());
    assert!(layout.materialized_workspaces().unwrap().is_empty());
}

#[test]
fn workspace_removal_is_staged_and_can_rollback_before_catalog_commit() {
    let state_home = tempfile::tempdir().unwrap();
    let workspace_root = tempfile::tempdir().unwrap();
    let binding =
        ProjectBinding::resolve(None, "gix-common-dir:/repo/.git", workspace_root.path()).unwrap();
    let layout = StateHomeLayout::new(state_home.path());
    let paths = layout.materialize_workspace(&binding).unwrap();
    let payload = paths.artifacts.join("payload");
    std::fs::write(&payload, b"payload").unwrap();

    let staged = layout
        .stage_workspace_removal(binding.workspace.digest.as_str())
        .unwrap();
    assert!(!paths.root.exists());
    assert!(staged.staged_root().is_dir());
    assert!(layout.materialized_workspaces().unwrap().is_empty());

    staged.rollback().unwrap();
    assert_eq!(std::fs::read(&payload).unwrap(), b"payload");

    let staged = layout
        .stage_workspace_removal(binding.workspace.digest.as_str())
        .unwrap();
    let staged_root = staged.staged_root().to_path_buf();
    staged.commit().unwrap();
    assert!(!staged_root.exists());
    assert!(!paths.root.exists());
}

#[test]
fn state_home_contract_finds_only_entries_outside_the_v1_allowlist() {
    let state_home = tempfile::tempdir().unwrap();
    let layout = StateHomeLayout::new(state_home.path());
    for canonical in [
        "catalog",
        "control/config/agents",
        "control/sessions",
        "runtime/bin",
        "runtime/artifacts",
        "runtime/serving",
        "workspaces",
        "cache",
        "resources",
        "blobs",
        "receipts",
        "trash",
    ] {
        std::fs::create_dir_all(state_home.path().join(canonical)).unwrap();
    }
    std::fs::write(
        layout.control().asp_config(),
        "[dev]\nenabled = false\n\n[hook-engine]\nenabled = true\n",
    )
    .unwrap();
    std::fs::create_dir_all(state_home.path().join("obsolete-root")).unwrap();
    std::fs::write(state_home.path().join("obsolete-root/payload"), b"old").unwrap();
    std::fs::create_dir_all(state_home.path().join("runtime/obsolete-runtime-tree")).unwrap();
    std::fs::write(
        state_home
            .path()
            .join("control/config/runtime-artifacts.toml"),
        b"[dev]\nenabled = false\n",
    )
    .unwrap();

    let violations = layout.non_contract_entries().unwrap();
    assert_eq!(
        violations
            .iter()
            .map(|entry| entry.relative_path.as_path())
            .collect::<Vec<_>>(),
        vec![
            std::path::Path::new("control/config/runtime-artifacts.toml"),
            std::path::Path::new("obsolete-root"),
            std::path::Path::new("runtime/obsolete-runtime-tree"),
        ]
    );

    let obsolete_root = violations
        .iter()
        .find(|entry| entry.relative_path == std::path::Path::new("obsolete-root"))
        .unwrap();
    let staged = layout.stage_non_contract_entry(obsolete_root).unwrap();
    assert!(!state_home.path().join("obsolete-root").exists());
    staged.rollback().unwrap();
    assert_eq!(
        std::fs::read(state_home.path().join("obsolete-root/payload")).unwrap(),
        b"old"
    );
}

#[test]
fn explicit_trash_purge_leaves_no_persistent_history_namespace() {
    let state_home = tempfile::tempdir().unwrap();
    let layout = StateHomeLayout::new(state_home.path());
    std::fs::create_dir_all(layout.trash().join("old-layout/nested")).unwrap();
    std::fs::write(
        layout.trash().join("old-layout/nested/payload"),
        b"obsolete",
    )
    .unwrap();
    std::fs::write(layout.trash().join("stale-receipt"), b"stale").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(
            layout.trash().join("old-layout/nested"),
            std::fs::Permissions::from_mode(0o555),
        )
        .unwrap();
    }

    let cleanup = layout.purge_trash().unwrap();

    assert_eq!(
        cleanup.relative_paths,
        vec![
            std::path::PathBuf::from("trash/old-layout"),
            std::path::PathBuf::from("trash/stale-receipt"),
        ]
    );
    assert_eq!(cleanup.byte_count, 13);
    assert!(layout.trash().read_dir().unwrap().next().is_none());
}

#[test]
fn runtime_transport_paths_are_short_and_bound_to_state_home_identity() {
    let first = StateHomeLayout::new(
        "/a/very/long/state/home/whose/durable/receipts/must/not/be/moved/to/the/os/runtime/directory/first",
    )
    .runtime_state()
    .transport();
    let same = StateHomeLayout::new(
        "/a/very/long/state/home/whose/durable/receipts/must/not/be/moved/to/the/os/runtime/directory/first",
    )
    .runtime_state()
    .transport();
    let second = StateHomeLayout::new(
        "/a/very/long/state/home/whose/durable/receipts/must/not/be/moved/to/the/os/runtime/directory/second",
    )
    .runtime_state()
    .transport();

    assert_eq!(first, same);
    assert_ne!(first, second);
    assert!(first.opentelemetry_query_socket().as_os_str().len() < 100);
    assert_eq!(first.root().parent(), Some(std::path::Path::new("/tmp")));
    assert!(
        first
            .root()
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("asp-runtime-"))
    );
}
