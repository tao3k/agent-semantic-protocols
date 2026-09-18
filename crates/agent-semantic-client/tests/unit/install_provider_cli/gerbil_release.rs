// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::process::Command;

use super::create_fake_curl_bin;
use super::create_gerbil_pinned_release_fixture;
use super::create_gerbil_script_release_fixture;
use super::prepend_path;
use super::temp_project_root;

#[test]
#[cfg(unix)]
fn install_language_gerbil_rejects_an_unpinned_native_fixture() {
    let root = temp_project_root();
    let home = root.join("home");
    let release_dir = create_gerbil_pinned_release_fixture(&root);
    let fake_bin = create_fake_curl_bin(&root);

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "install",
            "language",
            "gerbil-scheme",
            "--target",
            "x86_64-unknown-linux-gnu",
        ])
        .env("HOME", &home)
        .env("PATH", prepend_path(&fake_bin))
        .env("ASP_TEST_RELEASE_DIR", &release_dir)
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("run asp install language gerbil-scheme");

    assert!(
        !output.status.success(),
        "unpinned native fixture was installed"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("checksum mismatch"), "{stderr}");
    assert!(
        !home
            .join(".agent-semantic-protocols/runtime/bin/asp-gerbil-scheme")
            .exists()
    );
}

#[test]
#[cfg(unix)]
fn install_language_gerbil_rejects_an_unpinned_script_before_execution() {
    let root = temp_project_root();
    let home = root.join("home");
    let release_dir = create_gerbil_script_release_fixture(&root);
    let fake_bin = create_fake_curl_bin(&root);

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "install",
            "language",
            "gerbil-scheme",
            "--target",
            "x86_64-unknown-linux-gnu",
        ])
        .env("HOME", &home)
        .env("PATH", prepend_path(&fake_bin))
        .env("ASP_TEST_RELEASE_DIR", &release_dir)
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("run asp install language gerbil-scheme");

    assert!(
        !output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let output_text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output_text.contains("checksum mismatch"), "{output_text}");
    assert!(
        !home
            .join(".agent-semantic-protocols/runtime/bin/asp-gerbil-scheme")
            .exists(),
        "script payload must not be installed as asp-gerbil-scheme"
    );
}
