use std::fs;
use std::path::Path;

use super::{WorkspaceLaunchDescriptor, artifact_snapshot, copy_artifact_root, launcher_script};

#[test]
fn workspace_artifact_snapshot_covers_sibling_runtime_tree() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let artifact = temporary.path().join("artifact");
    fs::create_dir_all(artifact.join("bin")).expect("bin directory");
    fs::create_dir_all(artifact.join("lib/gslph/src")).expect("lib directory");
    fs::write(artifact.join("bin/gslph"), b"launcher").expect("launcher");
    fs::write(artifact.join("lib/gslph/src/cli.ssi"), b"module-v1").expect("module");

    let (before, leaf_count) = artifact_snapshot(&artifact).expect("snapshot");
    assert_eq!(leaf_count, 2);
    fs::write(artifact.join("lib/gslph/src/cli.ssi"), b"module-v2").expect("module update");
    let (after, _) = artifact_snapshot(&artifact).expect("updated snapshot");
    assert_ne!(
        before, after,
        "sibling lib bytes must affect artifact identity"
    );
}

#[test]
fn workspace_artifact_snapshot_single_file_uses_normalized_filename_leaf() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let artifact = temporary.path().join("provider.bin");
    fs::write(&artifact, b"provider").expect("provider artifact");

    let (first_digest, first_leaf_count) = artifact_snapshot(&artifact).expect("snapshot");
    let (second_digest, second_leaf_count) = artifact_snapshot(&artifact).expect("snapshot");

    assert_eq!(first_leaf_count, 1);
    assert_eq!(second_leaf_count, 1);
    assert!(!first_digest.is_empty());
    assert_eq!(first_digest, second_digest);
}

#[test]
fn workspace_artifact_copy_preserves_tree_identity() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let source = temporary.path().join("source");
    let target = temporary.path().join("target");
    fs::create_dir_all(source.join("bin")).expect("bin directory");
    fs::create_dir_all(source.join("lib/runtime")).expect("lib directory");
    fs::write(source.join("bin/provider"), b"provider").expect("provider");
    fs::write(source.join("lib/runtime/module"), b"module").expect("module");

    copy_artifact_root(&source, &target).expect("copy artifact tree");

    assert_eq!(
        artifact_snapshot(&source).expect("source snapshot"),
        artifact_snapshot(&target).expect("target snapshot")
    );
    assert!(target.join("bin/provider").is_file());
    assert!(target.join("lib/runtime/module").is_file());
}

#[test]
fn workspace_launcher_resolves_provider_relative_program_and_arguments() {
    let artifact_root = Path::new("/immutable/provider artifact/root");
    let entrypoint = artifact_root.join("bin/py-harness");
    let launch = WorkspaceLaunchDescriptor {
        program: "bin/python3".to_string(),
        args: vec!["bin/py-harness".to_string()],
        program_relative_to_artifact: true,
        args_relative_to_artifact: true,
    };

    let script = launcher_script(artifact_root, &entrypoint, Some(&launch)).expect("launcher");

    assert_eq!(
        script,
        "#!/bin/sh\nexec '/immutable/provider artifact/root/bin/python3' '/immutable/provider artifact/root/bin/py-harness' \"$@\"\n"
    );
}
