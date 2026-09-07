// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

#[tokio::test]
async fn cli_install_binary_composition_acquires_once_and_consumes_operation_lease() {
    let protocol_home = std::env::temp_dir().join(format!(
        "asp-cli-single-artifact-transaction-{}",
        std::process::id()
    ));
    let artifact_root = protocol_home.join("runtime/artifacts");
    let source = protocol_home.join("build/asp");
    std::fs::create_dir_all(source.parent().expect("source parent"))
        .expect("create source directory");
    std::fs::write(&source, b"asp-cli-composition").expect("write source artifact");
    let target = protocol_home.join("home/.local/bin/asp");

    let receipt = super::ensure_protocol_binary_installed(&super::ProtocolBinaryInstallPlan {
        binary_identity: super::RuntimeBinaryIdentityV1::asp_bootstrap(),
        current_exe: source,
        explicit_candidate_source: None,
        target,
        artifact_root,
    })
    .await
    .expect("CLI install binary composition");

    assert_eq!(receipt.lock_acquisition_count, 1);
    assert_eq!(receipt.quiescence_operation.as_deref(), Some("publish:asp"));
    assert_eq!(receipt.lease_producer_process_id, Some(std::process::id()));
    assert_eq!(receipt.lease_consumer_process_id, Some(std::process::id()));
    assert!(
        receipt
            .quiescence_lease_nonce
            .as_deref()
            .is_some_and(|nonce| !nonce.is_empty())
    );
    assert!(
        !agent_semantic_artifacts::runtime_artifact_quiescence::runtime_artifact_quiescence_lease_path(
            &protocol_home,
        )
        .exists()
    );
    std::fs::remove_dir_all(protocol_home).expect("remove CLI composition fixture");
}

#[cfg(unix)]
#[tokio::test]
async fn runtime_artifact_transaction_switches_client_launcher_without_switching_serving_aliases() {
    use std::os::unix::fs::symlink;

    let root = std::env::temp_dir().join(format!(
        "asp-managed-path-alias-reconciliation-{}",
        std::process::id()
    ));
    let artifact_root = root.join("runtime/artifacts");
    let old_artifact = artifact_root
        .join("blake3-256")
        .join("a".repeat(64))
        .join("asp");
    std::fs::create_dir_all(old_artifact.parent().expect("old artifact parent"))
        .expect("create old artifact store");
    std::fs::write(&old_artifact, b"old-asp").expect("write old artifact");

    let source = root.join("build/asp");
    std::fs::create_dir_all(source.parent().expect("source parent")).expect("create build dir");
    std::fs::write(&source, b"new-asp").expect("write new artifact");

    let primary_target = root.join("runtime/bin/asp");
    let first_alias = root.join("path-a/asp");
    let second_alias = root.join("path-b/asp");
    for alias in [&first_alias, &second_alias] {
        std::fs::create_dir_all(alias.parent().expect("alias parent")).expect("create alias dir");
        symlink(&old_artifact, alias).expect("link managed alias");
    }
    let unrelated = root.join("unrelated/asp");
    std::fs::create_dir_all(unrelated.parent().expect("unrelated parent"))
        .expect("create unrelated dir");
    std::fs::write(&unrelated, b"unrelated").expect("write unrelated binary");

    super::ensure_protocol_binary_installed(&super::ProtocolBinaryInstallPlan {
        binary_identity: super::RuntimeBinaryIdentityV1::asp_bootstrap(),
        current_exe: source,
        explicit_candidate_source: None,
        target: root.join("home/.local/bin/asp"),
        artifact_root: artifact_root.clone(),
    })
    .await
    .expect("reconcile managed aliases");
    let superseded_alias_target =
        std::fs::read_link(&first_alias).expect("first superseded alias target");
    assert_eq!(superseded_alias_target, old_artifact);
    assert_eq!(
        std::fs::read_link(&second_alias).expect("second superseded alias target"),
        superseded_alias_target
    );
    let state_home = artifact_root
        .parent()
        .and_then(std::path::Path::parent)
        .expect("Runtime artifact root belongs to State Home");
    let activation = agent_semantic_artifacts::runtime_artifact_activation::read_runtime_artifact_activation_event(state_home)
        .await
        .expect("read Runtime artifact activation")
        .expect("pending Runtime artifact activation");
    assert!(
        activation.artifact_path.starts_with(&artifact_root),
        "pending activation candidate must be content-addressed: {}",
        activation.artifact_path.display()
    );
    assert_ne!(
        activation.artifact_path, superseded_alias_target,
        "pending publication must not switch serving aliases before actor commit"
    );
    assert_eq!(
        std::fs::canonicalize(&primary_target).expect("canonical client launcher target"),
        std::fs::canonicalize(&activation.artifact_path)
            .expect("canonical pending activation artifact"),
        "normal client bootstrap must execute the pending immutable candidate"
    );
    assert_eq!(
        std::fs::read_link(&primary_target).expect("read Runtime compatibility alias"),
        root.join("home/.local/bin/asp"),
        "Runtime compatibility alias must point to the PATH-visible install entry"
    );
    assert_ne!(
        std::fs::read_link(root.join("home/.local/bin/asp"))
            .expect("read PATH-visible install entry"),
        primary_target,
        "PATH-visible install entry must never point back to the Runtime alias"
    );
    assert!(!activation.publication_nonce.is_empty());
    assert_eq!(
        std::fs::read(unrelated).expect("read unrelated binary"),
        b"unrelated"
    );

    std::fs::remove_dir_all(root).expect("remove alias reconciliation fixture");
}

#[test]
fn protocol_binary_entries_put_the_runtime_alias_behind_the_path_visible_install() {
    let root = std::env::temp_dir().join(format!(
        "asp-canonical-runtime-target-{}",
        std::process::id()
    ));
    let state_home = root.join("state-home");
    let artifact_root = state_home.join("runtime/artifacts");
    let user_home = root.join("home");
    let entries = super::resolve_protocol_binary_install_entries(None, &artifact_root, &user_home)
        .expect("canonical protocol binary entries");
    assert_eq!(entries.path_visible, user_home.join(".local/bin/asp"));
    assert_eq!(entries.runtime_alias, state_home.join("runtime/bin/asp"));
    assert_ne!(entries.path_visible, entries.runtime_alias);

    let explicit = root.join("explicit-bin");
    let explicit_entries =
        super::resolve_protocol_binary_install_entries(Some(&explicit), &artifact_root, &user_home)
            .expect("explicit protocol binary entries");
    assert_eq!(explicit_entries.path_visible, explicit.join("asp"));
    assert_eq!(explicit_entries.runtime_alias, entries.runtime_alias);
}

#[cfg(unix)]
#[test]
fn symlinked_parent_spellings_are_one_protocol_binary_entry() {
    use std::os::unix::fs::symlink;

    let root =
        std::env::temp_dir().join(format!("asp-canonical-entry-parent-{}", std::process::id()));
    let canonical_parent = root.join("canonical/bin");
    std::fs::create_dir_all(&canonical_parent).expect("create canonical binary parent");
    let alias_root = root.join("alias");
    symlink(root.join("canonical"), &alias_root).expect("link alternate runtime spelling");

    assert!(super::same_protocol_binary_entry(
        &canonical_parent.join("asp"),
        &alias_root.join("bin/asp")
    ));

    std::fs::remove_dir_all(root).expect("remove canonical entry fixture");
}
