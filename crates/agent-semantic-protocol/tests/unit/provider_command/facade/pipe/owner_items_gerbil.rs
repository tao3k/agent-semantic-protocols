use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_marker_provider,
};

#[test]
fn gerbil_owner_items_query_set_renders_scheme_item_selectors_without_provider() {
    let root = temp_project_root("search-owner-gerbil-query-set");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src/checker")).expect("create source");
    std::fs::write(
        root.join("src/checker/types.ss"),
        "(def (type-compatible? actual expected)\n  (or (equal? actual expected)\n      (and (equal? expected 'union)\n           (any-type-compatible? actual expected))))\n\n(def (any-type-compatible? actual expected-members)\n  (if (null? expected-members)\n    #f\n    (type-compatible? actual (car expected-members))))\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "gerbil-scheme-harness", &marker);
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
            "--view",
            "seeds",
            ".",
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
        stdout.contains("I=item:symbol(type-compatible?)@src/checker/types.ss:1:4!syntax"),
        "{stdout}"
    );
    assert!(
        stdout.contains("I2=item:symbol(any-type-compatible?)@src/checker/types.ss:6:9!syntax"),
        "{stdout}"
    );
    assert!(
        stdout.contains("nextCommand=asp gerbil-scheme query --selector src/checker/types.ss:1:4 --workspace . --code"),
        "{stdout}"
    );
    assert!(
        stdout.contains("reason=owner-item-selector-ready"),
        "{stdout}"
    );
    assert!(!stdout.contains("reason=no-owner-item-match"), "{stdout}");
    assert!(
        !marker.exists(),
        "Gerbil owner-items fast path should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn gerbil_owner_items_query_set_supports_ssi_sources_without_provider() {
    let root = temp_project_root("search-owner-gerbil-ssi-query-set");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src/api")).expect("create source");
    std::fs::write(
        root.join("src/api/types.ssi"),
        "(defstruct required-extension (name dependency-mode))\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "gerbil-scheme-harness", &marker);
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
            "--view",
            "seeds",
            ".",
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
        stdout.contains("I=item:symbol(required-extension)@src/api/types.ssi:1:1!syntax"),
        "{stdout}"
    );
    assert!(
        stdout.contains("nextCommand=asp gerbil-scheme query --selector src/api/types.ssi:1:1 --workspace . --code"),
        "{stdout}"
    );
    assert!(
        !marker.exists(),
        "Gerbil .ssi owner-items fast path should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn gerbil_owner_items_query_set_renders_config_owner_selector_without_provider() {
    let root = temp_project_root("search-owner-gerbil-config-query-set");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::write(
        root.join("gerbil.pkg"),
        "(package: sample/app\n depend: (\"git.cons.io/mighty-gerbils/gerbil-poo\"))\n",
    )
    .expect("write gerbil package");
    write_marker_provider(&bin_dir, "gerbil-scheme-harness", &marker);
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
            "--view",
            "seeds",
            ".",
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
    assert!(
        stdout.contains("reason=owner-item-selector-ready"),
        "{stdout}"
    );
    assert!(!stdout.contains("reason=no-owner-item-match"), "{stdout}");
    assert!(
        !marker.exists(),
        "Gerbil config owner-items fast path should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}
