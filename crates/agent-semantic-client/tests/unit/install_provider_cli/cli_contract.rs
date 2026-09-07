// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::process::Command;

#[test]
fn install_language_help_separates_locked_release_from_develop_mode() {
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args(["install", "language", "--help"])
        .output()
        .expect("run asp install language --help");

    assert!(output.status.success());
    let receipt = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(receipt.contains("Install a language provider"), "{receipt}");
    assert!(receipt.contains("--target <TARGET>"), "{receipt}");
    assert!(
        !receipt.contains("--from-workspace"),
        "develop installs are selected by state-home [dev].root: {receipt}"
    );
    assert!(
        !receipt.contains("--record-installed-receipt"),
        "the Justfile-only receipt bridge must stay off the downstream install surface: {receipt}"
    );
    assert!(
        !receipt.contains("state=locked-release-unavailable"),
        "help must not be resolved as a language release: {receipt}"
    );
}

#[test]
fn install_language_usage_separates_locked_release_from_develop_mode() {
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args(["install", "language"])
        .output()
        .expect("run asp install language without a language id");

    assert!(!output.status.success());
    let receipt = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        receipt.contains("release mode: plain `asp install language` resolves only the locked release artifact (installMode=locked-release)"),
        "{receipt}"
    );
    assert!(
        receipt.contains("develop mode: plain `asp install language` delegates to the development installer under [dev].root"),
        "{receipt}"
    );
    assert!(
        receipt.contains("[dev].root owns provider builds and installation"),
        "{receipt}"
    );
    assert!(
        !receipt.contains("[--from-workspace]"),
        "the internal workspace switch must not be advertised as the normal install surface: {receipt}"
    );
}
