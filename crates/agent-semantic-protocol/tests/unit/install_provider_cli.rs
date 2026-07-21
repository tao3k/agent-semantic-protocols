use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

#[test]
#[cfg(unix)]
fn install_language_pinned_release_writes_runtime_bin_package_and_lock() {
    assert_install_pinned_release_writes_runtime_bin_package_and_lock();
}

#[test]
#[cfg(unix)]
fn install_language_from_workspace_refreshes_home_local_bin() {
    let root = temp_project_root();
    let home = root.join("home");
    let workspace_provider =
        root.join("languages/rust-lang-project-harness/target/release/rs-harness");
    std::fs::create_dir_all(
        workspace_provider
            .parent()
            .expect("workspace provider parent"),
    )
    .expect("create workspace provider parent");
    std::fs::write(&workspace_provider, b"workspace-dev-provider\n")
        .expect("write workspace provider");
    let fake_cargo =
        create_fake_tool_bin(&root, "cargo", concat!("#!/usr/bin/env sh\n", "exit 0\n",));

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "install",
            "language",
            "rust",
            "--from-workspace",
            "--target",
            "x86_64-unknown-linux-gnu",
        ])
        .arg("--project")
        .arg(&root)
        .env("HOME", &home)
        .env(
            "PATH",
            format!(
                "{}:{}",
                fake_cargo.parent().expect("fake cargo parent").display(),
                std::env::var("PATH").expect("PATH")
            ),
        )
        .env_remove("PRJ_CACHE_HOME")
        .env_remove("SEMANTIC_AGENT_BIN_DIR")
        .output()
        .expect("run asp install language --from-workspace");

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("installMode=develop-workspace"), "{stdout}");
    assert!(stdout.contains("source=workspace-build"), "{stdout}");
    assert!(
        stdout.contains(&format!(
            "workspaceArtifact={}",
            workspace_provider.display()
        )),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!(
            "workspaceEntrypoint={}",
            workspace_provider.display()
        )),
        "{stdout}"
    );
    assert!(
        stdout.contains("installTargetSource=home-local-bin"),
        "{stdout}"
    );

    let installed = home.join(".local/bin/rs-harness");
    assert_eq!(
        std::fs::read(&installed).expect("read installed workspace provider"),
        b"workspace-dev-provider\n"
    );
    let runtime_artifact = home.join(".agent-semantic-protocols/runtime/bin/rs-harness");
    assert_eq!(
        std::fs::read(&runtime_artifact).expect("read installed runtime artifact"),
        b"workspace-dev-provider\n"
    );
}

#[test]
fn install_language_usage_separates_locked_release_from_develop_mode() {
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args(["install", "language"])
        .env("ASP_NO_AGENT_PLATFORM", "1")
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
        receipt.contains("develop mode: use the repository Justfile recipes"),
        "{receipt}"
    );
    assert!(
        !receipt.contains("[--from-workspace]"),
        "the internal workspace switch must not be advertised as the normal install surface: {receipt}"
    );
}

#[test]
fn install_language_help_separates_locked_release_from_develop_mode() {
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args(["install", "language", "--help"])
        .env("ASP_NO_AGENT_PLATFORM", "1")
        .output()
        .expect("run asp install language --help");

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
        receipt.contains("develop mode: use the repository Justfile recipes"),
        "{receipt}"
    );
    assert!(
        !receipt.contains("state=locked-release-unavailable"),
        "help must not be resolved as a language release: {receipt}"
    );
}

#[test]
fn install_language_without_pinned_release_reports_locked_release_unavailable() {
    let root = temp_project_root();
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "install",
            "language",
            "md",
            "--target",
            "x86_64-unknown-linux-gnu",
        ])
        .arg("--project")
        .arg(&root)
        .env("ASP_NO_AGENT_PLATFORM", "1")
        .output()
        .expect("run locked release install for an unpinned language");

    assert!(!output.status.success());
    let receipt = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        receipt.contains("state=locked-release-unavailable"),
        "{receipt}"
    );
    assert!(receipt.contains("installMode=locked-release"), "{receipt}");
    assert!(receipt.contains("reason=language-not-pinned"), "{receipt}");
    assert!(receipt.contains("language=md"), "{receipt}");
}

