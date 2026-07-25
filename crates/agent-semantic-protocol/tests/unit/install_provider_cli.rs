use std::process::Command;

#[path = "install_provider_cli/support.rs"]
mod support;

use support::{
    create_fake_curl_bin, create_fake_tool_bin, create_gerbil_pinned_release_fixture,
    create_gerbil_script_release_fixture, create_pinned_release_fixture, make_executable,
    prepend_path, sorted_file_names, temp_project_root, write_workspace_source_anchors,
};

#[test]
#[cfg(unix)]
fn install_language_pinned_release_writes_runtime_bin_package_and_lock() {
    assert_install_pinned_release_writes_runtime_bin_package_and_lock();
}

#[test]
#[cfg(unix)]
fn install_language_from_workspace_refreshes_state_home_runtime_bin() {
    let root = temp_project_root();
    let home = root.join("home");
    let state_home = root.join("state-home");
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
    write_workspace_source_anchors(
        &root,
        &[
            "languages/rust-lang-project-harness/Cargo.toml",
            "languages/rust-lang-project-harness/Cargo.lock",
        ],
    );
    let fake_cargo = create_fake_tool_bin(
        &root,
        "cargo",
        concat!(
            "#!/usr/bin/env sh\n",
            "mkdir -p target/release\n",
            "printf 'workspace-dev-provider\\n' > target/release/rs-harness\n",
            "chmod +x target/release/rs-harness\n",
        ),
    );

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
        .env("ASP_STATE_HOME", &state_home)
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
            "workspaceArtifact={}/runtime/provider-builds/rs-harness/",
            state_home.display()
        )),
        "{stdout}"
    );
    assert!(
        stdout.contains("languages/rust-lang-project-harness/target/release/rs-harness"),
        "{stdout}"
    );
    assert!(
        stdout.contains("installTargetSource=state-home-runtime-bin"),
        "{stdout}"
    );

    let installed = state_home.join("runtime/bin/rs-harness");
    assert_eq!(
        std::fs::read(&installed).expect("read installed workspace provider"),
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

    assert!(output.status.success());
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
    let state_home = root.join("state-home");
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
    write_workspace_source_anchors(
        &root,
        &[
            "languages/typescript-lang-project-harness/package-lock.json",
            "languages/typescript-lang-project-harness/pnpm-lock.yaml",
        ],
    );
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
    let fake_npm = create_fake_tool_bin(
        &root,
        "npm",
        concat!(
            "#!/usr/bin/env sh\n",
            "mkdir -p dist/provider\n",
            "printf '%s\\n' \"export const registry = 'workspace-module-graph';\" > dist/provider/registry.js\n",
            "printf '%s\\n' '#!/usr/bin/env node' \"import { registry } from './registry.js';\" \"console.log(JSON.stringify({ registry, args: process.argv.slice(2) }));\" > dist/provider/ts-harness.mjs\n",
            "chmod +x dist/provider/ts-harness.mjs\n",
        ),
    );

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
        .env("ASP_STATE_HOME", &state_home)
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
    assert!(
        stdout.contains(&format!(
            "workspaceArtifact={}/runtime/provider-builds/ts-harness/",
            state_home.display()
        )),
        "{stdout}"
    );
    assert!(
        stdout.contains("languages/typescript-lang-project-harness/dist/provider/ts-harness.mjs"),
        "{stdout}"
    );
    let installed = state_home.join("runtime/bin/ts-harness");
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
fn install_python_from_workspace_replaces_stale_state_home_wrapper() {
    let root = temp_project_root();
    let home = root.join("home");
    let state_home = root.join("state-home");
    let workspace_provider =
        root.join("languages/python-lang-project-harness/.venv/bin/py-harness");
    let runtime_bin_dir = state_home.join("runtime/bin");
    std::fs::create_dir_all(
        workspace_provider
            .parent()
            .expect("workspace provider parent"),
    )
    .expect("create workspace provider parent");
    std::fs::create_dir_all(&runtime_bin_dir).expect("create State Home runtime bin");
    write_workspace_source_anchors(
        &root,
        &[
            "languages/python-lang-project-harness/pyproject.toml",
            "languages/python-lang-project-harness/uv.lock",
        ],
    );

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
        runtime_bin_dir.join("py-harness"),
        concat!(
            "#!/usr/bin/env sh\n",
            "exec \"${PYTHON:-python3}\" -m python_lang_project_harness \"$@\"\n",
        ),
    )
    .expect("write stale State Home python wrapper");
    let fake_uv = create_fake_tool_bin(
        &root,
        "uv",
        concat!(
            "#!/usr/bin/env sh\n",
            "mkdir -p .venv/bin\n",
            "printf '%s\\n' '#!/usr/bin/env sh' 'exit 0' > .venv/bin/python3\n",
            "printf '%s\\n' '#!/usr/bin/env sh' 'exit 0' > .venv/bin/py-harness\n",
            "chmod +x .venv/bin/python3 .venv/bin/py-harness\n",
        ),
    );

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
        .env("ASP_STATE_HOME", &state_home)
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
    assert!(
        stdout.contains(&format!(
            "workspaceArtifact={}/runtime/provider-builds/py-harness/",
            state_home.display()
        )),
        "{stdout}"
    );
    assert!(
        stdout.contains("languages/python-lang-project-harness/.venv/bin/py-harness"),
        "{stdout}"
    );
    assert!(
        stdout.contains("installTargetSource=state-home-runtime-bin"),
        "{stdout}"
    );

    let installed = std::fs::read_to_string(runtime_bin_dir.join("py-harness"))
        .expect("read installed launcher");
    assert_ne!(installed, workspace_wrapper);
    assert!(
        installed.contains("provider-artifacts/py-harness") && installed.contains("bin/python3"),
        "develop launcher must execute the immutable CAS interpreter: {installed}"
    );
}

