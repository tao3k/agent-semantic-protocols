use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation,
    write_activation_env_guard_provider, write_echo_provider, write_marker_provider,
    write_stdout_stderr_exit_provider, write_stdout_stderr_provider,
};
use std::time::{Duration, Instant};

#[test]
fn gerbil_owner_items_query_set_uses_provider_scheme_item_selectors() {
    let root = temp_project_root("search-owner-gerbil-query-set");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(root.join("src/checker")).expect("create source");
    std::fs::write(
        root.join("src/checker/types.ss"),
        "(def (type-compatible? actual expected)\n  (or (equal? actual expected)\n      (and (equal? expected 'union)\n           (any-type-compatible? actual expected))))\n\n(def (any-type-compatible? actual expected-members)\n  (if (null? expected-members)\n    #f\n    (type-compatible? actual (car expected-members))))\n",
    )
    .expect("write source");
    write_stdout_stderr_provider(
        &bin_dir,
        "gslph",
        "I=item:symbol(type-compatible?)@src/checker/types.ss:1:4!syntax\n\
I2=item:symbol(any-type-compatible?)@src/checker/types.ss:6:9!syntax\n\
nextCommand=asp gerbil-scheme query --selector src/checker/types.ss:1:4 --workspace . --code\n\
reason=owner-item-selector-ready\n",
        "provider-owned-owner-items\n",
    );
    write_activation(&root, &[provider("gerbil-scheme", Vec::new())]);

    let started_at = Instant::now();
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
    let elapsed = started_at.elapsed();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "Gerbil facade owner-items should stay in Rust inline fast path, elapsed={elapsed:?}"
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("I=item:symbol(type-compatible?)@gerbil-scheme://src/checker/types.ss#item/def/type-compatible?!syntax"),
        "{stdout}"
    );
    assert!(
        stdout.contains("item:symbol(any-type-compatible?)@gerbil-scheme://src/checker/types.ss#item/def/any-type-compatible?!syntax"),
        "{stdout}"
    );
    assert!(
        stdout.contains("sourceLocatorHint=src/checker/types.ss:1:4"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("item:symbol(type-compatible?)@src/checker/types.ss:"),
        "{stdout}"
    );
    assert!(
        stdout.contains("nextCommand=asp gerbil-scheme query --from-hook query-code --selector 'gerbil-scheme://src/checker/types.ss#item/def/type-compatible?' --workspace . --code"),
        "{stdout}"
    );
    assert!(
        stdout.contains("reason=owner-item-selector-ready"),
        "{stdout}"
    );
    assert!(!stdout.contains("reason=no-owner-item-match"), "{stdout}");
    assert!(
        stdout.contains("rust-inline-gerbil-owner-items"),
        "Gerbil owner-items should stay in the ASP inline fast path: {stdout}"
    );
    assert!(
        !String::from_utf8(output.stderr)
            .expect("stderr")
            .contains("provider-owned-owner-items"),
        "Gerbil owner-items should not spawn the language provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn gerbil_structural_selector_query_materializes_inline_without_provider() {
    let root = temp_project_root("query-gerbil-structural-selector-inline");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(root.join("src/checker")).expect("create source");
    std::fs::write(
        root.join("src/checker/types.ss"),
        "(def (type-compatible? actual expected)\n  (equal? actual expected))\n\n(type-compatible? 'a 'a)\n",
    )
    .expect("write source");
    write_stdout_stderr_provider(&bin_dir, "gslph", "", "provider-owned-structural-query\n");
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
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("(def (type-compatible? actual expected)"),
        "{stdout}"
    );
    assert!(stdout.contains("  (equal? actual expected)"), "{stdout}");
    assert!(
        !stdout.contains("(type-compatible? 'a 'a)"),
        "structural def selector should not fall through to call matches: {stdout}"
    );
    assert!(
        !String::from_utf8(output.stderr)
            .expect("stderr")
            .contains("provider-owned-structural-query"),
        "Gerbil structural selector query should stay in the inline parser"
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
        !stdout.contains("(def (type-compatible? actual expected)"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "gerbil args=[query][--from-hook][direct-source-read][--selector][src/checker/types.ss:1:2][--code]"
        ),
        "regular selector query should invoke provider-owned direct-source-read: {stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn gerbil_owner_items_query_set_bypasses_client_backend_worker() {
    let root = temp_project_root("search-owner-gerbil-direct-provider");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(root.join("src/checker")).expect("create source");
    std::fs::write(
        root.join("src/checker/types.ss"),
        "(def (type-compatible? actual expected)\n  (equal? actual expected))\n",
    )
    .expect("write source");
    write_activation_env_guard_provider(
        &bin_dir,
        "gslph",
        "I=item:symbol(type-compatible?)@src/checker/types.ss:1:2!syntax\n\
reason=owner-item-selector-ready\n",
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
        stdout.contains("item:symbol(type-compatible?)@gerbil-scheme://src/checker/types.ss#item/def/type-compatible?!syntax"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("item:symbol(type-compatible?)@src/checker/types.ss:"),
        "{stdout}"
    );
    assert!(
        stdout.contains("rust-inline-gerbil-owner-items"),
        "{stdout}"
    );
    assert!(
        !String::from_utf8(output.stderr)
            .expect("stderr")
            .contains("unexpected client backend activation env")
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
        stdout.contains("I=item:symbol(compile-module)@gerbil-scheme://src/gerbil/compiler/driver.ss#item/def/compile-module!syntax"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("I=item:symbol(compile-module)@src/gerbil/compiler/driver.ss:"),
        "{stdout}"
    );
    assert!(
        stdout.contains("reason=owner-item-selector-ready"),
        "{stdout}"
    );
    assert!(
        stdout.contains("rust-inline-gerbil-owner-items"),
        "external workspace owner-items should stay in the ASP inline fast path: {stdout}"
    );
    assert!(
        !String::from_utf8(output.stderr)
            .expect("stderr")
            .contains("activation-bin-provider"),
        "external workspace owner-items should not spawn the Gerbil provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn gerbil_owner_items_query_set_uses_provider_ssi_sources() {
    let root = temp_project_root("search-owner-gerbil-ssi-query-set");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(root.join("src/api")).expect("create source");
    std::fs::write(
        root.join("src/api/types.ssi"),
        "(defstruct required-extension (name dependency-mode))\n",
    )
    .expect("write source");
    write_stdout_stderr_provider(
        &bin_dir,
        "gslph",
        "I=item:symbol(required-extension)@src/api/types.ssi:1:1!syntax\n\
nextCommand=asp gerbil-scheme query --selector src/api/types.ssi:1:1 --workspace . --code\n\
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
            "src/api/types.ssi",
            "items",
            "--query",
            "required-extension",
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
        stdout.contains("I=item:symbol(required-extension)@gerbil-scheme://src/api/types.ssi#item/struct/required-extension!syntax"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("I=item:symbol(required-extension)@src/api/types.ssi:"),
        "{stdout}"
    );
    assert!(
        stdout.contains("nextCommand=asp gerbil-scheme query --from-hook query-code --selector 'gerbil-scheme://src/api/types.ssi#item/struct/required-extension' --workspace . --code"),
        "{stdout}"
    );
    assert!(
        stdout.contains("rust-inline-gerbil-owner-items"),
        "Gerbil .ssi owner-items should stay in the ASP inline fast path: {stdout}"
    );
    assert!(
        !String::from_utf8(output.stderr)
            .expect("stderr")
            .contains("provider-owned-owner-items"),
        "Gerbil .ssi owner-items should not spawn the language provider"
    );
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

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        !stdout.contains("asp-fast-owner-query-v1"),
        "existing Gerbil owner path must not fall back to Rust owner query rendering"
    );
    assert!(
        stdout.contains("rust-inline-gerbil-owner-items"),
        "existing Gerbil owner path should use the inline fast path: {stdout}"
    );
    assert!(!marker.exists(), "provider should not be invoked");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn gerbil_owner_items_query_set_delegates_poo_operator_items_to_provider() {
    let root = temp_project_root("search-owner-gerbil-poo-operator-items");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(root.join("gerbil/src/poo-flow")).expect("create source");
    std::fs::write(
        root.join("gerbil/src/poo-flow/poo.ss"),
        "(package: sample/poo-flow)\n\
(.def root-cache\n\
  value: 1)\n\
\n\
(.defgeneric (distance self other))\n\
\n\
(defclass (FlowError Exception) (slot) transparent: #t)\n\
\n\
(defmethod (@method :flow object)\n\
  (.@ root-cache value))\n\
\n\
(def (.mix slots: (slots '()) . supers)\n\
  (.o value: slots))\n\
\n\
(def (make-node seed)\n\
  (.o value: seed)\n\
  (.@ root-cache value)\n\
  (.mix root-cache (.o extra: seed)))\n",
    )
    .expect("write source");
    write_stdout_stderr_provider(
        &bin_dir,
        "gslph",
        "I=item:symbol(.defgeneric)@gerbil/src/poo-flow/poo.ss:5:5!syntax;\n\
I2=item:symbol(defclass)@gerbil/src/poo-flow/poo.ss:7:7!syntax;\n\
I3=item:symbol(defmethod)@gerbil/src/poo-flow/poo.ss:9:10!syntax;\n\
I4=item:symbol(.o)@gerbil/src/poo-flow/poo.ss:16:16!syntax;\n\
I5=item:symbol(.@)@gerbil/src/poo-flow/poo.ss:17:17!syntax;\n\
I6=item:symbol(.mix)@gerbil/src/poo-flow/poo.ss:18:18!syntax;\n\
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
            ".o|.@|.mix|object?|defclass|defgeneric|defmethod",
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
        stdout.contains("item:symbol(.defgeneric)@gerbil-scheme://gerbil/src/poo-flow/poo.ss#item/call/.defgeneric!syntax"),
        "{stdout}"
    );
    assert!(
        stdout.contains("item:symbol(defclass)@gerbil-scheme://gerbil/src/poo-flow/poo.ss#item/call/defclass!syntax"),
        "{stdout}"
    );
    assert!(
        stdout.contains("item:symbol(defmethod)@gerbil-scheme://gerbil/src/poo-flow/poo.ss#item/call/defmethod!syntax"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "item:symbol(.o)@gerbil-scheme://gerbil/src/poo-flow/poo.ss#item/call/.o!syntax"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "item:symbol(.@)@gerbil-scheme://gerbil/src/poo-flow/poo.ss#item/call/.@!syntax"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "item:symbol(.mix)@gerbil-scheme://gerbil/src/poo-flow/poo.ss#item/def/.mix!syntax"
        ) || stdout.contains(
            "item:symbol(.mix)@gerbil-scheme://gerbil/src/poo-flow/poo.ss#item/call/.mix!syntax"
        ),
        "{stdout}"
    );
    assert!(
        !stdout.contains("item:symbol(.defgeneric)@gerbil/src/poo-flow/poo.ss:"),
        "{stdout}"
    );
    assert!(
        stdout.contains("reason=owner-item-selector-ready"),
        "{stdout}"
    );
    assert!(!stdout.contains("reason=no-owner-item-match"), "{stdout}");
    assert!(
        stdout.contains("rust-inline-gerbil-owner-items"),
        "Gerbil POO owner-items must stay in the ASP inline fast path: {stdout}"
    );
    assert!(
        !String::from_utf8(output.stderr)
            .expect("stderr")
            .contains("provider-owned-owner-items"),
        "Gerbil POO owner-items should not spawn the language provider"
    );
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
        stdout.contains(
            "I=item:symbol(gerbil.pkg)@gerbil-scheme://gerbil.pkg#item/package/gerbil.pkg!syntax"
        ),
        "{stdout}"
    );
    assert!(
        !stdout.contains("I=item:symbol(gerbil.pkg)@gerbil.pkg:1:1!syntax"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "nextCommand=asp gerbil-scheme query --from-hook query-code --selector 'gerbil-scheme://gerbil.pkg#item/package/gerbil.pkg' --workspace . --code"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("reason=owner-item-selector-ready"),
        "{stdout}"
    );
    assert!(!stdout.contains("reason=no-owner-item-match"), "{stdout}");
    assert!(
        stdout.contains("rust-inline-gerbil-owner-items"),
        "Gerbil config owner-items should stay in the ASP inline fast path: {stdout}"
    );
    assert!(
        !String::from_utf8(output.stderr)
            .expect("stderr")
            .contains("provider-owned-owner-items"),
        "Gerbil config owner-items should not spawn the language provider"
    );
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
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        !stdout.contains("reason=no-owner-item-match"),
        "existing non-Gerbil owner path must not use Rust fallback output: {stdout}"
    );
    assert!(
        !stdout.contains("I=item:symbol(Cargo.toml)@Cargo.toml:1:1!syntax"),
        "{stdout}"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("provider-owned owner-items failed"),
        "{stderr}"
    );
    assert!(stderr.contains("owner not found Cargo.toml"), "{stderr}");
    let _ = std::fs::remove_dir_all(root);
}