#[test]
#[cfg(unix)]
fn install_typescript_from_workspace_uses_built_provider_entrypoint() {
    let root = temp_project_root();
    let home = root.join("home");
    let workspace_provider =
        root.join("languages/typescript-lang-project-harness/dist/provider/ts-harness.mjs");
    std::fs::create_dir_all(
        workspace_provider
            .parent()
            .expect("workspace provider parent"),
    )
    .expect("create workspace provider parent");
    std::fs::write(
        root.join("languages/typescript-lang-project-harness/package.json"),
        "{\"type\":\"module\"}\n",
    )
    .expect("write workspace package manifest");
    std::fs::write(
        workspace_provider
            .parent()
            .expect("workspace provider parent")
            .join("registry.js"),
        "export const registry = 'workspace-module-graph';\n",
    )
    .expect("write workspace provider sibling module");
    std::fs::write(
        &workspace_provider,
        concat!(
            "#!/usr/bin/env node\n",
            "import { registry } from './registry.js';\n",
            "console.log(JSON.stringify({ registry, args: process.argv.slice(2) }));\n",
        ),
    )
    .expect("write workspace provider");
    make_executable(&workspace_provider);
    let fake_npm = create_fake_tool_bin(&root, "npm", concat!("#!/usr/bin/env sh\n", "exit 0\n",));

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "install",
            "language",
            "typescript",
            "--from-workspace",
            "--target",
            "x86_64-unknown-linux-gnu",
        ])
        .arg("--project")
        .arg(&root)
        .env("HOME", &home)
        .env("ASP_NO_AGENT_PLATFORM", "1")
        .env(
            "PATH",
            format!(
                "{}:{}",
                fake_npm.parent().expect("fake npm parent").display(),
                std::env::var("PATH").expect("PATH")
            ),
        )
        .env_remove("PRJ_CACHE_HOME")
        .env_remove("SEMANTIC_AGENT_BIN_DIR")
        .output()
        .expect("run asp install language typescript --from-workspace");

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("installMode=develop-workspace"), "{stdout}");
    assert!(stdout.contains("source=workspace-build"), "{stdout}");
    let workspace_root = workspace_provider
        .parent()
        .expect("workspace provider artifact root");
    assert!(
        stdout.contains(&format!("workspaceArtifact={}", workspace_root.display())),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!(
            "workspaceEntrypoint={}",
            workspace_provider.display()
        )),
        "{stdout}"
    );
    let installed = home.join(".local/bin/ts-harness");
    assert!(
        std::fs::symlink_metadata(&installed)
            .expect("stat installed TypeScript provider")
            .file_type()
            .is_file(),
        "develop install must materialize an executable launcher"
    );
    let provider_output = Command::new(&installed)
        .args(["agent", "doctor"])
        .output()
        .expect("run installed TypeScript provider");
    assert!(
        provider_output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&provider_output.stdout),
        String::from_utf8_lossy(&provider_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&provider_output.stdout),
        "{\"registry\":\"workspace-module-graph\",\"args\":[\"agent\",\"doctor\"]}\n"
    );
}