#[test]
#[cfg(unix)]
fn install_julia_from_workspace_replaces_stale_state_home_binary() {
    let root = temp_project_root();
    let home = root.join("home");
    let state_home = root.join("state-home");
    let workspace_provider =
        root.join("languages/JuliaLangProjectHarness.jl/build/juliac-asp-local/asp-julia-harness");
    let workspace_build =
        root.join("languages/JuliaLangProjectHarness.jl/juliac/build_provider.sh");
    let runtime_bin_dir = state_home.join("runtime/bin");
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
    std::fs::create_dir_all(&runtime_bin_dir).expect("create State Home runtime bin");
    write_workspace_source_anchors(
        &root,
        &[
            "languages/JuliaLangProjectHarness.jl/Project.toml",
            "languages/JuliaLangProjectHarness.jl/Manifest.toml",
            "languages/JuliaLangProjectHarness.jl/juliac/Project.toml",
            "languages/JuliaLangProjectHarness.jl/juliac/Manifest.toml",
        ],
    );

    std::fs::write(&workspace_provider, b"workspace-julia-provider\n")
        .expect("write workspace julia provider");
    std::fs::write(
        &workspace_build,
        concat!(
            "#!/usr/bin/env sh\n",
            "mkdir -p build/juliac-asp-local\n",
            "printf 'workspace-julia-provider\\n' > build/juliac-asp-local/asp-julia-harness\n",
            "chmod +x build/juliac-asp-local/asp-julia-harness\n",
        ),
    )
    .expect("write workspace Julia build program");
    make_executable(&workspace_build);
    std::fs::write(
        runtime_bin_dir.join("asp-julia-harness"),
        b"stale-release-provider-with-ci-rpath\n",
    )
    .expect("write stale State Home julia provider");

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
        .env("ASP_STATE_HOME", &state_home)
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
        stdout.contains("installTargetSource=state-home-runtime-bin"),
        "{stdout}"
    );

    let installed = std::fs::read(runtime_bin_dir.join("asp-julia-harness"))
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
    let state_home = root.join("state-home");
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
        .env("ASP_STATE_HOME", &state_home)
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
    let runtime = state_home.join("runtime");
    let bin = runtime.join("bin/gslph");
    let package_binary = runtime.join(
        "provider-locks/gerbil-scheme/v0.1.0/x86_64-unknown-linux-gnu/bin/gerbil-scheme-harness",
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
    let runtime_bin_entries = sorted_file_names(&runtime.join("bin"));
    assert_eq!(
        runtime_bin_entries,
        vec!["gslph".to_string()],
        "provider install must not copy package companions or build artifacts into State Home runtime/bin"
    );
    let lock = std::fs::read_to_string(runtime.join("provider-locks/gerbil-scheme.lock.toml"))
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
    let state_home = root.join("state-home");
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
        .env("ASP_STATE_HOME", &state_home)
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
        !state_home.join("runtime/bin/gslph").exists(),
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
    let state_home = root.join("state-home");
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
        .env("ASP_STATE_HOME", &state_home)
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

    let runtime = state_home.join("runtime");
    let bin = runtime.join("bin/rs-harness");
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
    let state_home = root.join("state-home");
    let release_dir = create_pinned_release_fixture(&root);
    let fake_bin = create_fake_curl_bin(&root);
    std::fs::create_dir_all(root.join(".agents")).expect("create .agents");
    std::fs::write(
        root.join(".agents/asp.toml"),
        "[languages.rust]\nbin = \"custom-rs-harness\"\n",
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
        .env("ASP_STATE_HOME", &state_home)
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
        stdout.contains("installTargetSource=state-home-runtime-bin"),
        "{stdout}"
    );

    let bin = state_home.join("runtime/bin/rs-harness");
    assert!(bin.is_file(), "missing State Home bin {}", bin.display());
    assert!(
        !state_home.join("runtime/bin/custom-rs-harness").exists(),
        "asp.toml language basename must not override the pinned release install target"
    );
    let package_binary =
        state_home.join("runtime/provider-locks/rust/v0.1.2/x86_64-unknown-linux-gnu/rs-harness");
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
