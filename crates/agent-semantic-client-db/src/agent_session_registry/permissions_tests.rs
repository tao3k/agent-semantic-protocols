use super::{prepare_private_registry_path, verify_owner_ids};
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_registry_path(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir()
        .join(format!(
            "asp-registry-permissions-{name}-{}-{nonce}",
            std::process::id()
        ))
        .join("agent-sessions.db")
}

#[test]
fn registry_creation_enforces_private_directory_and_file_modes() {
    let db_path = temp_registry_path("private");
    let parent = db_path.parent().expect("parent");
    fs::create_dir_all(parent).expect("create fixture directory");
    fs::set_permissions(parent, fs::Permissions::from_mode(0o777)).expect("relax fixture mode");

    prepare_private_registry_path(&db_path).expect("prepare private registry");

    assert_eq!(
        fs::metadata(parent)
            .expect("parent metadata")
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(&db_path)
            .expect("db metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let _ = fs::remove_dir_all(parent);
}

#[test]
fn registry_creation_diagnoses_symlink_without_replacing_ownership() {
    let db_path = temp_registry_path("symlink");
    let parent = db_path.parent().expect("parent");
    fs::create_dir_all(parent).expect("create fixture directory");
    let target = parent.join("target.db");
    fs::write(&target, []).expect("create target");
    symlink(&target, &db_path).expect("create registry symlink");

    let error = prepare_private_registry_path(&db_path).expect_err("symlink must be rejected");
    assert!(error.contains("registryWriteStatus=denied"), "{error}");
    assert!(error.contains("symlink-not-authoritative"), "{error}");
    let _ = fs::remove_dir_all(parent);
}

#[test]
fn registry_creation_reports_foreign_owner_without_chown() {
    let path = PathBuf::from("/synthetic/agent-sessions.db");
    let error = verify_owner_ids(&path, 41, 42).expect_err("foreign owner must be rejected");
    assert!(error.contains("registryWriteStatus=denied"), "{error}");
    assert!(error.contains("reason=foreign-owner"), "{error}");
    assert!(error.contains("refusing-to-chown"), "{error}");
}