#[test]
#[cfg(unix)]
fn install_python_from_workspace_replaces_stale_home_wrapper() {
    let root = temp_project_root();
    let home = root.join("home");
    let workspace_provider =
        root.join("languages/python-lang-project-harness/.venv/bin/py-harness");
    let home_bin_dir = home.join(".local/bin");
    std::fs::create_dir_all(
        workspace_provider
            .parent()
            .expect("workspace provider parent"),
    )
    .expect("create workspace provider parent");
    std::fs::create_dir_all(&home_bin_dir).expect("create home-local bin");

    let workspace_wrapper = concat!(
        "#!/usr/bin/env bash\n",
        "exec uv run --project \"$ASP_PYTHON_PROJECT\" --frozen py-harness \"$@\"\n",
    );
    std::fs::write(&workspace_provider, workspace_wrapper).expect("write workspace python wrapper");
    let workspace_python = workspace_provider
        .parent()
        .expect("workspace provider parent")
        .join("python3");
    std::fs::write(&workspace_python, "#!/usr/bin/env sh\nexit 0\n")
        .expect("write workspace python interpreter");
    make_executable(&workspace_python);
    std::fs::write(
        home_bin_dir.join("py-harness"),
        concat!(
            "#!/usr/bin/env sh\n",
            "exec \"${PYTHON:-python3}\" -m python_lang_project_harness \"$@\"\n",
        ),
    )
    .expect("write stale home-local python wrapper");
    let fake_uv = create_fake_tool_bin(&root, "uv", concat!("#!/usr/bin/env sh\n", "exit 0\n",));

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "install",
            "language",
            "python",
            "--from-workspace",
            "--target",
            "x86_64-unknown-linux-gnu",
        ])
        .arg("--project")
        .arg(&root)
        .env("HOME", &home)
        .env("ASP_NO_AGENT_PLATFORM", "1")
        .env(
            "PATH",
            format!(
                "{}:{}",
                fake_uv.parent().expect("fake uv parent").display(),
                std::env::var("PATH").expect("PATH")
            ),
        )
        .env_remove("PRJ_CACHE_HOME")
        .env_remove("SEMANTIC_AGENT_BIN_DIR")
        .output()
        .expect("run asp install language python --from-workspace");

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("installMode=develop-workspace"), "{stdout}");
    assert!(stdout.contains("source=workspace-build"), "{stdout}");
    assert!(stdout.contains("binary=py-harness"), "{stdout}");
    let workspace_root = root.join("languages/python-lang-project-harness/.venv");
    assert!(
        stdout.contains(&format!("workspaceArtifact={}", workspace_root.display())),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!(
            "workspaceEntrypoint={}",
            workspace_provider.display()
        )),
        "{stdout}"
    );
    assert!(
        stdout.contains("installTargetSource=home-local-bin"),
        "{stdout}"
    );

    let installed =
        std::fs::read_to_string(home_bin_dir.join("py-harness")).expect("read installed launcher");
    assert_ne!(installed, workspace_wrapper);
    assert!(
        installed.contains("provider-artifacts/py-harness") && installed.contains("bin/python3"),
        "develop launcher must execute the immutable CAS interpreter: {installed}"
    );
}

#[test]
#[cfg(unix)]
fn install_julia_from_workspace_replaces_stale_home_binary() {
    let root = temp_project_root();
    let home = root.join("home");
    let workspace_provider =
        root.join("languages/JuliaLangProjectHarness.jl/build/juliac-asp-local/asp-julia-harness");
    let workspace_build =
        root.join("languages/JuliaLangProjectHarness.jl/juliac/build_provider.sh");
    let home_bin_dir = home.join(".local/bin");
    std::fs::create_dir_all(
        workspace_provider
            .parent()
            .expect("workspace Julia provider parent"),
    )
    .expect("create workspace Julia provider parent");
    std::fs::create_dir_all(
        workspace_build
            .parent()
            .expect("workspace Julia build parent"),
    )
    .expect("create workspace Julia build parent");
    std::fs::create_dir_all(&home_bin_dir).expect("create home-local bin");

    std::fs::write(&workspace_provider, b"workspace-julia-provider\n")
        .expect("write workspace julia provider");
    std::fs::write(
        &workspace_build,
        concat!("#!/usr/bin/env sh\n", "exit 0\n",),
    )
    .expect("write workspace Julia build program");
    make_executable(&workspace_build);
    std::fs::write(
        home_bin_dir.join("asp-julia-harness"),
        b"stale-release-provider-with-ci-rpath\n",
    )
    .expect("write stale home-local julia provider");

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "install",
            "language",
            "julia",
            "--from-workspace",
            "--target",
            "x86_64-unknown-linux-gnu",
        ])
        .arg("--project")
        .arg(&root)
        .env("HOME", &home)
        .env_remove("PRJ_CACHE_HOME")
        .env_remove("SEMANTIC_AGENT_BIN_DIR")
        .output()
        .expect("run asp install language julia --from-workspace");

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("installMode=develop-workspace"), "{stdout}");
    assert!(stdout.contains("source=workspace-build"), "{stdout}");
    assert!(stdout.contains("binary=asp-julia-harness"), "{stdout}");
    assert!(
        stdout.contains("installTargetSource=home-local-bin"),
        "{stdout}"
    );

    let installed = std::fs::read(home_bin_dir.join("asp-julia-harness"))
        .expect("read installed julia provider");
    assert_eq!(installed, b"workspace-julia-provider\n");
}

