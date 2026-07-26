use super::ProtocolBinaryReconciliationGuard;

#[test]
fn global_reconciliation_lock_rejects_concurrent_writer_without_waiting() {
    let protocol_home = std::env::temp_dir().join(format!(
        "asp-global-reconciliation-lock-{}",
        std::process::id()
    ));
    let first =
        ProtocolBinaryReconciliationGuard::acquire(&protocol_home).expect("first writer lock");
    let contender_home = protocol_home.clone();
    let contender = std::thread::spawn(move || {
        let started = std::time::Instant::now();
        let error = ProtocolBinaryReconciliationGuard::acquire(&contender_home)
            .err()
            .expect("concurrent writer must be rejected");
        (started.elapsed(), error)
    })
    .join()
    .expect("concurrent writer thread");

    assert!(
        contender.0 < std::time::Duration::from_millis(50),
        "lock contention must fail in milliseconds: elapsed={:?}",
        contender.0
    );
    assert!(
        contender
            .1
            .contains("global ASP reconciliation is already active")
    );

    drop(first);
    ProtocolBinaryReconciliationGuard::acquire(&protocol_home)
        .expect("lock must be reusable after commit");
    std::fs::remove_dir_all(protocol_home).expect("remove reconciliation lock fixture");
}

#[cfg(unix)]
#[test]
fn global_reconciliation_atomically_updates_every_managed_path_alias() {
    use std::os::unix::fs::symlink;

    let root = std::env::temp_dir().join(format!(
        "asp-managed-path-alias-reconciliation-{}",
        std::process::id()
    ));
    let artifact_root = root.join("artifacts");
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

    let path_dirs = vec![
        first_alias
            .parent()
            .expect("first alias parent")
            .to_path_buf(),
        second_alias
            .parent()
            .expect("second alias parent")
            .to_path_buf(),
        unrelated.parent().expect("unrelated parent").to_path_buf(),
    ];
    let managed =
        super::managed_protocol_binary_path_aliases(&artifact_root, &primary_target, &path_dirs)
            .expect("discover managed aliases");
    assert_eq!(managed, vec![first_alias.clone(), second_alias.clone()]);

    let started = std::time::Instant::now();
    super::ensure_protocol_binary_installed(&super::ProtocolBinaryInstallPlan {
        current_exe: source,
        target: primary_target.clone(),
        artifact_root: artifact_root.clone(),
        managed_path_aliases: managed,
    })
    .expect("reconcile managed aliases");
    assert!(
        started.elapsed() < std::time::Duration::from_millis(50),
        "managed alias reconciliation must complete in milliseconds: elapsed={:?}",
        started.elapsed()
    );
    assert_eq!(
        std::fs::read_link(first_alias).expect("first alias target"),
        primary_target
    );
    assert_eq!(
        std::fs::read_link(second_alias).expect("second alias target"),
        primary_target
    );
    assert_eq!(
        std::fs::read(unrelated).expect("read unrelated binary"),
        b"unrelated"
    );

    std::fs::remove_dir_all(root).expect("remove alias reconciliation fixture");
}

#[test]
fn install_target_rejects_an_unmanaged_path_binary() {
    let root =
        std::env::temp_dir().join(format!("asp-unmanaged-path-binary-{}", std::process::id()));
    let artifact_root = root.join("artifacts");
    std::fs::create_dir_all(&artifact_root).expect("create artifact root");
    let current = root.join("build/asp");
    std::fs::create_dir_all(current.parent().expect("current parent")).expect("create current dir");
    std::fs::write(&current, b"current").expect("write current binary");
    let unrelated = root.join("path/asp");
    std::fs::create_dir_all(unrelated.parent().expect("unrelated parent"))
        .expect("create unrelated dir");
    std::fs::write(&unrelated, b"unrelated").expect("write unrelated binary");

    let error = super::resolve_protocol_binary_install_target(
        &current,
        None,
        &[unrelated.parent().expect("unrelated parent").to_path_buf()],
        &artifact_root,
    )
    .expect_err("unmanaged PATH binary must be rejected");
    assert!(error.contains("refusing to update unrelated PATH binary"));

    std::fs::remove_dir_all(root).expect("remove unmanaged binary fixture");
}
