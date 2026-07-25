use std::process::Command;

use super::{
    create_fake_curl_bin, create_gerbil_pinned_release_fixture,
    create_gerbil_script_release_fixture, prepend_path, provider_package_path, receipt_path,
    sorted_file_names, temp_project_root,
};

#[test]
#[cfg(unix)]
fn install_language_gerbil_uses_release_asset_prefix_and_installs_gslph() {
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
        .arg("--project")
        .arg(&root)
        .env("HOME", &home)
        .env("PATH", prepend_path(&fake_bin))
        .env("ASP_TEST_RELEASE_DIR", &release_dir)
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("run asp install language gerbil-scheme");

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let runtime_bin = home.join(".agent-semantic-protocols/runtime/bin");
    let bin = runtime_bin.join("gslph");
    let lock_path = receipt_path(&stdout, "lock");
    let lock = std::fs::read_to_string(&lock_path).expect("read Gerbil lock");
    let package_binary = provider_package_path(&lock).join("bin/gerbil-scheme-harness");
    assert!(bin.is_file(), "missing installed gslph {}", bin.display());
    assert!(
        package_binary.is_file(),
        "missing Gerbil package binary {}",
        package_binary.display()
    );
    assert!(
        !std::fs::symlink_metadata(&bin)
            .expect("stat installed gslph")
            .file_type()
            .is_symlink(),
        "installed provider command must be a binary file, not a symlink"
    );
    assert!(
        std::fs::read(&bin)
            .expect("read installed Gerbil provider")
            .starts_with(b"\x7FELF"),
        "installed Gerbil provider must be a native binary release payload"
    );
    let runtime_bin_entries = sorted_file_names(&runtime_bin);
    assert_eq!(
        runtime_bin_entries,
        vec!["gslph".to_string()],
        "provider install must not copy package companions or build artifacts into the State Home runtime bin"
    );
    assert!(lock.contains("binary = \"gslph\""), "{lock}");
    assert!(lock.contains(
        "source = \"https://github.com/tao3k/gerbil-scheme-language-project-harness/releases/download/v0.1.0/gerbil-scheme-harness-x86_64-unknown-linux-gnu.tar.gz\""
    ), "{lock}");
}

#[test]
#[cfg(unix)]
fn install_language_gerbil_rejects_script_release_payload() {
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
        .arg("--project")
        .arg(&root)
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
    assert!(
        output_text.contains("is not a native executable"),
        "{output_text}"
    );
    assert!(
        !home
            .join(".agent-semantic-protocols/runtime/bin/gslph")
            .exists(),
        "script payload must not be installed as gslph"
    );
}