#[test]
#[cfg(unix)]
fn install_language_pinned_release_ignores_asp_toml_provider_bin() {
    assert_install_language_pinned_release_ignores_asp_toml_provider_bin();
}

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
    let bin = home.join(".local/bin/gslph");
    let package_binary = home.join(
        ".agent-semantic-protocols/runtime/provider-locks/gerbil-scheme/v0.1.0/x86_64-unknown-linux-gnu/bin/gerbil-scheme-harness",
    );
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
    let local_bin_entries = sorted_file_names(&home.join(".local/bin"));
    assert_eq!(
        local_bin_entries,
        vec!["gslph".to_string()],
        "provider install must not copy package companions or build artifacts into ~/.local/bin"
    );
    let lock = std::fs::read_to_string(
        home.join(".agent-semantic-protocols/runtime/provider-locks/gerbil-scheme.lock.toml"),
    )
    .expect("read Gerbil lock");
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
        !home.join(".local/bin/gslph").exists(),
        "script payload must not be installed as gslph"
    );
}

#[test]
#[cfg(unix)]
fn install_language_rejects_release_override_flags() {
    let root = temp_project_root();
    let home = root.join("home");

    for (flag, value, expected) in [
        (
            "--rev",
            "vtest",
            "pinned provider releases; --rev is not supported",
        ),
        (
            "--repo",
            "example/repo",
            "pinned provider repositories; --repo is not supported",
        ),
        (
            "--archive",
            "release.tar.gz",
            "pinned GitHub release downloads; --archive is not supported",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .args(["install", "language", "rust", flag, value, "--project"])
            .arg(&root)
            .env("HOME", &home)
            .env_remove("PRJ_CACHE_HOME")
            .output()
            .expect("run asp install language");

        assert!(
            !output.status.success(),
            "flag: {flag}\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let output_text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            output_text.contains(expected),
            "flag: {flag}\n{output_text}"
        );
    }
}

fn assert_install_pinned_release_writes_runtime_bin_package_and_lock() {
    let root = temp_project_root();
    let home = root.join("home");
    let release_dir = create_pinned_release_fixture(&root);
    let workspace_decoy =
        root.join("languages/rust-lang-project-harness/target/release/rs-harness");
    std::fs::create_dir_all(workspace_decoy.parent().expect("workspace decoy parent"))
        .expect("create workspace decoy parent");
    std::fs::write(&workspace_decoy, b"workspace-decoy\n").expect("write workspace decoy");
    let fake_bin = create_fake_curl_bin(&root);

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "install",
            "language",
            "rust",
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
        .expect("run asp install language");

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[asp-install]"), "{stdout}");
    assert!(stdout.contains("installMode=locked-release"), "{stdout}");
    assert!(stdout.contains("rev=v0.1.2"), "{stdout}");

    let runtime = home.join(".agent-semantic-protocols/runtime");
    let bin = home.join(".local/bin/rs-harness");
    let package_binary =
        runtime.join("provider-locks/rust/v0.1.2/x86_64-unknown-linux-gnu/rs-harness");
    let lock = runtime.join("provider-locks/rust.lock.toml");
    assert!(bin.is_file(), "missing runtime bin {}", bin.display());
    assert!(
        package_binary.is_file(),
        "missing package binary {}",
        package_binary.display()
    );
    assert_eq!(
        std::fs::read(&bin).expect("read installed provider"),
        std::fs::read(&package_binary).expect("read package provider"),
        "installed provider target should be the release binary, not a shell launcher"
    );
    assert_ne!(
        std::fs::read(&bin).expect("read installed provider"),
        std::fs::read(&workspace_decoy).expect("read workspace decoy"),
        "plain install must never select a current-workspace artifact"
    );
    let lock_contents = std::fs::read_to_string(&lock).expect("read install lock");
    assert!(lock_contents.contains("rev = \"v0.1.2\""));
    assert!(lock_contents.contains(
        "source = \"https://github.com/tao3k/rust-lang-project-harness/releases/download/v0.1.2/rs-harness-x86_64-unknown-linux-gnu.tar.gz\""
    ));
    assert!(lock_contents.contains("packagePath = "));

    let provider_output = Command::new(&bin)
        .arg("probe")
        .output()
        .expect("run installed provider");
    assert!(
        provider_output.status.success(),
        "provider stderr: {}",
        String::from_utf8_lossy(&provider_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&provider_output.stdout),
        "provider-ok:probe\n"
    );
}

fn assert_install_language_pinned_release_ignores_asp_toml_provider_bin() {
    let root = temp_project_root();
    let home = root.join("home");
    let release_dir = create_pinned_release_fixture(&root);
    let fake_bin = create_fake_curl_bin(&root);
    std::fs::create_dir_all(root.join(".agents")).expect("create .agents");
    std::fs::write(
        root.join(".agents/asp.toml"),
        "[languages.rust]\nbin = \"tools/rs-harness-config\"\n",
    )
    .expect("write asp.toml");

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "install",
            "language",
            "rust",
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
        .expect("run asp install language");

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("installTargetSource=home-local-bin"),
        "{stdout}"
    );

    let bin = home.join(".local/bin/rs-harness");
    assert!(bin.is_file(), "missing home-local bin {}", bin.display());
    assert!(
        !root.join("tools/rs-harness-config").exists(),
        "asp.toml language bin must not be an install target"
    );
    let package_binary = home.join(
        ".agent-semantic-protocols/runtime/provider-locks/rust/v0.1.2/x86_64-unknown-linux-gnu/rs-harness",
    );
    assert_eq!(
        std::fs::read(&bin).expect("read configured provider"),
        std::fs::read(&package_binary).expect("read package provider"),
        "configured provider target should be the release binary, not a shell launcher"
    );

    let provider_output = Command::new(&bin)
        .arg("probe")
        .output()
        .expect("run configured provider");
    assert!(
        provider_output.status.success(),
        "provider stderr: {}",
        String::from_utf8_lossy(&provider_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&provider_output.stdout),
        "provider-ok:probe\n"
    );
}

