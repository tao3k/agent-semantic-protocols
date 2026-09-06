// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

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
fn workspace_retirement_requires_the_full_bound_digest() {
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

    assert!(layout.retire_workspace("workspace-short-id").is_err());
    assert!(paths.root.exists());
    layout
        .retire_workspace(binding.workspace.digest.as_str())
        .unwrap();
    assert!(!paths.root.exists());
    assert!(layout.materialized_workspaces().unwrap().is_empty());
}

#[test]
fn workspace_retirement_is_staged_and_can_rollback_before_catalog_commit() {
    let state_home = tempfile::tempdir().unwrap();
    let workspace_root = tempfile::tempdir().unwrap();
    let binding =
        ProjectBinding::resolve(None, "gix-common-dir:/repo/.git", workspace_root.path()).unwrap();
    let layout = StateHomeLayout::new(state_home.path());
    let paths = layout.materialize_workspace(&binding).unwrap();
    let payload = paths.artifacts.join("payload");
    std::fs::write(&payload, b"payload").unwrap();

    let staged = layout
        .stage_workspace_retirement(binding.workspace.digest.as_str())
        .unwrap();
    assert!(!paths.root.exists());
    assert!(staged.staged_root().is_dir());
    assert!(layout.materialized_workspaces().unwrap().is_empty());

    staged.rollback().unwrap();
    assert_eq!(std::fs::read(&payload).unwrap(), b"payload");

    let staged = layout
        .stage_workspace_retirement(binding.workspace.digest.as_str())
        .unwrap();
    let staged_root = staged.staged_root().to_path_buf();
    staged.commit().unwrap();
    assert!(!staged_root.exists());
    assert!(!paths.root.exists());
}

#[test]
fn retired_state_root_requires_identity_and_observation_then_stages_atomically() {
    let state_home = tempfile::tempdir().unwrap();
    let layout = StateHomeLayout::new(state_home.path());
    let root = state_home
        .path()
        .join("projects/by-id/repo-0123456789abcdef");
    std::fs::create_dir_all(root.join("cache")).unwrap();
    std::fs::write(
        root.join("project.json"),
        serde_json::to_vec(&serde_json::json!({
            "repoId": "repo-0123456789abcdef",
            "checkoutRoot": "/workspace/project"
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::write(root.join(".last-seen-ms"), b"1000\n").unwrap();
    std::fs::write(root.join("cache/payload"), b"payload").unwrap();

    let roots = layout.retired_state_roots().unwrap();
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].object_id, "repo-0123456789abcdef");
    assert_eq!(roots[0].last_observed_at_ms, 1000);
    assert!(roots[0].byte_count >= 7);

    let staged = layout.stage_retired_state_root(&roots[0]).unwrap();
    assert!(!root.exists());
    staged.rollback().unwrap();
    assert_eq!(
        std::fs::read(root.join("cache/payload")).unwrap(),
        b"payload"
    );

    let root = layout.retired_state_roots().unwrap().remove(0);
    let staged = layout.stage_retired_state_root(&root).unwrap();
    let staged_path = staged.staged_root().to_path_buf();
    staged.commit().unwrap();
    assert!(!staged_path.exists());
    assert!(!root.root.exists());
}
