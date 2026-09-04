use super::merge_immutable_tree;
use super::normalize_source_snapshot_envelope_tree;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[test]
fn migration_preserves_provider_versions_and_rewrites_cas_root() {
    let root = temp_root("provider-version-migration");
    let canonical = root.join("canonical/source-snapshot-envelopes");
    let legacy = root.join("legacy/source-snapshot-envelopes");
    let canonical_cas = root.join("canonical/source-blob-cas/v1");
    write_flat_envelope(&canonical, "digest-a", "/old/canonical/cas");
    write_flat_envelope(&legacy, "digest-b", "/old/legacy/cas");

    normalize_source_snapshot_envelope_tree(&canonical, &canonical_cas)
        .expect("normalize canonical envelopes");
    normalize_source_snapshot_envelope_tree(&legacy, &canonical_cas)
        .expect("normalize legacy envelopes");
    merge_immutable_tree(&canonical, &legacy).expect("merge digest-qualified envelopes");

    let snapshot_dir = legacy.join("v1/snapshot-root");
    for digest in ["digest-a", "digest-b"] {
        let envelope_path = snapshot_dir.join(format!("asp-rust--{digest}.json"));
        let envelope: serde_json::Value =
            serde_json::from_slice(&fs::read(&envelope_path).expect("read migrated envelope"))
                .expect("parse migrated envelope");
        assert_eq!(
            envelope["casRoot"],
            canonical_cas.display().to_string(),
            "{envelope_path:?}"
        );
    }
    assert!(!snapshot_dir.join("asp-rust.json").exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn populated_canonical_workspace_retires_legacy_mutable_state() {
    let root = temp_root("populated-canonical");
    let checkout = root.join("checkout");
    fs::create_dir_all(&checkout).expect("create checkout fixture");
    run_git(&checkout, &["init"]);
    run_git(
        &checkout,
        &[
            "remote",
            "add",
            "origin",
            "git@github.com:tao3k/state-migration-fixture.git",
        ],
    );

    let state_home = root.join("state");
    let state = crate::state_core::ResolvedState::resolve_with_state_home(&checkout, &state_home)
        .expect("resolve canonical state");
    state
        .ensure_minimal_layout()
        .expect("materialize canonical state");
    fs::write(state.paths.client_dir.join("facts.turso"), b"canonical")
        .expect("write canonical database fixture");
    write_flat_envelope(
        &state
            .paths
            .workspace_dir
            .join("live/client/source-snapshot-envelopes"),
        "digest-a",
        "/old/canonical/cas",
    );

    let (legacy_repo_id, legacy_workspace_id) = state
        .legacy_identity_candidates()
        .into_iter()
        .next()
        .expect("legacy path identity candidate");
    let legacy_workspace_dir = state.legacy_workspace_dir(&legacy_repo_id, &legacy_workspace_id);
    let legacy_client_dir = legacy_workspace_dir.join("live/client");
    fs::create_dir_all(&legacy_client_dir).expect("create legacy client fixture");
    fs::write(legacy_client_dir.join("facts.turso"), b"legacy")
        .expect("write legacy database fixture");
    let legacy_blob_path = legacy_client_dir.join("source-blob-cas/v1/aa/blob");
    fs::create_dir_all(legacy_blob_path.parent().expect("legacy blob parent"))
        .expect("create legacy CAS fixture");
    fs::write(&legacy_blob_path, b"immutable").expect("write legacy CAS fixture");
    write_flat_envelope(
        &legacy_client_dir.join("source-snapshot-envelopes"),
        "digest-b",
        "/old/legacy/cas",
    );
    let legacy_artifact = legacy_workspace_dir.join("artifacts/prompt-output/legacy.txt");
    fs::create_dir_all(legacy_artifact.parent().expect("legacy artifact parent"))
        .expect("create legacy artifact fixture");
    fs::write(&legacy_artifact, b"legacy-artifact").expect("write legacy artifact fixture");

    state
        .ensure_minimal_layout()
        .expect("merge populated canonical and legacy state");

    assert_eq!(
        fs::read(state.paths.client_dir.join("facts.turso"))
            .expect("read canonical active database"),
        b"canonical"
    );
    assert_eq!(
        fs::read(state.paths.client_dir.join("source-blob-cas/v1/aa/blob"))
            .expect("read migrated immutable CAS blob"),
        b"immutable"
    );
    let snapshot_dir = state
        .paths
        .client_dir
        .join("source-snapshot-envelopes/v1/snapshot-root");
    assert!(snapshot_dir.join("asp-rust--digest-a.json").is_file());
    assert!(snapshot_dir.join("asp-rust--digest-b.json").is_file());

    let retired_workspace_dir = state
        .paths
        .workspace_dir
        .join(".state/migrations/project-identity-v1/retired-workspaces")
        .join(format!(
            "{}--{}",
            legacy_repo_id.as_str(),
            legacy_workspace_id.as_str()
        ));
    assert_eq!(
        fs::read(retired_workspace_dir.join("live/client/facts.turso"))
            .expect("read retired legacy database"),
        b"legacy"
    );
    assert_eq!(
        fs::read(retired_workspace_dir.join("artifacts/prompt-output/legacy.txt"))
            .expect("read retired legacy artifact"),
        b"legacy-artifact"
    );
    let receipt: serde_json::Value = serde_json::from_slice(
        &fs::read(retired_workspace_dir.join(".state/migrations/project-identity-v1.json"))
            .expect("read identity migration receipt"),
    )
    .expect("parse identity migration receipt");
    assert_eq!(receipt["status"], "retired-read-disabled");
    assert!(!legacy_workspace_dir.exists());

    let duplicate_legacy_client_dir = legacy_workspace_dir.join("live/client");
    fs::create_dir_all(&duplicate_legacy_client_dir)
        .expect("recreate interrupted legacy client fixture");
    fs::write(duplicate_legacy_client_dir.join("facts.turso"), b"legacy")
        .expect("write duplicate legacy database fixture");
    let duplicate_legacy_artifact = legacy_workspace_dir.join("artifacts/prompt-output/legacy.txt");
    fs::create_dir_all(
        duplicate_legacy_artifact
            .parent()
            .expect("duplicate legacy artifact parent"),
    )
    .expect("recreate duplicate legacy artifact parent");
    fs::write(&duplicate_legacy_artifact, b"legacy-artifact")
        .expect("write duplicate legacy artifact fixture");
    let duplicate_receipt = legacy_workspace_dir.join(".state/migrations/project-identity-v1.json");
    fs::create_dir_all(
        duplicate_receipt
            .parent()
            .expect("duplicate migration receipt parent"),
    )
    .expect("recreate duplicate migration receipt parent");
    fs::copy(
        retired_workspace_dir.join(".state/migrations/project-identity-v1.json"),
        &duplicate_receipt,
    )
    .expect("copy duplicate migration receipt");

    state
        .ensure_minimal_layout()
        .expect("identical interrupted migration converges");
    assert!(!legacy_workspace_dir.exists());

    fs::create_dir_all(&duplicate_legacy_client_dir)
        .expect("recreate conflicting legacy client fixture");
    fs::write(
        duplicate_legacy_client_dir.join("facts.turso"),
        b"conflicting-legacy",
    )
    .expect("write conflicting legacy database fixture");
    state
        .ensure_minimal_layout()
        .expect("different retired state moves to a content-addressed generation");
    assert!(!legacy_workspace_dir.exists());
    let retired_parent = retired_workspace_dir
        .parent()
        .expect("retired workspace parent");
    let retired_prefix = format!(
        "{}--sha256-",
        retired_workspace_dir
            .file_name()
            .and_then(|name| name.to_str())
            .expect("retired workspace file name")
    );
    let generations = fs::read_dir(retired_parent)
        .expect("read retired workspace generations")
        .map(|entry| entry.expect("read retired workspace generation").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&retired_prefix))
        })
        .collect::<Vec<_>>();
    assert_eq!(generations.len(), 1);
    assert_eq!(
        fs::read(generations[0].join("live/client/facts.turso"))
            .expect("read conflicting retired generation"),
        b"conflicting-legacy"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn populated_canonical_hooks_retire_legacy_non_event_state() {
    let root = temp_root("populated-canonical-hooks");
    let checkout = root.join("checkout");
    fs::create_dir_all(&checkout).expect("create checkout fixture");
    run_git(&checkout, &["init"]);
    run_git(
        &checkout,
        &[
            "remote",
            "add",
            "origin",
            "git@github.com:tao3k/state-migration-fixture.git",
        ],
    );

    let state_home = root.join("state");
    let state = crate::state_core::ResolvedState::resolve_with_state_home(&checkout, &state_home)
        .expect("resolve canonical state");
    state
        .ensure_minimal_layout()
        .expect("materialize canonical state");
    let canonical_activation = state.paths.hooks_dir.join("state/activation.json");
    fs::create_dir_all(
        canonical_activation
            .parent()
            .expect("canonical activation parent"),
    )
    .expect("create canonical hook state");
    fs::write(&canonical_activation, b"canonical-activation").expect("write canonical activation");

    let (legacy_repo_id, legacy_workspace_id) = state
        .legacy_identity_candidates()
        .into_iter()
        .next()
        .expect("legacy path identity candidate");
    let legacy_hooks_dir = state.legacy_hooks_dir(&legacy_repo_id, &legacy_workspace_id);
    let legacy_hook_state = legacy_hooks_dir.join("state");
    fs::create_dir_all(&legacy_hook_state).expect("create legacy hook state");
    fs::write(
        legacy_hook_state.join("activation.json"),
        b"legacy-activation",
    )
    .expect("write legacy activation");
    fs::write(
        legacy_hook_state.join("active-asp-artifact-receipt.v1.json"),
        b"legacy-artifact-receipt",
    )
    .expect("write legacy artifact receipt");

    state
        .ensure_minimal_layout()
        .expect("retire legacy non-event hook state");

    assert_eq!(
        fs::read(&canonical_activation).expect("read canonical activation"),
        b"canonical-activation"
    );
    let retired_hooks_dir = state
        .paths
        .hooks_dir
        .join(".state/migrations/project-identity-v1/retired-hook-state")
        .join(format!(
            "{}--{}",
            legacy_repo_id.as_str(),
            legacy_workspace_id.as_str()
        ));
    assert_eq!(
        fs::read(retired_hooks_dir.join("state/activation.json"))
            .expect("read retired legacy activation"),
        b"legacy-activation"
    );
    assert_eq!(
        fs::read(retired_hooks_dir.join("state/active-asp-artifact-receipt.v1.json"))
            .expect("read retired legacy artifact receipt"),
        b"legacy-artifact-receipt"
    );
    let receipt: serde_json::Value = serde_json::from_slice(
        &fs::read(retired_hooks_dir.join(".state/migrations/hook-project-tree-v1.json"))
            .expect("read retired hook state receipt"),
    )
    .expect("parse retired hook state receipt");
    assert_eq!(receipt["status"], "retired-read-disabled");
    assert!(!legacy_hooks_dir.exists());

    let duplicate_legacy_hook_state = legacy_hooks_dir.join("state");
    fs::create_dir_all(&duplicate_legacy_hook_state)
        .expect("recreate interrupted legacy hook fixture");
    fs::write(
        duplicate_legacy_hook_state.join("activation.json"),
        b"legacy-activation",
    )
    .expect("write duplicate legacy activation");
    fs::write(
        duplicate_legacy_hook_state.join("active-asp-artifact-receipt.v1.json"),
        b"legacy-artifact-receipt",
    )
    .expect("write duplicate legacy artifact receipt");
    let duplicate_hook_receipt =
        legacy_hooks_dir.join(".state/migrations/hook-project-tree-v1.json");
    fs::create_dir_all(
        duplicate_hook_receipt
            .parent()
            .expect("duplicate hook receipt parent"),
    )
    .expect("recreate duplicate hook receipt parent");
    fs::copy(
        retired_hooks_dir.join(".state/migrations/hook-project-tree-v1.json"),
        &duplicate_hook_receipt,
    )
    .expect("copy duplicate hook receipt");

    state
        .ensure_minimal_layout()
        .expect("identical interrupted hook migration converges");
    assert!(!legacy_hooks_dir.exists());

    let _ = fs::remove_dir_all(root);
}

fn write_flat_envelope(root: &std::path::Path, provider_digest: &str, cas_root: &str) {
    let snapshot_dir = root.join("v1/snapshot-root");
    fs::create_dir_all(&snapshot_dir).expect("create snapshot envelope fixture");
    fs::write(
        snapshot_dir.join("asp-rust.json"),
        serde_json::to_vec_pretty(&json!({
            "schemaId": "asp.exact-source-snapshot-envelope.v1",
            "schemaVersion": "1",
            "providerId": "asp-rust",
            "sourceSnapshot": {
                "schemaId": "asp.source-snapshot.v1",
                "algorithm": "blake3-merkle-v1",
                "rootDigest": "snapshot-root",
                "sourceKind": "filesystem",
                "leafCount": 1,
                "providerDigest": provider_digest
            },
            "casRoot": cas_root,
            "owners": []
        }))
        .expect("serialize snapshot envelope fixture"),
    )
    .expect("write snapshot envelope fixture");
}

fn temp_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "asp-state-migration-{label}-{}-{nonce}",
        std::process::id()
    ))
}

fn run_git(cwd: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .expect("run git fixture command");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}