fn create_pinned_release_fixture(root: &Path) -> PathBuf {
    let release_dir = root.join("release");
    let payload_dir = release_dir.join("payload");
    let binary = payload_dir.join("rs-harness");
    std::fs::create_dir_all(&payload_dir).expect("create release payload dir");
    std::fs::write(
        &binary,
        "#!/bin/sh\nprintf 'provider-ok:%s\\n' \"${1:-missing}\"\n",
    )
    .expect("write fake provider binary");
    make_executable(&binary);

    let archive = release_dir.join("rs-harness-x86_64-unknown-linux-gnu.tar.gz");
    let status = Command::new("tar")
        .arg("-czf")
        .arg(&archive)
        .arg("-C")
        .arg(&payload_dir)
        .arg("rs-harness")
        .status()
        .expect("create provider archive");
    assert!(status.success(), "tar failed with status {status}");
    let sha256 = sha256_file(&archive);
    std::fs::write(
        release_dir.join("rs-harness-x86_64-unknown-linux-gnu.tar.gz.sha256"),
        format!("{sha256}  rs-harness-x86_64-unknown-linux-gnu.tar.gz\n"),
    )
    .expect("write provider checksum");
    release_dir
}

fn create_gerbil_pinned_release_fixture(root: &Path) -> PathBuf {
    create_gerbil_release_fixture(root, b"\x7FELFfake-gerbil-native-provider\n")
}

fn create_gerbil_script_release_fixture(root: &Path) -> PathBuf {
    create_gerbil_release_fixture(
        root,
        b"#!/bin/sh\nprintf 'gerbil-provider-ok:%s\\n' \"${1:-missing}\"\n",
    )
}

