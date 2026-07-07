use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_echo_provider,
    write_marker_provider, write_stdout_stderr_exit_provider, write_stdout_stderr_provider,
};

#[test]
fn gerbil_owner_items_query_set_delegates_to_provider() {
    let root = temp_project_root("search-owner-gerbil-provider-owned");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(root.join("src/checker")).expect("create source");
    std::fs::write(
        root.join("src/checker/types.ss"),
        "(def (type-compatible? actual expected)\n  (equal? actual expected))\n",
    )
    .expect("write source");
    write_stdout_stderr_provider(
        &bin_dir,
        "gslph",
        "I=item:symbol(type-compatible?)@src/checker/types.ss:1:2!syntax\n\
reason=owner-item-selector-ready\n",
        "provider-owned-owner-items\n",
    );
    write_activation(&root, &[provider("gerbil-scheme", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "gerbil-scheme",
            "search",
            "owner",
            "src/checker/types.ss",
            "items",
            "--query",
            "type-compatible",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp gerbil search owner items");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("I=item:symbol(type-compatible?)@src/checker/types.ss:1:2!syntax"),
        "{stdout}"
    );
    assert!(
        stdout.contains("reason=owner-item-selector-ready"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("rust-inline-gerbil-owner-items"),
        "{stdout}"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(stderr.contains("provider-owned-owner-items"), "{stderr}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn gerbil_structural_selector_query_delegates_to_provider() {
    let root = temp_project_root("query-gerbil-structural-selector-provider");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(root.join("src/checker")).expect("create source");
    std::fs::write(
        root.join("src/checker/types.ss"),
        "(def (type-compatible? actual expected)\n  (equal? actual expected))\n",
    )
    .expect("write source");
    write_stdout_stderr_provider(
        &bin_dir,
        "gslph",
        "provider-owned-code\n",
        "provider-owned-structural-query\n",
    );
    write_activation(&root, &[provider("gerbil-scheme", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "gerbil-scheme",
            "query",
            "--selector",
            "gerbil-scheme://src/checker/types.ss#item/def/type-compatible?",
            "--workspace",
            ".",
            "--code",
        ])
        .output()
        .expect("run asp gerbil structural query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "provider-owned-code\n"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("provider-owned-structural-query"),
        "{stderr}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn gerbil_regular_range_query_delegates_to_provider_without_source_read() {
    let root = temp_project_root("search-owner-gerbil-range-query-provider");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(root.join("src/checker")).expect("create source");
    std::fs::write(
        root.join("src/checker/types.ss"),
        "(def (type-compatible? actual expected)\n  (equal? actual expected))\n",
    )
    .expect("write source");
    write_echo_provider(&bin_dir, "gslph", "gerbil");
    write_activation(&root, &[provider("gerbil-scheme", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "gerbil-scheme",
            "query",
            "--selector",
            "src/checker/types.ss:1:2",
            "--workspace",
            ".",
            "--code",
        ])
        .output()
        .expect("run asp gerbil range query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("gerbil args=[query][--selector][src/checker/types.ss:1:2][--code]"),
        "regular selector query should invoke provider-owned direct-source-read: {stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn gerbil_owner_items_external_workspace_uses_activation_bin_config() {
    let root = temp_project_root("search-owner-gerbil-external-workspace-bin-root");
    let workspace = root.join(".data/gerbil-v0.19-staging");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(workspace.join("src/gerbil/compiler")).expect("create source");
    std::fs::write(
        workspace.join("src/gerbil/compiler/driver.ss"),
        "(def (compile-module ctx mod) (invoke-gsc mod))\n",
    )
    .expect("write source");
    std::fs::create_dir_all(root.join(".agents")).expect("create asp config dir");
    std::fs::write(
        root.join(".agents/asp.toml"),
        "[languages.gerbil-scheme]\nbin = \".bin/gslph\"\n",
    )
    .expect("write asp config");
    write_stdout_stderr_provider(
        &bin_dir,
        "gslph",
        "I=item:symbol(compile-module)@src/gerbil/compiler/driver.ss:1:6!syntax\n\
reason=owner-item-selector-ready\n",
        "activation-bin-provider\n",
    );
    write_activation(&root, &[provider("gerbil-scheme", Vec::new())]);

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "gerbil-scheme",
            "search",
            "owner",
            "src/gerbil/compiler/driver.ss",
            "items",
            "--query",
            "compile-module|invoke-gsc|parallel|compile-file|compile-scm-file|gsc-options|keep-scm",
            "--workspace",
            ".data/gerbil-v0.19-staging",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp gerbil search owner items");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("I=item:symbol(compile-module)@src/gerbil/compiler/driver.ss:1:6!syntax"),
        "{stdout}"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(stderr.contains("activation-bin-provider"), "{stderr}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn gerbil_owner_items_query_set_rejects_empty_provider_output_for_existing_owner() {
    let root = temp_project_root("search-owner-gerbil-empty-provider-query-set");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src/checker")).expect("create source");
    std::fs::write(
        root.join("src/checker/types.ss"),
        "(def (type-compatible? actual expected)\n  (equal? actual expected))\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "gslph", &marker);
    write_activation(&root, &[provider("gerbil-scheme", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "gerbil-scheme",
            "search",
            "owner",
            "src/checker/types.ss",
            "items",
            "--query",
            "type-compatible",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp gerbil search owner items");

    assert!(!output.status.success(), "owner-items should fail closed");
    assert!(marker.exists(), "provider should be invoked");
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("provider-owned owner-items produced empty output"),
        "{stderr}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn gerbil_owner_items_query_set_delegates_poo_operator_items_to_provider() {
    let root = temp_project_root("search-owner-gerbil-poo-operator-items");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(root.join("gerbil/src/poo-flow")).expect("create source");
    std::fs::write(
        root.join("gerbil/src/poo-flow/poo.ss"),
        "(.def root-cache value: 1)\n(.defgeneric (distance self other))\n(.@ root-cache value)\n(.o value: 1)\n(.mix root-cache)\n",
    )
    .expect("write source");
    write_stdout_stderr_provider(
        &bin_dir,
        "gslph",
        "I=item:symbol(.defgeneric)@gerbil/src/poo-flow/poo.ss:2:2!syntax;\n\
I2=item:symbol(.o)@gerbil/src/poo-flow/poo.ss:4:4!syntax;\n\
I3=item:symbol(.@)@gerbil/src/poo-flow/poo.ss:3:3!syntax;\n\
I4=item:symbol(.mix)@gerbil/src/poo-flow/poo.ss:5:5!syntax;\n\
reason=owner-item-selector-ready\n",
        "provider-owned-owner-items\n",
    );
    write_activation(&root, &[provider("gerbil-scheme", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "gerbil-scheme",
            "search",
            "owner",
            "gerbil/src/poo-flow/poo.ss",
            "items",
            "--query",
            ".o|.@|.mix|defgeneric",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp gerbil search owner items");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    for token in [".defgeneric", ".o", ".@", ".mix"] {
        assert!(stdout.contains(token), "{stdout}");
    }
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(stderr.contains("provider-owned-owner-items"), "{stderr}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn gerbil_owner_items_query_set_uses_provider_config_owner_selector() {
    let root = temp_project_root("search-owner-gerbil-config-query-set");
    let bin_dir = root.join(".bin");
    std::fs::write(
        root.join("gerbil.pkg"),
        "(package: sample/app\n depend: (\"git.cons.io/mighty-gerbils/gerbil-poo\"))\n",
    )
    .expect("write gerbil package");
    write_stdout_stderr_provider(
        &bin_dir,
        "gslph",
        "I=item:symbol(gerbil.pkg)@gerbil.pkg:1:1!syntax\n\
nextCommand=asp gerbil-scheme query --selector gerbil.pkg:1:1 --workspace . --code\n\
reason=owner-item-selector-ready\n",
        "provider-owned-owner-items\n",
    );
    write_activation(&root, &[provider("gerbil-scheme", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "gerbil-scheme",
            "search",
            "owner",
            "gerbil.pkg",
            "items",
            "--query",
            "matrix|gxpkg|deps|install|cache|gerbil.pkg",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp gerbil search owner items");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("I=item:symbol(gerbil.pkg)@gerbil.pkg:1:1!syntax"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "nextCommand=asp gerbil-scheme query --selector gerbil.pkg:1:1 --workspace . --code"
        ),
        "{stdout}"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(stderr.contains("provider-owned-owner-items"), "{stderr}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn gerbil_owner_items_query_set_rejects_other_language_config_owner() {
    let root = temp_project_root("search-owner-gerbil-non-gerbil-config");
    let bin_dir = root.join(".bin");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"not-gerbil\"\n",
    )
    .expect("write rust package");
    write_stdout_stderr_exit_provider(&bin_dir, "gslph", "", "owner not found Cargo.toml\n", 1);
    write_activation(&root, &[provider("gerbil-scheme", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "gerbil-scheme",
            "search",
            "owner",
            "Cargo.toml",
            "items",
            "--query",
            "Cargo.toml",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp gerbil search owner items");

    assert!(
        !output.status.success(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("provider-owned owner-items failed"),
        "{stderr}"
    );
    assert!(stderr.contains("owner not found Cargo.toml"), "{stderr}");
    let _ = std::fs::remove_dir_all(root);
}
