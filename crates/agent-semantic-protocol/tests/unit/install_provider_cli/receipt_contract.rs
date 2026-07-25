use std::fs;
use std::process::Command;

use super::{make_executable, receipt_path, temp_project_root};

#[cfg(unix)]
#[test]
fn root_justfile_binary_can_record_and_reconcile_a_develop_receipt_without_release_download() {
    let root = temp_project_root();
    let state_home = root.join("state");
    let installed = state_home.join("runtime/bin/orgize");
    fs::create_dir_all(installed.parent().expect("runtime bin parent"))
        .expect("create runtime bin");
    fs::write(&installed, b"#!/bin/sh\nexit 0\n").expect("write provider fixture");
    make_executable(&installed);

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "install",
            "language",
            "org",
            "--project",
            root.to_str().expect("utf-8 project root"),
            "--record-installed-receipt",
            installed.to_str().expect("utf-8 installed path"),
        ])
        .env("ASP_STATE_HOME", &state_home)
        .env("ASP_NO_AGENT_PLATFORM", "1")
        .output()
        .expect("record installed provider receipt");
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt = String::from_utf8_lossy(&output.stdout);
    assert!(
        receipt.contains("installMode=record-installed-receipt"),
        "{receipt}"
    );
    assert!(
        receipt.contains("sourceKind=develop-root-justfile"),
        "{receipt}"
    );
    let lock_path = receipt_path(&receipt, "lock");
    let lock = fs::read_to_string(&lock_path).expect("read provider lock");
    assert!(lock.contains("schemaId = \"asp.provider-install-lock.v1\""));
    assert!(lock.contains("language = \"org\""));
    assert!(lock.contains("provider = \"orgize\""));
    assert!(lock.contains("sourceKind = \"develop-root-justfile\""));
    let lock_value: toml::Value = toml::from_str(&lock).expect("parse provider lock");
    for field in [
        "installedEntrypointDigest",
        "installedEntrypointMetadataDigest",
    ] {
        let digest = lock_value
            .get(field)
            .and_then(toml::Value::as_str)
            .unwrap_or_else(|| panic!("missing {field} in provider lock"));
        assert_eq!(digest.len(), 64, "unexpected {field}: {digest}");
        assert!(
            digest.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "unexpected {field}: {digest}"
        );
    }

    let reconcile = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "install",
            "language",
            "org",
            "--project",
            root.to_str().expect("utf-8 project root"),
            "--reconcile-receipt",
        ])
        .env("ASP_STATE_HOME", &state_home)
        .env("ASP_NO_AGENT_PLATFORM", "1")
        .output()
        .expect("reconcile installed provider receipt");
    assert!(
        reconcile.status.success(),
        "{}{}",
        String::from_utf8_lossy(&reconcile.stdout),
        String::from_utf8_lossy(&reconcile.stderr)
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn record_and_reconcile_receipt_modes_are_mutually_exclusive() {
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "install",
            "language",
            "org",
            "--record-installed-receipt",
            "orgize",
            "--reconcile-receipt",
        ])
        .env("ASP_NO_AGENT_PLATFORM", "1")
        .output()
        .expect("run conflicting receipt modes");

    assert!(!output.status.success());
    let receipt = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        receipt.contains(
            "the argument '--record-installed-receipt <BINARY>' cannot be used with '--reconcile-receipt'"
        ) || receipt
            .contains("--reconcile-receipt conflicts with --record-installed-receipt"),
        "{receipt}"
    );
}