fn create_gerbil_release_fixture(root: &Path, payload: &[u8]) -> PathBuf {
    let release_dir = root.join("release");
    let payload_dir = release_dir.join("payload");
    let bin_dir = payload_dir.join("bin");
    let binary = bin_dir.join("gerbil-scheme-harness");
    std::fs::create_dir_all(&bin_dir).expect("create Gerbil release bin dir");
    std::fs::write(&binary, payload).expect("write fake Gerbil provider binary");
    make_executable(&binary);

    let archive = release_dir.join("gerbil-scheme-harness-x86_64-unknown-linux-gnu.tar.gz");
    let status = Command::new("tar")
        .arg("-czf")
        .arg(&archive)
        .arg("-C")
        .arg(&payload_dir)
        .arg("bin")
        .status()
        .expect("create Gerbil provider archive");
    assert!(status.success(), "tar failed with status {status}");
    let sha256 = sha256_file(&archive);
    std::fs::write(
        release_dir.join("gerbil-scheme-harness-x86_64-unknown-linux-gnu.tar.gz.sha256"),
        format!("{sha256}  gerbil-scheme-harness-x86_64-unknown-linux-gnu.tar.gz\n"),
    )
    .expect("write Gerbil provider checksum");
    release_dir
}

fn create_fake_curl_bin(root: &Path) -> PathBuf {
    let fake_bin = root.join("fake-bin");
    let fake_curl = fake_bin.join("curl");
    std::fs::create_dir_all(&fake_bin).expect("create fake bin dir");
    std::fs::write(
        &fake_curl,
        r#"#!/bin/sh
if [ "$1" != "-fsSL" ] || [ "$2" != "-o" ]; then
  echo "unexpected curl args: $*" >&2
  exit 1
fi
out="$3"
url="$4"
case "$url" in
  https://github.com/tao3k/rust-lang-project-harness/releases/download/v0.1.2/*)
    ;;
  https://github.com/tao3k/gerbil-scheme-language-project-harness/releases/download/v0.1.0/*)
    ;;
  *)
    echo "unexpected release url: $url" >&2
    exit 1
    ;;
esac
name="${url##*/}"
case "$name" in
  rs-harness-x86_64-unknown-linux-gnu.tar.gz|rs-harness-x86_64-unknown-linux-gnu.tar.gz.sha256)
    cp "$ASP_TEST_RELEASE_DIR/$name" "$out"
    ;;
  gerbil-scheme-harness-x86_64-unknown-linux-gnu.tar.gz|gerbil-scheme-harness-x86_64-unknown-linux-gnu.tar.gz.sha256)
    cp "$ASP_TEST_RELEASE_DIR/$name" "$out"
    ;;
  *)
    echo "unexpected release asset: $url" >&2
    exit 1
    ;;
esac
"#,
    )
    .expect("write fake curl");
    make_executable(&fake_curl);
    fake_bin
}

fn prepend_path(path: &Path) -> std::ffi::OsString {
    let mut paths = vec![path.to_path_buf()];
    if let Some(existing_path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&existing_path));
    }
    std::env::join_paths(paths).expect("join PATH")
}

fn sha256_file(path: &Path) -> String {
    let mut file = std::fs::File::open(path).expect("open file for sha256");
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 32 * 1024];
    loop {
        let read = file.read(&mut buffer).expect("read file for sha256");
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    format!("{:x}", hasher.finalize())
}

fn sorted_file_names(path: &Path) -> Vec<String> {
    let mut entries = std::fs::read_dir(path)
        .expect("read dir")
        .map(|entry| {
            entry
                .expect("dir entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

fn temp_project_root() -> PathBuf {
    let unique = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "asp-install-provider-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos(),
        unique,
    ));
    std::fs::create_dir_all(&root).expect("create temp root");
    Command::new("git")
        .args(["init", "-q"])
        .current_dir(&root)
        .status()
        .expect("git init");
    root
}

fn create_fake_tool_bin(root: &PathBuf, name: &str, body: &str) -> PathBuf {
    let bin_dir = root.join(".fake-bin");
    std::fs::create_dir_all(&bin_dir).expect("create fake bin dir");
    let tool = bin_dir.join(name);
    std::fs::write(&tool, body).expect("write fake tool");
    make_executable(&tool);
    tool
}

fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)
        .expect("provider metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).expect("provider permissions");
}
