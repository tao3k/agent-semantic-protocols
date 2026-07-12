use crate::provider_command::support::{
    asp_command, home_local_bin, make_executable, prepend_path, provider,
    provider_with_owner_items, temp_project_root, write_activation, write_marker_provider,
};
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

const OWNER_ITEMS_FACADE_DEBUG_SUBPROCESS_GATE: Duration = Duration::from_millis(750);

#[test]
fn language_owner_items_uses_dynamic_owner_items_without_provider_fallback() {
    let root = temp_project_root("search-owner-language-routes-to-harness");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("crate/src")).expect("create source");
    std::fs::write(
        root.join("crate/src/lib.rs"),
        "pub async fn dynamic_owner_item_index() {}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "owner",
            "crate/src/lib.rs",
            "items",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp language search owner items");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("alg=asp-dynamic-owner-items-v1"),
        "{stdout}"
    );
    assert!(stdout.contains("dynamic_owner_item_index"), "{stdout}");
    assert!(
        !marker.exists(),
        "dynamic owner-items should not invoke the provider fallback"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_owner_items_reuses_dynamic_owner_items_without_provider_cache() {
    let root = temp_project_root("search-owner-language-harness-cache");
    let bin_dir = home_local_bin(&root);
    let count_path = root.join("provider-count");
    std::fs::create_dir_all(root.join("crate/src")).expect("create source");
    std::fs::write(
        root.join("crate/src/lib.rs"),
        "pub async fn dynamic_owner_item_index() {}\n",
    )
    .expect("write source");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    let provider_path = bin_dir.join("rs-harness");
    std::fs::write(
        &provider_path,
        format!(
            "#!/bin/sh\ncount=0\nif [ -f '{count}' ]; then count=$(cat '{count}'); fi\ncount=$((count + 1))\nprintf '%s' \"$count\" > '{count}'\nprintf '[search-owner] q=crate/src/lib.rs pkg=. selector=items alg=item-frontier\\n'\nprintf 'O=owner:path(crate/src/lib.rs)!owner;I=item:symbol(dynamic_owner_item_index)@crate/src/lib.rs:1:1!syntax\\n'\n",
            count = count_path.display()
        ),
    )
    .expect("write provider");
    let mut permissions = std::fs::metadata(&provider_path)
        .expect("provider metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&provider_path, permissions).expect("chmod provider");
    write_activation(&root, &[provider("rust", Vec::new())]);

    for _ in 0..2 {
        let output = asp_command(&root)
            .env("PATH", prepend_path(&bin_dir))
            .env("PRJ_CACHE_HOME", root.join(".cache"))
            .args([
                "rust",
                "search",
                "owner",
                "crate/src/lib.rs",
                "items",
                "--workspace",
                ".",
                "--view",
                "seeds",
            ])
            .output()
            .expect("run asp language search owner items");
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).expect("stdout");
        assert!(
            stdout.contains("item:symbol(dynamic_owner_item_index)"),
            "{stdout}"
        );
    }

    assert!(
        !count_path.exists(),
        "dynamic owner-items should not invoke or cache the provider output"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_owner_items_provider_stdout_is_compacted_without_language_special_case() {
    let root = temp_project_root("search-owner-language-provider-stdout-compact");
    let bin_dir = home_local_bin(&root);
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src/pkg")).expect("create source");
    std::fs::write(
        root.join("src/pkg/service.lang"),
        "def fetch():\n    return 1\n",
    )
    .expect("write source");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    let provider_path = bin_dir.join("py-harness");
    std::fs::write(
        &provider_path,
        format!(
            "#!/bin/sh\nprintf called > '{}'\ncat <<'OUT'\n[search-owner] q=src/pkg/service.lang pkg=. selector=items alg=provider-owned-language-owner-items\n|item fetch kind=function structuralSelector=language://src/pkg/service.lang#item/function/fetch displayLineRange=1:2 sourceLocatorHint=src/pkg/service.lang:1:2 reason=owner-item-skeleton-ready\nactionFrontier=A1.item-skeleton,A2.syntax-outline,A3.query-code\nrecommendedNext=A1.item-skeleton\nreason=owner-item-skeleton-ready\nOUT\n",
            marker.display()
        ),
    )
    .expect("write provider");
    make_executable(&provider_path);
    write_activation(&root, &[provider_with_owner_items("python", Vec::new())]);

    let args = [
        "python",
        "search",
        "owner",
        "src/pkg/service.lang",
        "items",
        "--query",
        "fetch",
        "--workspace",
        ".",
        "--view",
        "hits",
    ];
    let warmup = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(args)
        .output()
        .expect("warm asp language search owner items");
    assert!(
        warmup.status.success(),
        "warm stderr: {}",
        String::from_utf8_lossy(&warmup.stderr)
    );

    let mut fastest = None;
    for _ in 0..5 {
        let started_at = Instant::now();
        let output = asp_command(&root)
            .env("PRJ_CACHE_HOME", root.join(".cache"))
            .args(args)
            .output()
            .expect("run asp language search owner items");
        let elapsed = started_at.elapsed();
        if fastest
            .as_ref()
            .is_none_or(|(best_elapsed, _)| elapsed < *best_elapsed)
        {
            fastest = Some((elapsed, output));
        }
        if elapsed < OWNER_ITEMS_FACADE_DEBUG_SUBPROCESS_GATE {
            break;
        }
    }
    let (elapsed, output) = fastest.expect("language owner-items sample");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        marker.exists(),
        "owner-items should call provider-owned owner-items"
    );
    assert!(
        stdout.contains("alg=provider-owned-language-owner-items")
            && stdout.contains("|item fetch kind=function")
            && stdout
                .contains("structuralSelector=language://src/pkg/service.lang#item/function/fetch")
            && stdout.contains("displayLineRange=1:2")
            && stdout.contains("sourceLocatorHint=src/pkg/service.lang:1:2")
            && stdout.contains("reason=owner-item-skeleton-ready"),
        "{stdout}"
    );
    assert!(!stdout.contains("actionFrontier="), "{stdout}");
    assert!(!stdout.contains("recommendedNext="), "{stdout}");
    assert!(
        !stdout.contains("read=src/pkg/service.lang:1:2"),
        "line range must not remain an executable read selector: {stdout}"
    );
    assert!(
        !stdout.contains("fallback="),
        "owner-items success output must not advertise fallback: {stdout}"
    );
    assert!(
        elapsed < OWNER_ITEMS_FACADE_DEBUG_SUBPROCESS_GATE,
        "language owner-items debug subprocess exceeded {OWNER_ITEMS_FACADE_DEBUG_SUBPROCESS_GATE:?}; elapsed={elapsed:?}; stdout={stdout}; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_owner_items_missing_owner_errors_without_provider_fallback() {
    let root = temp_project_root("search-owner-language-items-no-fallback");
    let bin_dir = home_local_bin(&root);
    let marker = root.join("provider-called");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    let provider_path = bin_dir.join("py-harness");
    std::fs::write(
        &provider_path,
        format!(
            "#!/bin/sh\nprintf called > '{}'\nprintf 'provider should not run\\n' >&2\nexit 2\n",
            marker.display()
        ),
    )
    .expect("write provider");
    make_executable(&provider_path);
    write_activation(&root, &[provider_with_owner_items("python", Vec::new())]);

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "python",
            "search",
            "owner",
            "src/pkg/missing.py",
            "items",
            "--query",
            "fetch",
            "--workspace",
            ".",
            "--view",
            "hits",
        ])
        .output()
        .expect("run asp language search owner items");

    assert!(!output.status.success(), "owner-items should hard-fail");
    assert!(
        !marker.exists(),
        "owner-items should not call provider after exact-owner miss"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("reason=missing-owner")
            && stderr.contains(
                "search owner requires a concrete source owner path; workspace roots and directories are search scopes, not owners"
            ),
        "{stderr}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("[search-owner]"),
        "missing exact owner should not render fallback owner-query: {stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}
